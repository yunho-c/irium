use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use ratatui::text::Line;

use crate::{
    fs_scan::{self, FsEntry},
    mock,
    model::{
        AppState, CategoryFilter, FileNode, FileTree, FocusPane, InputMode, MarketplacePreset,
        NamingTab, PresetState, RenameRow, ScopeTab, SessionUndoEntry, SizeConstraint, Stage,
        StyleOptions, TimeConstraint, Toast, ToastLevel, VisibleNode,
    },
};

impl AppState {
    pub fn new(cwd: PathBuf) -> Self {
        let tree = FileTree::new(cwd.clone());
        let mut app = Self {
            cwd,
            stage: Stage::Scope,
            scope_tab: ScopeTab::Files,
            scope_left_tab: ScopeTab::Files,
            scope_right_tab: ScopeTab::Constraint,
            naming_tab: NamingTab::Suggestions,
            focus: FocusPane::ScopeFiles,
            focus_manager: Default::default(),
            mode: InputMode::Normal,
            should_quit: false,
            show_help: false,
            show_hidden: false,
            files_settings_open: false,
            show_selected_categories_only: false,
            tree,
            scan_error: None,
            category_filters: mock::default_category_filters(),
            selected_extensions: HashSet::new(),
            select_cursor: 0,
            new_category_input: String::new(),
            time_constraint: TimeConstraint::Any,
            size_constraint: SizeConstraint::Any,
            constraint_cursor: 0,
            presets: vec![None; 9],
            preset_cursor: 0,
            marketplace_presets: mock::default_marketplace_presets(),
            marketplace_cursor: 0,
            rename_rows: Vec::new(),
            rename_cursor: 0,
            rename_scroll: 0,
            suggestions: mock::make_suggestion_options(),
            suggestion_cursor: 0,
            style: StyleOptions::default(),
            style_cursor: 0,
            command_input: String::new(),
            override_input: String::new(),
            editing_row: None,
            override_vocab: Default::default(),
            toasts: vec![Toast {
                level: ToastLevel::Info,
                message: "Welcome to irm. Scope files, shape names, and simulate apply."
                    .to_string(),
                ttl_ticks: 55,
            }],
            undo_history: Vec::new(),
            apply_cursor: 0,
            history_cursor: 0,
            click_regions: Default::default(),
            ticks: 0,
            help_lines: vec![
                Line::from("Global: q quit | [ / ] stage | Tab focus | ? help"),
                Line::from(
                    "Files: arrows move | Space select | Left/Right collapse/expand | Ctrl+Right expand all",
                ),
                Line::from(
                    "Naming: e edit override | g then 1-9 assign group | / style command | Enter apply",
                ),
                Line::from("Apply: Enter submit+exit (simulated) | Ctrl+Enter submit+stay"),
            ],
            selected_by_tree: HashSet::new(),
            filter_cache: HashMap::new(),
        };

        app.sync_focus_manager();
        app.reload_tree();
        app.sync_rename_rows();
        app
    }

    pub fn set_scope_tab(&mut self, tab: ScopeTab) {
        self.scope_tab = tab;
        match tab {
            ScopeTab::Files | ScopeTab::Select => self.scope_left_tab = tab,
            ScopeTab::Constraint | ScopeTab::Preset | ScopeTab::Marketplace => {
                self.scope_right_tab = tab;
            }
        }

        if self.stage == Stage::Scope {
            let focus = match tab {
                ScopeTab::Select => FocusPane::ScopeCategory,
                ScopeTab::Files => FocusPane::ScopeFiles,
                ScopeTab::Constraint | ScopeTab::Preset | ScopeTab::Marketplace => {
                    FocusPane::ScopeOptions
                }
            };
            self.focus = focus;
            self.focus_manager.set(focus);
        }
    }

    pub fn next_scope_tab(&mut self) {
        self.set_scope_tab(self.scope_tab.next());
    }

    pub fn prev_scope_tab(&mut self) {
        self.set_scope_tab(self.scope_tab.prev());
    }

    pub fn stage_focuses(&self) -> &'static [FocusPane] {
        match self.stage {
            Stage::Scope => &[
                FocusPane::ScopeCategory,
                FocusPane::ScopeFiles,
                FocusPane::ScopeOptions,
            ],
            Stage::Naming => &[
                FocusPane::NamingTable,
                FocusPane::NamingRight,
                FocusPane::NamingCommand,
            ],
            Stage::Apply => &[FocusPane::ApplyReview, FocusPane::ApplyHistory],
        }
    }

    pub fn next_focus(&mut self) {
        self.focus_manager.next();
        if let Some(focus) = self.focus_manager.current() {
            self.focus = *focus;
        }
        self.sync_scope_tab_from_focus();
    }

    pub fn prev_focus(&mut self) {
        self.focus_manager.prev();
        if let Some(focus) = self.focus_manager.current() {
            self.focus = *focus;
        }
        self.sync_scope_tab_from_focus();
    }

    pub fn set_stage(&mut self, stage: Stage) {
        self.stage = stage;
        self.sync_focus_manager();
        self.set_focus(self.stage_focuses()[0]);
        if self.stage == Stage::Naming {
            self.sync_rename_rows();
        }
    }

    pub fn set_focus(&mut self, focus: FocusPane) {
        self.focus = focus;
        self.focus_manager.set(focus);
        self.sync_scope_tab_from_focus();
    }

    fn sync_scope_tab_from_focus(&mut self) {
        if self.stage != Stage::Scope {
            return;
        }

        match self.focus {
            FocusPane::ScopeCategory => self.scope_tab = ScopeTab::Select,
            FocusPane::ScopeFiles => self.scope_tab = ScopeTab::Files,
            FocusPane::ScopeOptions => {
                if matches!(self.scope_tab, ScopeTab::Files | ScopeTab::Select) {
                    self.scope_tab = self.scope_right_tab;
                }
            }
            FocusPane::NamingTable
            | FocusPane::NamingRight
            | FocusPane::NamingCommand
            | FocusPane::ApplyReview
            | FocusPane::ApplyHistory => {}
        }
    }

    fn sync_focus_manager(&mut self) {
        self.focus_manager.clear();
        self.focus_manager
            .register_all(self.stage_focuses().iter().copied());
        self.focus_manager.set(self.focus);
        if let Some(current) = self.focus_manager.current() {
            self.focus = *current;
        } else if let Some(first) = self.stage_focuses().first().copied() {
            self.focus = first;
            self.focus_manager.set(first);
        }
    }

    pub fn tick(&mut self) {
        self.ticks = self.ticks.saturating_add(1);
        for toast in &mut self.toasts {
            toast.ttl_ticks = toast.ttl_ticks.saturating_sub(1);
        }
        self.toasts.retain(|toast| toast.ttl_ticks > 0);
    }

    pub fn push_toast(&mut self, level: ToastLevel, message: impl Into<String>) {
        self.toasts.push(Toast {
            level,
            message: message.into(),
            ttl_ticks: 45,
        });

        if self.toasts.len() > 4 {
            let drop_count = self.toasts.len() - 4;
            self.toasts.drain(0..drop_count);
        }
    }

    pub fn reload_tree(&mut self) {
        self.tree = FileTree::new(self.cwd.clone());
        self.scan_error = None;
        if let Err(error) = self.ensure_children_loaded(self.tree.root) {
            self.scan_error = Some(error);
        }
        self.normalize_scope_files_cursor();
    }

    pub fn scope_files_rows(&self) -> Vec<VisibleNode> {
        if self.show_selected_categories_only && self.selected_extensions.is_empty() {
            return Vec::new();
        }

        let mut rows = self.tree.visible_nodes();
        if !self.show_selected_categories_only {
            return rows;
        }

        let mut directory_match_cache: HashMap<PathBuf, bool> = HashMap::new();
        rows.retain(|row| {
            self.node_matches_selected_categories(row.node_id, &mut directory_match_cache)
        });
        rows
    }

    pub fn scope_files_rows_len(&self) -> usize {
        self.scope_files_rows().len()
    }

    pub fn current_scope_file_node_id(&self) -> Option<usize> {
        let rows = self.scope_files_rows();
        if rows.is_empty() {
            return None;
        }
        Some(rows[self.tree.cursor.min(rows.len() - 1)].node_id)
    }

    pub fn normalize_scope_files_cursor(&mut self) {
        let len = self.scope_files_rows_len();
        if len == 0 {
            self.tree.cursor = 0;
            self.tree.scroll = 0;
            return;
        }
        self.tree.cursor = self.tree.cursor.min(len - 1);
        self.tree.scroll = self.tree.scroll.min(self.tree.cursor);
    }

    pub fn toggle_files_settings_popup(&mut self) {
        self.files_settings_open = !self.files_settings_open;
    }

    pub fn toggle_show_selected_categories_only(&mut self) {
        self.show_selected_categories_only = !self.show_selected_categories_only;
        self.normalize_scope_files_cursor();
        self.push_toast(
            ToastLevel::Info,
            if self.show_selected_categories_only {
                "Files view: showing selected categories only"
            } else {
                "Files view: showing all categories"
            },
        );
    }

    fn node_matches_selected_categories(
        &self,
        node_id: usize,
        directory_match_cache: &mut HashMap<PathBuf, bool>,
    ) -> bool {
        let node = &self.tree.nodes[node_id];
        if node.is_dir {
            if let Some(cached) = directory_match_cache.get(&node.path) {
                return *cached;
            }

            let has_matching_descendant = fs_scan::collect_files(&node.path, self.show_hidden, 2500)
                .into_iter()
                .any(|entry| {
                    let ext = entry.extension.unwrap_or_default();
                    self.selected_extensions.contains(&ext)
                });
            directory_match_cache.insert(node.path.clone(), has_matching_descendant);
            return has_matching_descendant;
        }

        let ext = node
            .path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_lowercase())
            .unwrap_or_default();
        self.selected_extensions.contains(&ext)
    }

    pub fn ensure_children_loaded(&mut self, node_id: usize) -> Result<(), String> {
        if !self.tree.nodes[node_id].is_dir || self.tree.nodes[node_id].children_loaded {
            return Ok(());
        }

        let path = self.tree.nodes[node_id].path.clone();
        match fs_scan::scan_children(&path, self.show_hidden) {
            Ok(entries) => {
                let mut child_ids = Vec::new();
                for entry in entries {
                    let child_id = self.tree.nodes.len();
                    self.tree.nodes.push(FileNode {
                        parent: Some(node_id),
                        path: entry.path,
                        name: entry.name,
                        is_dir: entry.is_dir,
                        children: Vec::new(),
                        children_loaded: false,
                        expanded: false,
                        selected: false,
                        unreadable: false,
                    });
                    child_ids.push(child_id);
                }
                self.tree.nodes[node_id].children = child_ids;
                self.tree.nodes[node_id].children_loaded = true;
                Ok(())
            }
            Err(error) => {
                self.tree.nodes[node_id].unreadable = true;
                Err(error)
            }
        }
    }

    pub fn move_up(&mut self) {
        match self.stage {
            Stage::Scope => self.move_scope_up(),
            Stage::Naming => self.move_naming_up(),
            Stage::Apply => self.move_apply_up(),
        }
    }

    pub fn move_down(&mut self) {
        match self.stage {
            Stage::Scope => self.move_scope_down(),
            Stage::Naming => self.move_naming_down(),
            Stage::Apply => self.move_apply_down(),
        }
    }

    fn move_scope_up(&mut self) {
        match self.scope_tab {
            ScopeTab::Files => {
                self.tree.cursor = self.tree.cursor.saturating_sub(1);
                self.normalize_scope_files_cursor();
            }
            ScopeTab::Select => {
                self.select_cursor = self.select_cursor.saturating_sub(1);
            }
            ScopeTab::Constraint => {
                self.constraint_cursor = self.constraint_cursor.saturating_sub(1);
            }
            ScopeTab::Preset => {
                self.preset_cursor = self.preset_cursor.saturating_sub(1);
            }
            ScopeTab::Marketplace => {
                self.marketplace_cursor = self.marketplace_cursor.saturating_sub(1);
            }
        }
    }

    fn move_scope_down(&mut self) {
        match self.scope_tab {
            ScopeTab::Files => {
                let count = self.scope_files_rows_len();
                if count > 0 {
                    self.tree.cursor = (self.tree.cursor + 1).min(count - 1);
                    self.normalize_scope_files_cursor();
                }
            }
            ScopeTab::Select => {
                if !self.category_filters.is_empty() {
                    self.select_cursor =
                        (self.select_cursor + 1).min(self.category_filters.len() - 1);
                }
            }
            ScopeTab::Constraint => {
                let max = TimeConstraint::ALL.len() + SizeConstraint::ALL.len() - 1;
                self.constraint_cursor = (self.constraint_cursor + 1).min(max);
            }
            ScopeTab::Preset => {
                self.preset_cursor = (self.preset_cursor + 1).min(8);
            }
            ScopeTab::Marketplace => {
                if !self.marketplace_presets.is_empty() {
                    self.marketplace_cursor =
                        (self.marketplace_cursor + 1).min(self.marketplace_presets.len() - 1);
                }
            }
        }
    }

    fn move_naming_up(&mut self) {
        match self.focus {
            FocusPane::NamingTable => {
                self.rename_cursor = self.rename_cursor.saturating_sub(1);
            }
            FocusPane::NamingRight => match self.naming_tab {
                NamingTab::Suggestions => {
                    self.suggestion_cursor = self.suggestion_cursor.saturating_sub(1);
                }
                NamingTab::Style => {
                    self.style_cursor = self.style_cursor.saturating_sub(1);
                }
            },
            FocusPane::NamingCommand => {}
            _ => {}
        }
    }

    fn move_naming_down(&mut self) {
        match self.focus {
            FocusPane::NamingTable => {
                if !self.rename_rows.is_empty() {
                    self.rename_cursor = (self.rename_cursor + 1).min(self.rename_rows.len() - 1);
                }
            }
            FocusPane::NamingRight => match self.naming_tab {
                NamingTab::Suggestions => {
                    if !self.suggestions.is_empty() {
                        self.suggestion_cursor =
                            (self.suggestion_cursor + 1).min(self.suggestions.len() - 1);
                    }
                }
                NamingTab::Style => {
                    self.style_cursor = (self.style_cursor + 1).min(4);
                }
            },
            FocusPane::NamingCommand => {}
            _ => {}
        }
    }

    fn move_apply_up(&mut self) {
        match self.focus {
            FocusPane::ApplyReview => {
                self.apply_cursor = self.apply_cursor.saturating_sub(1);
            }
            FocusPane::ApplyHistory => {
                self.history_cursor = self.history_cursor.saturating_sub(1);
            }
            _ => {}
        }
    }

    fn move_apply_down(&mut self) {
        match self.focus {
            FocusPane::ApplyReview => {
                if !self.rename_rows.is_empty() {
                    self.apply_cursor = (self.apply_cursor + 1).min(self.rename_rows.len() - 1);
                }
            }
            FocusPane::ApplyHistory => {
                if !self.undo_history.is_empty() {
                    self.history_cursor =
                        (self.history_cursor + 1).min(self.undo_history.len() - 1);
                }
            }
            _ => {}
        }
    }

    pub fn expand_current(&mut self) {
        if self.stage != Stage::Scope || self.scope_tab != ScopeTab::Files {
            return;
        }

        let Some(node_id) = self.current_scope_file_node_id() else {
            return;
        };

        if !self.tree.nodes[node_id].is_dir {
            return;
        }

        if let Err(error) = self.ensure_children_loaded(node_id) {
            self.push_toast(
                ToastLevel::Error,
                format!("Could not expand folder: {error}"),
            );
            return;
        }

        self.tree.nodes[node_id].expanded = true;
    }

    pub fn collapse_current(&mut self) {
        if self.stage != Stage::Scope || self.scope_tab != ScopeTab::Files {
            return;
        }

        let Some(node_id) = self.current_scope_file_node_id() else {
            return;
        };

        if self.tree.nodes[node_id].is_dir && self.tree.nodes[node_id].expanded {
            self.tree.nodes[node_id].expanded = false;
            self.normalize_scope_files_cursor();
            return;
        }

        if let Some(parent) = self.tree.nodes[node_id].parent {
            let visible = self.scope_files_rows();
            if let Some(index) = visible.iter().position(|row| row.node_id == parent) {
                self.tree.cursor = index;
            }
        }
        self.normalize_scope_files_cursor();
    }

    pub fn expand_all_current(&mut self) {
        if self.stage != Stage::Scope || self.scope_tab != ScopeTab::Files {
            return;
        }

        let Some(node_id) = self.current_scope_file_node_id() else {
            return;
        };

        if let Err(error) = self.expand_recursive(node_id) {
            self.push_toast(
                ToastLevel::Warning,
                format!("Expanded partially (some folders unreadable): {error}"),
            );
        }
        self.normalize_scope_files_cursor();
    }

    fn expand_recursive(&mut self, node_id: usize) -> Result<(), String> {
        if !self.tree.nodes[node_id].is_dir {
            return Ok(());
        }

        self.ensure_children_loaded(node_id)?;
        self.tree.nodes[node_id].expanded = true;

        let children = self.tree.nodes[node_id].children.clone();
        for child in children {
            let _ = self.expand_recursive(child);
        }

        Ok(())
    }

    pub fn collapse_all_current(&mut self) {
        if self.stage != Stage::Scope || self.scope_tab != ScopeTab::Files {
            return;
        }

        let Some(node_id) = self.current_scope_file_node_id() else {
            return;
        };

        self.collapse_recursive(node_id);
        self.normalize_scope_files_cursor();
    }

    fn collapse_recursive(&mut self, node_id: usize) {
        self.tree.nodes[node_id].expanded = false;
        let children = self.tree.nodes[node_id].children.clone();
        for child in children {
            self.collapse_recursive(child);
        }
    }

    pub fn toggle_scope_file_selection(&mut self) {
        let Some(node_id) = self.current_scope_file_node_id() else {
            return;
        };

        let selected = !self.node_selected_for_display(node_id);
        if let Err(error) = self.set_node_selected(node_id, selected) {
            self.push_toast(
                ToastLevel::Error,
                format!("Selection update failed: {error}"),
            );
        }
        self.sync_rename_rows();
    }

    pub fn select_all_visible_files(&mut self) {
        let visible_file_nodes: Vec<usize> = self
            .scope_files_rows()
            .into_iter()
            .map(|row| row.node_id)
            .filter(|node_id| !self.tree.nodes[*node_id].is_dir)
            .collect();

        if visible_file_nodes.is_empty() {
            self.push_toast(ToastLevel::Warning, "No visible files to select");
            self.sync_rename_rows();
            return;
        }

        let all_selected = visible_file_nodes
            .iter()
            .all(|node_id| self.tree.nodes[*node_id].selected);

        for node_id in &visible_file_nodes {
            self.tree.nodes[*node_id].selected = !all_selected;
            let path = self.tree.nodes[*node_id].path.clone();
            if all_selected {
                self.selected_by_tree.remove(&path);
            } else {
                self.selected_by_tree.insert(path);
            }
        }

        let affected = visible_file_nodes.len();
        if all_selected {
            self.push_toast(
                ToastLevel::Success,
                format!("Deselected {affected} visible file(s)"),
            );
        } else {
            self.push_toast(
                ToastLevel::Success,
                format!("Selected {affected} visible file(s)"),
            );
        }
        self.sync_rename_rows();
    }

    pub fn node_selected_for_display(&self, node_id: usize) -> bool {
        let node = &self.tree.nodes[node_id];
        if !node.is_dir {
            return node.selected;
        }

        let (has_descendants, all_selected) = self.subtree_selection_state(node_id);
        if has_descendants {
            all_selected
        } else {
            node.selected
        }
    }

    fn subtree_selection_state(&self, node_id: usize) -> (bool, bool) {
        let node = &self.tree.nodes[node_id];
        if !node.is_dir {
            return (true, node.selected);
        }

        if !node.children_loaded {
            // Treat unloaded directories as a single unit using their explicit selection bit.
            return (true, node.selected);
        }

        let mut has_descendants = false;
        let mut all_selected = true;
        for child_id in &node.children {
            let (child_has_descendants, child_all_selected) =
                self.subtree_selection_state(*child_id);
            if child_has_descendants {
                has_descendants = true;
                all_selected &= child_all_selected;
            }
        }

        (has_descendants, all_selected)
    }

    fn set_node_selected(&mut self, node_id: usize, selected: bool) -> Result<(), String> {
        self.tree.nodes[node_id].selected = selected;

        if self.tree.nodes[node_id].is_dir {
            self.ensure_children_loaded(node_id)?;
            let children = self.tree.nodes[node_id].children.clone();
            for child in children {
                let _ = self.set_node_selected(child, selected);
            }
            return Ok(());
        }

        let path = self.tree.nodes[node_id].path.clone();
        if selected {
            self.selected_by_tree.insert(path);
        } else {
            self.selected_by_tree.remove(&path);
        }

        Ok(())
    }

    pub fn toggle_current_scope_filter(&mut self) {
        match self.scope_tab {
            ScopeTab::Select => {
                if self.select_cursor < self.category_filters.len() {
                    let category = &self.category_filters[self.select_cursor];
                    let all_selected = category
                        .extensions
                        .iter()
                        .all(|ext| self.selected_extensions.contains(ext));
                    for ext in &category.extensions {
                        if all_selected {
                            self.selected_extensions.remove(ext);
                        } else {
                            self.selected_extensions.insert(ext.clone());
                        }
                    }
                    self.push_toast(
                        ToastLevel::Info,
                        format!(
                            "{} {}",
                            if all_selected { "Disabled" } else { "Enabled" },
                            category.name
                        ),
                    );
                    self.sync_rename_rows();
                }
            }
            ScopeTab::Constraint => {
                let idx = self.constraint_cursor;
                if idx < TimeConstraint::ALL.len() {
                    self.time_constraint = TimeConstraint::ALL[idx];
                    self.push_toast(
                        ToastLevel::Info,
                        format!("Time constraint: {}", self.time_constraint.title()),
                    );
                } else {
                    let s_idx = idx - TimeConstraint::ALL.len();
                    if s_idx < SizeConstraint::ALL.len() {
                        self.size_constraint = SizeConstraint::ALL[s_idx];
                        self.push_toast(
                            ToastLevel::Info,
                            format!("Size constraint: {}", self.size_constraint.title()),
                        );
                    }
                }
                self.sync_rename_rows();
            }
            ScopeTab::Marketplace => {
                self.apply_marketplace(self.marketplace_cursor);
            }
            _ => {}
        }
    }

    pub fn add_custom_category_from_input(&mut self) {
        let raw = self.new_category_input.trim();
        if raw.is_empty() {
            self.push_toast(ToastLevel::Warning, "Category input is empty");
            return;
        }

        let (name, ext_part) = if let Some((name, rest)) = raw.split_once(':') {
            (name.trim().to_string(), rest.trim().to_string())
        } else {
            (raw.to_string(), raw.to_string())
        };

        let extensions: Vec<String> = ext_part
            .split(',')
            .map(|token| token.trim().trim_start_matches('.').to_lowercase())
            .filter(|token| !token.is_empty())
            .collect();

        if extensions.is_empty() {
            self.push_toast(
                ToastLevel::Warning,
                "Add at least one extension, e.g. Research:pdf,md",
            );
            return;
        }

        self.category_filters
            .push(CategoryFilter { name, extensions });
        self.select_cursor = self.category_filters.len().saturating_sub(1);
        self.new_category_input.clear();
        self.mode = InputMode::Normal;
        self.push_toast(ToastLevel::Success, "Custom category added");
    }

    pub fn save_preset(&mut self, slot: u8) {
        if !(1..=9).contains(&slot) {
            return;
        }

        self.presets[(slot - 1) as usize] = Some(PresetState {
            selected_extensions: self.selected_extensions.clone(),
            time_constraint: self.time_constraint,
            size_constraint: self.size_constraint,
            style: self.style.clone(),
            show_hidden: self.show_hidden,
        });
        self.push_toast(ToastLevel::Success, format!("Saved preset to {slot}"));
    }

    pub fn load_preset(&mut self, slot: u8) {
        if !(1..=9).contains(&slot) {
            return;
        }

        let Some(preset) = self.presets[(slot - 1) as usize].clone() else {
            self.push_toast(ToastLevel::Warning, format!("Preset {slot} is empty"));
            return;
        };

        self.selected_extensions = preset.selected_extensions;
        self.time_constraint = preset.time_constraint;
        self.size_constraint = preset.size_constraint;
        self.style = preset.style;

        if self.show_hidden != preset.show_hidden {
            self.show_hidden = preset.show_hidden;
            self.reload_tree();
        }

        self.sync_rename_rows();
        self.push_toast(ToastLevel::Success, format!("Loaded preset {slot}"));
    }

    pub fn apply_marketplace(&mut self, idx: usize) {
        if idx >= self.marketplace_presets.len() {
            return;
        }
        let preset: MarketplacePreset = self.marketplace_presets[idx].clone();
        self.selected_extensions = preset.extensions.iter().cloned().collect();
        self.time_constraint = preset.time_constraint;
        self.set_scope_tab(ScopeTab::Select);
        self.sync_rename_rows();
        self.push_toast(
            ToastLevel::Success,
            format!("Applied marketplace preset: {}", preset.name),
        );
    }

    fn file_name_parts(path: &Path) -> (String, Option<String>) {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("file")
            .to_string();
        let stem = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("file")
            .to_string();
        let ext = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase());
        let fallback_ext = if file_name == stem {
            None
        } else {
            file_name
                .rsplit_once('.')
                .map(|(_, ext)| ext.to_lowercase())
        };
        (stem, ext.or(fallback_ext))
    }

    pub fn sync_rename_rows(&mut self) {
        let all_files = fs_scan::collect_files(&self.cwd, self.show_hidden, 2500);
        let mut include_paths = HashSet::new();

        for path in &self.selected_by_tree {
            include_paths.insert(path.clone());
        }

        self.filter_cache.clear();
        for file in &all_files {
            let include = self.matches_filters(file);
            self.filter_cache.insert(file.path.clone(), include);
            if include {
                include_paths.insert(file.path.clone());
            }
        }

        let mut old_rows = HashMap::new();
        for row in self.rename_rows.drain(..) {
            old_rows.insert(row.path.clone(), row);
        }

        let mut rows = Vec::new();
        let mut included_files: Vec<FsEntry> = all_files
            .into_iter()
            .filter(|entry| include_paths.contains(&entry.path))
            .collect();
        included_files.sort_by(|a, b| a.path.cmp(&b.path));

        for entry in included_files {
            let (stem, ext) = Self::file_name_parts(&entry.path);
            let existing = old_rows.remove(&entry.path);
            let override_name = existing.as_ref().and_then(|row| row.override_name.clone());
            let selected = existing.as_ref().map(|row| row.selected).unwrap_or(true);
            let group_id = existing.as_ref().map(|row| row.group_id).unwrap_or(1);

            let proposed_name = self.propose_name(&stem, ext.as_deref(), override_name.as_deref());
            let current_name = entry
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown")
                .to_string();

            rows.push(RenameRow {
                path: entry.path,
                current_name,
                proposed_name,
                override_name,
                group_id,
                selected,
                conflict: false,
            });
        }

        self.rename_rows = rows;
        if self.rename_rows.is_empty() {
            self.rename_cursor = 0;
        } else {
            self.rename_cursor = self.rename_cursor.min(self.rename_rows.len() - 1);
        }

        self.recompute_conflicts();
    }

    fn propose_name(&self, stem: &str, ext: Option<&str>, override_name: Option<&str>) -> String {
        if let Some(override_name) = override_name {
            let trimmed = override_name.trim();
            if !trimmed.is_empty() {
                if self.style.keep_extension
                    && let Some(ext) = ext
                {
                    let suffix = format!(".{ext}");
                    if !trimmed.to_lowercase().ends_with(&suffix) {
                        return format!("{trimmed}{suffix}");
                    }
                }
                return trimmed.to_string();
            }
        }

        mock::format_name(stem, ext, &self.style)
    }

    pub fn recompute_proposals(&mut self) {
        for idx in 0..self.rename_rows.len() {
            let path = self.rename_rows[idx].path.clone();
            let (stem, ext) = Self::file_name_parts(&path);
            let override_name = self.rename_rows[idx].override_name.clone();
            self.rename_rows[idx].proposed_name =
                self.propose_name(&stem, ext.as_deref(), override_name.as_deref());
        }
        self.recompute_conflicts();
    }

    pub fn recompute_conflicts(&mut self) {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for row in &self.rename_rows {
            if row.selected {
                *counts.entry(row.proposed_name.clone()).or_insert(0) += 1;
            }
        }

        for row in &mut self.rename_rows {
            row.conflict = row.selected && counts.get(&row.proposed_name).copied().unwrap_or(0) > 1;
        }
    }

    fn matches_filters(&self, file: &FsEntry) -> bool {
        if !self.selected_extensions.is_empty() {
            let ext = file.extension.clone().unwrap_or_default();
            if !self.selected_extensions.contains(&ext) {
                return false;
            }
        }

        if !mock::within_time_constraint(file.modified, self.time_constraint) {
            return false;
        }

        if !mock::within_size_constraint(file.size, self.size_constraint) {
            return false;
        }

        true
    }

    pub fn apply_suggestion(&mut self) {
        if self.suggestion_cursor >= self.suggestions.len() {
            return;
        }
        self.style = self.suggestions[self.suggestion_cursor].style.clone();
        self.recompute_proposals();
        self.push_toast(
            ToastLevel::Success,
            format!(
                "Applied suggestion: {}",
                self.suggestions[self.suggestion_cursor].label
            ),
        );
    }

    pub fn toggle_current_naming_row(&mut self) {
        if self.rename_rows.is_empty() {
            return;
        }
        if let Some(row) = self.rename_rows.get_mut(self.rename_cursor) {
            row.selected = !row.selected;
            self.recompute_conflicts();
        }
    }

    pub fn begin_override_edit(&mut self) {
        if self.rename_rows.is_empty() {
            return;
        }
        self.mode = InputMode::EditingOverride;
        self.editing_row = Some(self.rename_cursor);
        self.override_input = self.rename_rows[self.rename_cursor]
            .override_name
            .clone()
            .unwrap_or_else(|| {
                self.rename_rows[self.rename_cursor]
                    .current_name
                    .rsplit_once('.')
                    .map(|(stem, _)| stem.to_string())
                    .unwrap_or_else(|| self.rename_rows[self.rename_cursor].current_name.clone())
            });
    }

    pub fn commit_override_edit(&mut self) {
        let Some(row_idx) = self.editing_row else {
            return;
        };
        if row_idx >= self.rename_rows.len() {
            return;
        }

        let value = self.override_input.trim().to_string();
        if value.is_empty() {
            self.rename_rows[row_idx].override_name = None;
        } else {
            self.override_vocab.insert(value.clone());
            self.rename_rows[row_idx].override_name = Some(value);
        }
        self.mode = InputMode::Normal;
        self.editing_row = None;
        self.override_input.clear();
        self.recompute_proposals();
        self.push_toast(ToastLevel::Success, "Override updated");
    }

    pub fn cancel_override_edit(&mut self) {
        self.mode = InputMode::Normal;
        self.editing_row = None;
        self.override_input.clear();
    }

    pub fn autocomplete_override(&mut self) {
        let prefix = self.override_input.trim().to_lowercase();
        if prefix.is_empty() {
            return;
        }

        if let Some(candidate) = self
            .override_vocab
            .iter()
            .find(|item| item.to_lowercase().starts_with(&prefix) && item.to_lowercase() != prefix)
        {
            self.override_input = candidate.clone();
        }
    }

    pub fn apply_style_cursor_delta(&mut self, delta: i8) {
        match self.style_cursor {
            0 => {
                self.style.length = if delta > 0 {
                    self.style.length.next()
                } else {
                    self.style.length.prev()
                };
            }
            1 => {
                self.style.capitalization = if delta > 0 {
                    self.style.capitalization.next()
                } else {
                    self.style.capitalization.prev()
                };
            }
            2 => {
                self.style.separator = if delta > 0 {
                    self.style.separator.next()
                } else {
                    self.style.separator.prev()
                };
            }
            3 => {
                self.style.keep_extension = !self.style.keep_extension;
            }
            4 => {
                self.style.strip_colons = !self.style.strip_colons;
            }
            _ => {}
        }
        self.recompute_proposals();
    }

    pub fn apply_command_input(&mut self) {
        let command = self.command_input.clone();
        let changed = mock::parse_style_command(&command, &mut self.style);
        self.command_input.clear();
        self.mode = InputMode::Normal;
        if changed == 0 {
            self.push_toast(
                ToastLevel::Warning,
                "No known style token found. Try: short title dash keep-ext",
            );
            return;
        }
        self.recompute_proposals();
        self.push_toast(
            ToastLevel::Success,
            format!("Applied {changed} style command(s)"),
        );
    }

    pub fn assign_group(&mut self, group_id: u8) {
        if !(1..=9).contains(&group_id) {
            return;
        }

        let mut changed = 0usize;
        for row in &mut self.rename_rows {
            if row.selected {
                row.group_id = group_id;
                changed += 1;
            }
        }

        if changed == 0
            && let Some(row) = self.rename_rows.get_mut(self.rename_cursor)
        {
            row.group_id = group_id;
            changed = 1;
        }

        self.mode = InputMode::Normal;
        self.push_toast(
            ToastLevel::Info,
            format!("Assigned group {group_id} to {changed} row(s)"),
        );
    }

    pub fn submit_simulated(&mut self, stay: bool) {
        let selected: Vec<&RenameRow> =
            self.rename_rows.iter().filter(|row| row.selected).collect();
        if selected.is_empty() {
            self.push_toast(ToastLevel::Warning, "No selected rows to apply");
            return;
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|delta| delta.as_secs().to_string())
            .unwrap_or_else(|_| "0".to_string());

        let entry = SessionUndoEntry {
            timestamp,
            affected_paths: selected.iter().map(|row| row.path.clone()).collect(),
            previous_names: selected
                .iter()
                .map(|row| row.current_name.clone())
                .collect(),
            simulated_new_names: selected
                .iter()
                .map(|row| row.proposed_name.clone())
                .collect(),
        };

        self.undo_history.insert(0, entry);
        if self.undo_history.len() > 20 {
            self.undo_history.truncate(20);
        }

        self.push_toast(
            ToastLevel::Success,
            format!("Simulated apply for {} file(s)", selected.len()),
        );

        if !stay {
            self.should_quit = true;
        }
    }

    pub fn toggle_show_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.reload_tree();
        self.sync_rename_rows();
        self.push_toast(
            ToastLevel::Info,
            if self.show_hidden {
                "Showing hidden files"
            } else {
                "Hiding hidden files"
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_save_load_roundtrip() {
        let cwd = std::env::current_dir().expect("cwd");
        let mut app = AppState::new(cwd);
        app.selected_extensions.insert("pdf".to_string());
        app.time_constraint = TimeConstraint::Week;
        app.save_preset(1);

        app.selected_extensions.clear();
        app.time_constraint = TimeConstraint::Any;
        app.load_preset(1);

        assert!(app.selected_extensions.contains("pdf"));
        assert_eq!(app.time_constraint, TimeConstraint::Week);
    }

    #[test]
    fn submit_without_selection_warns() {
        let cwd = std::env::current_dir().expect("cwd");
        let mut app = AppState::new(cwd);
        app.rename_rows.clear();
        app.submit_simulated(true);

        assert!(!app.toasts.is_empty());
    }

    #[test]
    fn select_all_visible_files_marks_visible_file_nodes_selected() {
        let cwd = std::env::current_dir().expect("cwd");
        let mut app = AppState::new(cwd);

        let visible_file_nodes: Vec<usize> = app
            .scope_files_rows()
            .into_iter()
            .map(|row| row.node_id)
            .filter(|node_id| !app.tree.nodes[*node_id].is_dir)
            .collect();

        app.select_all_visible_files();

        for node_id in &visible_file_nodes {
            assert!(app.tree.nodes[*node_id].selected);
            assert!(app.selected_by_tree.contains(&app.tree.nodes[*node_id].path));
        }
    }

    #[test]
    fn select_all_visible_files_toggles_to_deselect_when_all_selected() {
        let cwd = std::env::current_dir().expect("cwd");
        let mut app = AppState::new(cwd);

        let visible_file_nodes: Vec<usize> = app
            .scope_files_rows()
            .into_iter()
            .map(|row| row.node_id)
            .filter(|node_id| !app.tree.nodes[*node_id].is_dir)
            .collect();

        app.select_all_visible_files();
        app.select_all_visible_files();

        for node_id in &visible_file_nodes {
            assert!(!app.tree.nodes[*node_id].selected);
            assert!(!app.selected_by_tree.contains(&app.tree.nodes[*node_id].path));
        }
    }

    #[test]
    fn select_all_visible_files_only_affects_currently_visible_rows() {
        let unique = format!(
            "irium-select-all-visible-only-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        let nested_dir = root.join("nested").join("deeper");
        std::fs::create_dir_all(&nested_dir).expect("create nested dirs");
        let nested_file = nested_dir.join("inside.txt");
        std::fs::write(&nested_file, b"x").expect("write nested file");
        let top_file = root.join("top.txt");
        std::fs::write(&top_file, b"y").expect("write top file");

        let mut app = AppState::new(root.clone());
        app.select_all_visible_files();

        assert!(app.selected_by_tree.contains(&top_file));
        assert!(!app.selected_by_tree.contains(&nested_file));

        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn select_all_visible_files_respects_selected_categories_only_filter() {
        let unique = format!(
            "irium-select-all-filtered-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&root).expect("create root");
        let pdf_file = root.join("doc.pdf");
        let audio_file = root.join("song.mp3");
        std::fs::write(&pdf_file, b"x").expect("write pdf file");
        std::fs::write(&audio_file, b"y").expect("write audio file");

        let mut app = AppState::new(root.clone());
        app.selected_extensions.insert("pdf".to_string());
        app.show_selected_categories_only = true;
        app.normalize_scope_files_cursor();
        app.select_all_visible_files();

        assert!(app.selected_by_tree.contains(&pdf_file));
        assert!(!app.selected_by_tree.contains(&audio_file));

        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn scope_files_rows_empty_when_filtered_and_no_categories_selected() {
        let cwd = std::env::current_dir().expect("cwd");
        let mut app = AppState::new(cwd);
        app.show_selected_categories_only = true;
        app.selected_extensions.clear();

        assert!(app.scope_files_rows().is_empty());
    }

    #[test]
    fn scope_files_rows_hides_folders_without_matching_descendants_when_filtered() {
        let unique = format!(
            "irium-folder-filter-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        let pdf_dir = root.join("pdfs");
        let audio_dir = root.join("audio");
        std::fs::create_dir_all(&pdf_dir).expect("create pdf dir");
        std::fs::create_dir_all(&audio_dir).expect("create audio dir");
        std::fs::write(pdf_dir.join("a.pdf"), b"x").expect("write pdf file");
        std::fs::write(audio_dir.join("a.mp3"), b"y").expect("write audio file");

        let mut app = AppState::new(root.clone());
        app.show_selected_categories_only = true;
        app.selected_extensions.insert("pdf".to_string());
        app.normalize_scope_files_cursor();

        let visible_names: Vec<String> = app
            .scope_files_rows()
            .into_iter()
            .map(|row| app.tree.nodes[row.node_id].name.clone())
            .collect();
        assert!(visible_names.contains(&"pdfs".to_string()));
        assert!(!visible_names.contains(&"audio".to_string()));

        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn directory_renders_selected_when_all_children_selected() {
        let cwd = std::env::current_dir().expect("cwd");
        let mut app = AppState::new(cwd);
        app.tree = FileTree {
            nodes: vec![
                FileNode {
                    parent: None,
                    path: PathBuf::from("."),
                    name: ".".to_string(),
                    is_dir: true,
                    children: vec![1],
                    children_loaded: true,
                    expanded: true,
                    selected: false,
                    unreadable: false,
                },
                FileNode {
                    parent: Some(0),
                    path: PathBuf::from("./docs"),
                    name: "docs".to_string(),
                    is_dir: true,
                    children: vec![2, 3],
                    children_loaded: true,
                    expanded: true,
                    selected: false,
                    unreadable: false,
                },
                FileNode {
                    parent: Some(1),
                    path: PathBuf::from("./docs/a.txt"),
                    name: "a.txt".to_string(),
                    is_dir: false,
                    children: vec![],
                    children_loaded: false,
                    expanded: false,
                    selected: true,
                    unreadable: false,
                },
                FileNode {
                    parent: Some(1),
                    path: PathBuf::from("./docs/b.txt"),
                    name: "b.txt".to_string(),
                    is_dir: false,
                    children: vec![],
                    children_loaded: false,
                    expanded: false,
                    selected: true,
                    unreadable: false,
                },
            ],
            root: 0,
            cursor: 0,
            scroll: 0,
        };

        assert!(app.node_selected_for_display(1));
    }

    #[test]
    fn directory_renders_unselected_when_any_child_unselected() {
        let cwd = std::env::current_dir().expect("cwd");
        let mut app = AppState::new(cwd);
        app.tree = FileTree {
            nodes: vec![
                FileNode {
                    parent: None,
                    path: PathBuf::from("."),
                    name: ".".to_string(),
                    is_dir: true,
                    children: vec![1],
                    children_loaded: true,
                    expanded: true,
                    selected: false,
                    unreadable: false,
                },
                FileNode {
                    parent: Some(0),
                    path: PathBuf::from("./docs"),
                    name: "docs".to_string(),
                    is_dir: true,
                    children: vec![2, 3],
                    children_loaded: true,
                    expanded: true,
                    selected: false,
                    unreadable: false,
                },
                FileNode {
                    parent: Some(1),
                    path: PathBuf::from("./docs/a.txt"),
                    name: "a.txt".to_string(),
                    is_dir: false,
                    children: vec![],
                    children_loaded: false,
                    expanded: false,
                    selected: true,
                    unreadable: false,
                },
                FileNode {
                    parent: Some(1),
                    path: PathBuf::from("./docs/b.txt"),
                    name: "b.txt".to_string(),
                    is_dir: false,
                    children: vec![],
                    children_loaded: false,
                    expanded: false,
                    selected: false,
                    unreadable: false,
                },
            ],
            root: 0,
            cursor: 0,
            scroll: 0,
        };

        assert!(!app.node_selected_for_display(1));
    }
}
