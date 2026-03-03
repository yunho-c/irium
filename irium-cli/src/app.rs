use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::mpsc::TryRecvError,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use arboard::Clipboard;
use ratatui::{layout::Rect, text::Line};
use ratatui_image::picker::Picker;
use tachyonfx::{Interpolation, fx, pattern::SweepPattern};

use crate::{
    ai::{
        self,
        worker::{AiWorkerCommand, AiWorkerEvent, WorkerProgressStage},
    },
    config::{
        AppConfig, load_app_config, load_prompt_history, save_app_config, save_prompt_history,
    },
    fs_scan::{self, FsEntry},
    mock,
    model::{
        AiSettingsField, AiSettingsState, AppState, CategoryFilter, FileNode, FileTree,
        FilesPreviewStatus, FocusPane, InputMode, MarketplacePreset, NamingAiStatus, NamingTab,
        PresetState, RenameRow, ScopeFilesScrollbarGeometry, ScopeTab, SessionUndoEntry,
        SizeConstraint, Stage, StyleOptions, SuggestionSet, TimeConstraint, Toast, ToastLevel,
        VisibleNode,
    },
    preview::{
        self, PreviewKind,
        worker::{PreviewWorkerCommand, PreviewWorkerEvent},
    },
};

fn build_ai_settings_from_config(config: &AppConfig) -> AiSettingsState {
    let mut settings = AiSettingsState {
        api_key: config.openrouter_api_key.clone(),
        selected_model: config.openrouter_model.clone(),
        ..Default::default()
    };
    settings.api_key_input = settings.api_key.clone().unwrap_or_default();
    settings.manual_model_input = settings.selected_model.clone().unwrap_or_default();
    settings
}

const MAX_PROMPT_HISTORY_ENTRIES: usize = 200;

impl AppState {
    pub fn new(cwd: PathBuf) -> Self {
        let tree = FileTree::new(cwd.clone());
        let loaded_config = load_app_config().unwrap_or_default();
        let (mut prompt_history, prompt_history_error) = match load_prompt_history() {
            Ok(entries) => (entries, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        if prompt_history.len() > MAX_PROMPT_HISTORY_ENTRIES {
            let drop_count = prompt_history.len() - MAX_PROMPT_HISTORY_ENTRIES;
            prompt_history.drain(0..drop_count);
        }
        let mut ai_settings = build_ai_settings_from_config(&loaded_config);
        let (ai_worker_tx, ai_worker_rx) = match ai::worker::spawn_worker() {
            Ok((tx, rx)) => (Some(tx), Some(rx)),
            Err(_) => (None, None),
        };
        let (files_preview_worker_tx, files_preview_worker_rx) =
            match preview::worker::spawn_worker() {
                Ok((tx, rx)) => (Some(tx), Some(rx)),
                Err(_) => (None, None),
            };
        if ai_settings.selected_model.is_none() {
            ai_settings.manual_model_input = "google/gemini-2.0-flash-001".to_string();
        }
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
            files_preview_visible: false,
            files_preview_status: FilesPreviewStatus::Hidden,
            files_preview_target: None,
            files_preview_error: None,
            files_preview_source_meta: None,
            files_preview_protocol: None,
            files_preview_picker: None,
            files_preview_next_request_id: 1,
            files_preview_active_request_id: None,
            files_preview_worker_tx,
            files_preview_worker_rx,
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
            suggestions: mock::default_suggestion_labels(),
            suggestion_cursor: 0,
            suggestion_set: None,
            style: StyleOptions::default(),
            style_cursor: 0,
            command_input: String::new(),
            command_history_nav: None,
            command_history_draft: None,
            override_input: String::new(),
            editing_row: None,
            override_vocab: Default::default(),
            toasts: vec![Toast {
                level: ToastLevel::Info,
                message: "Welcome to irm. Scope files, shape names, and simulate apply."
                    .to_string(),
                ttl_ticks: 55,
            }],
            logs: Vec::new(),
            show_log_overlay: false,
            log_scroll: 0,
            log_view_height: 0,
            log_col_scroll: 0,
            log_view_width: 0,
            prompt_history,
            show_prompt_history_overlay: false,
            prompt_history_cursor: 0,
            prompt_history_scroll: 0,
            prompt_history_view_height: 0,
            undo_history: Vec::new(),
            apply_cursor: 0,
            history_cursor: 0,
            click_regions: Default::default(),
            ticks: 0,
            help_lines: vec![
                Line::from("Global: q quit | [ / ] stage | Tab focus | ? help"),
                Line::from("Global: L logs overlay"),
                Line::from(
                    "Files: arrows move | Space select | A select-all | p settings | f category-filter | v preview",
                ),
                Line::from(
                    "Naming: r refresh AI | m suggestions settings | p prompt history | 1-3 row option | / prompt | v preview",
                ),
                Line::from("Apply: Enter submit+exit (simulated) | Ctrl+Enter submit+stay"),
            ],
            selected_by_tree: HashSet::new(),
            filter_cache: HashMap::new(),
            category_match_dirs: HashSet::new(),
            scope_files_focus_nodes: HashSet::new(),
            scope_files_drag: None,
            scope_files_scrollbar: None,
            title_startup_fx: Some(
                fx::hsl_shift(
                    Some([120.0, 25.0, 25.0]),
                    Some([-40.0, -50.0, -50.0]),
                    (1000, Interpolation::Linear),
                )
                .with_pattern(SweepPattern::left_to_right(80)),
            ),
            title_fx_last_frame: Some(Instant::now()),
            ai_settings,
            naming_ai_status: NamingAiStatus::Idle,
            naming_ai_error: None,
            ai_next_request_id: 1,
            ai_active_request_id: None,
            ai_worker_tx,
            ai_worker_rx,
        };

        app.sync_focus_manager();
        app.reload_tree();
        app.sync_rename_rows();
        app.append_log("INFO", "App initialized");
        if let Some(error) = prompt_history_error {
            app.append_log("WARN", &format!("Prompt history load failed: {error}"));
        }
        app.naming_ai_status = if app.ai_settings_has_minimum_config() {
            NamingAiStatus::Idle
        } else {
            NamingAiStatus::NeedsConfig
        };
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
            self.trigger_naming_suggestions_refresh_if_configured();
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
        self.poll_ai_events();
        self.poll_files_preview_events();
        for toast in &mut self.toasts {
            toast.ttl_ticks = toast.ttl_ticks.saturating_sub(1);
        }
        self.toasts.retain(|toast| toast.ttl_ticks > 0);
    }

    pub fn shutdown_ai_worker(&mut self) {
        if let Some(tx) = &self.ai_worker_tx {
            let _ = tx.send(AiWorkerCommand::Shutdown);
        }
    }

    pub fn shutdown_preview_worker(&mut self) {
        if let Some(tx) = &self.files_preview_worker_tx {
            let _ = tx.send(PreviewWorkerCommand::Shutdown);
        }
    }

    pub fn toggle_log_overlay(&mut self) {
        self.show_log_overlay = !self.show_log_overlay;
        if self.show_log_overlay {
            self.log_col_scroll = 0;
            self.scroll_logs_to_bottom();
        }
    }

    pub fn click_target_at(&self, col: u16, row: u16) -> Option<crate::model::ClickTarget> {
        self.click_regions.handle_click(col, row).cloned()
    }

    pub fn click_target_at_topmost(&self, col: u16, row: u16) -> Option<crate::model::ClickTarget> {
        self.click_regions
            .regions()
            .iter()
            .rev()
            .find(|region| region.contains(col, row))
            .map(|region| region.data.clone())
    }

    pub fn update_log_viewport(&mut self, height: usize, width: usize) {
        self.log_view_height = height;
        self.log_view_width = width;
        self.clamp_log_scroll_bounds();
    }

    pub fn scroll_log_up(&mut self) {
        let max_start = self.max_log_row_start();
        self.log_scroll = self.log_scroll.min(max_start).saturating_sub(1);
    }

    pub fn scroll_log_down(&mut self) {
        let max_start = self.max_log_row_start();
        self.log_scroll = self.log_scroll.saturating_add(1).min(max_start);
    }

    pub fn scroll_log_left(&mut self) {
        self.log_col_scroll = self.log_col_scroll.saturating_sub(1);
    }

    pub fn scroll_log_right(&mut self) {
        let max_start = self.max_log_col_start();
        self.log_col_scroll = self.log_col_scroll.saturating_add(1).min(max_start);
    }

    pub fn toggle_prompt_history_overlay(&mut self) {
        self.show_prompt_history_overlay = !self.show_prompt_history_overlay;
        if self.show_prompt_history_overlay {
            self.mode = InputMode::Normal;
            self.prompt_history_cursor = 0;
            self.prompt_history_scroll = 0;
        }
    }

    pub fn prompt_history_move_up(&mut self) {
        self.prompt_history_cursor = self.prompt_history_cursor.saturating_sub(1);
        self.clamp_prompt_history_scroll();
    }

    pub fn prompt_history_move_down(&mut self) {
        if self.prompt_history.is_empty() {
            self.prompt_history_cursor = 0;
            self.prompt_history_scroll = 0;
            return;
        }
        self.prompt_history_cursor = self
            .prompt_history_cursor
            .saturating_add(1)
            .min(self.prompt_history.len() - 1);
        self.clamp_prompt_history_scroll();
    }

    pub fn prompt_history_set_view_height(&mut self, height: usize) {
        self.prompt_history_view_height = height.max(1);
        self.clamp_prompt_history_scroll();
    }

    pub fn reset_command_history_nav(&mut self) {
        self.command_history_nav = None;
        self.command_history_draft = None;
    }

    pub fn command_history_prev(&mut self) {
        if self.prompt_history.is_empty() {
            return;
        }
        let max_offset = self.prompt_history.len() - 1;
        let next_offset = match self.command_history_nav {
            None => {
                self.command_history_draft = Some(self.command_input.clone());
                0
            }
            Some(current) => current.saturating_add(1).min(max_offset),
        };
        self.command_history_nav = Some(next_offset);
        if let Some(prompt) = self.prompt_history_by_offset(next_offset) {
            self.command_input = prompt;
        }
    }

    pub fn command_history_next(&mut self) {
        let Some(current) = self.command_history_nav else {
            return;
        };

        if current == 0 {
            self.command_history_nav = None;
            self.command_input = self.command_history_draft.take().unwrap_or_default();
            return;
        }

        let next_offset = current - 1;
        self.command_history_nav = Some(next_offset);
        if let Some(prompt) = self.prompt_history_by_offset(next_offset) {
            self.command_input = prompt;
        }
    }

    pub fn select_prompt_history_entry(&mut self) {
        let Some(prompt) = self.prompt_history_entry_at_cursor() else {
            self.push_toast(ToastLevel::Warning, "Prompt history is empty");
            return;
        };
        self.command_input = prompt;
        self.show_prompt_history_overlay = false;
        self.set_focus(FocusPane::NamingCommand);
        self.mode = InputMode::Command;
        self.reset_command_history_nav();
        self.push_toast(ToastLevel::Info, "Loaded prompt from history");
    }

    pub fn visible_prompt_history_items(&self) -> Vec<(usize, String)> {
        let len = self.prompt_history.len();
        let max_visible = self.prompt_history_view_height.max(1);
        let start = self
            .prompt_history_scroll
            .min(len.saturating_sub(max_visible));
        let end = (start + max_visible).min(len);
        let mut out = Vec::with_capacity(end.saturating_sub(start));
        for display_idx in start..end {
            let actual_idx = len.saturating_sub(1).saturating_sub(display_idx);
            if let Some(item) = self.prompt_history.get(actual_idx) {
                out.push((display_idx, item.clone()));
            }
        }
        out
    }

    fn prompt_history_entry_at_cursor(&self) -> Option<String> {
        let len = self.prompt_history.len();
        if len == 0 {
            return None;
        }
        let actual_idx = len
            .saturating_sub(1)
            .saturating_sub(self.prompt_history_cursor);
        self.prompt_history.get(actual_idx).cloned()
    }

    fn prompt_history_by_offset(&self, offset_from_latest: usize) -> Option<String> {
        let len = self.prompt_history.len();
        if len == 0 {
            return None;
        }
        let actual_idx = len.saturating_sub(1).saturating_sub(offset_from_latest);
        self.prompt_history.get(actual_idx).cloned()
    }

    fn clamp_prompt_history_scroll(&mut self) {
        let len = self.prompt_history.len();
        if len == 0 {
            self.prompt_history_cursor = 0;
            self.prompt_history_scroll = 0;
            return;
        }
        let max_cursor = len - 1;
        self.prompt_history_cursor = self.prompt_history_cursor.min(max_cursor);
        let visible = self.prompt_history_view_height.max(1);
        let max_start = len.saturating_sub(visible);
        if self.prompt_history_cursor < self.prompt_history_scroll {
            self.prompt_history_scroll = self.prompt_history_cursor;
        } else if self.prompt_history_cursor >= self.prompt_history_scroll + visible {
            self.prompt_history_scroll = self.prompt_history_cursor + 1 - visible;
        }
        self.prompt_history_scroll = self.prompt_history_scroll.min(max_start);
    }

    fn push_prompt_history(&mut self, prompt: &str) {
        let prompt = prompt.trim();
        if prompt.is_empty() {
            return;
        }
        self.prompt_history.retain(|entry| entry != prompt);
        self.prompt_history.push(prompt.to_string());
        if self.prompt_history.len() > MAX_PROMPT_HISTORY_ENTRIES {
            let drop_count = self.prompt_history.len() - MAX_PROMPT_HISTORY_ENTRIES;
            self.prompt_history.drain(0..drop_count);
        }
        if let Err(error) = save_prompt_history(&self.prompt_history) {
            self.append_log("WARN", &format!("Prompt history save failed: {error}"));
        }
        self.clamp_prompt_history_scroll();
    }

    pub fn copy_log_line(&mut self, index: usize) {
        let Some(line) = self.logs.get(index).cloned() else {
            return;
        };

        match Clipboard::new().and_then(|mut clipboard| clipboard.set_text(line)) {
            Ok(()) => self.push_toast(ToastLevel::Success, "Copied log line"),
            Err(error) => self.push_toast(
                ToastLevel::Error,
                format!("Failed to copy log line: {error}"),
            ),
        }
    }

    pub fn push_toast(&mut self, level: ToastLevel, message: impl Into<String>) {
        let message = message.into();
        let level_label = match level {
            ToastLevel::Info => "INFO",
            ToastLevel::Success => "SUCCESS",
            ToastLevel::Warning => "WARN",
            ToastLevel::Error => "ERROR",
        };
        self.append_log(level_label, &message);
        self.toasts.push(Toast {
            level,
            message,
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
        rows.retain(|row| self.node_matches_selected_categories(row.node_id));
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
        let rows = self.scope_files_rows();
        let len = rows.len();
        let visible_nodes: HashSet<usize> = rows.into_iter().map(|row| row.node_id).collect();
        self.scope_files_focus_nodes
            .retain(|node_id| visible_nodes.contains(node_id));
        if len == 0 {
            self.tree.cursor = 0;
            self.tree.scroll = 0;
            self.scope_files_scrollbar = None;
            return;
        }
        self.tree.cursor = self.tree.cursor.min(len - 1);
        self.tree.scroll = self.tree.scroll.min(self.tree.cursor);
    }

    pub fn clear_scope_files_scrollbar(&mut self) {
        self.scope_files_scrollbar = None;
    }

    pub fn set_scope_files_scrollbar_geometry(
        &mut self,
        track_area: Rect,
        content_len: usize,
        viewport_len: usize,
    ) {
        if track_area.width == 0 || track_area.height == 0 || viewport_len == 0 || content_len == 0
        {
            self.scope_files_scrollbar = None;
            return;
        }

        let viewport_len = viewport_len.min(content_len);
        if content_len <= viewport_len {
            self.scope_files_scrollbar = None;
            return;
        }

        let track_height = track_area.height as usize;
        let max_scroll = content_len.saturating_sub(viewport_len);
        self.tree.scroll = self.tree.scroll.min(max_scroll);

        let mut thumb_height = viewport_len
            .saturating_mul(track_height)
            .checked_div(content_len)
            .unwrap_or(1);
        thumb_height = thumb_height.max(1).min(track_height);
        let max_thumb_top = track_height.saturating_sub(thumb_height);
        let thumb_top = if max_scroll == 0 || max_thumb_top == 0 {
            0
        } else {
            (self
                .tree
                .scroll
                .saturating_mul(max_thumb_top)
                .saturating_add(max_scroll / 2))
                / max_scroll
        };

        self.scope_files_scrollbar = Some(ScopeFilesScrollbarGeometry {
            track_area,
            thumb_area: Rect::new(
                track_area.x,
                track_area.y.saturating_add(thumb_top as u16),
                track_area.width,
                thumb_height as u16,
            ),
            content_len,
            viewport_len,
            max_scroll,
        });
    }

    fn apply_scope_files_scroll(&mut self, scroll: usize, viewport_len: usize) {
        let len = self.scope_files_rows_len();
        if len == 0 {
            self.tree.cursor = 0;
            self.tree.scroll = 0;
            return;
        }

        let viewport = viewport_len.max(1).min(len);
        let max_scroll = len.saturating_sub(viewport);
        self.tree.scroll = scroll.min(max_scroll);

        if self.tree.cursor < self.tree.scroll {
            self.tree.cursor = self.tree.scroll;
        }
        let visible_end = self.tree.scroll.saturating_add(viewport.saturating_sub(1));
        if self.tree.cursor > visible_end {
            self.tree.cursor = visible_end;
        }
        self.normalize_scope_files_cursor();
    }

    fn recompute_scope_files_scrollbar_geometry(&mut self) {
        let Some(geometry) = self.scope_files_scrollbar else {
            return;
        };
        self.set_scope_files_scrollbar_geometry(
            geometry.track_area,
            geometry.content_len,
            geometry.viewport_len,
        );
    }

    pub fn jump_scope_files_scrollbar_to_track_row(&mut self, row: u16) {
        let Some(geometry) = self.scope_files_scrollbar else {
            return;
        };

        let scroll = geometry.scroll_for_track_row_centered(row);
        self.apply_scope_files_scroll(scroll, geometry.viewport_len);
        self.recompute_scope_files_scrollbar_geometry();
    }

    pub fn drag_scope_files_scrollbar_thumb(&mut self, row: u16, grab_offset_rows: u16) {
        let Some(geometry) = self.scope_files_scrollbar else {
            return;
        };

        let top_row = row.saturating_sub(grab_offset_rows);
        let scroll = geometry.scroll_for_thumb_top_row(top_row);
        self.apply_scope_files_scroll(scroll, geometry.viewport_len);
        self.recompute_scope_files_scrollbar_geometry();
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

    pub fn toggle_files_preview(&mut self) {
        self.files_preview_visible = !self.files_preview_visible;
        if self.files_preview_visible {
            self.files_preview_status = FilesPreviewStatus::Empty;
            self.files_preview_target = None;
            self.files_preview_error = None;
            self.files_preview_source_meta = None;
            self.files_preview_protocol = None;
        } else {
            self.files_preview_status = FilesPreviewStatus::Hidden;
            self.files_preview_target = None;
            self.files_preview_error = None;
            self.files_preview_source_meta = None;
            self.files_preview_protocol = None;
            self.files_preview_active_request_id = None;
        }
    }

    pub fn sync_files_preview_with_focus(&mut self) {
        if !self.files_preview_visible {
            return;
        }

        if self.stage == Stage::Scope && self.scope_tab == ScopeTab::Files {
            let Some(node_id) = self.current_scope_file_node_id() else {
                self.files_preview_status = FilesPreviewStatus::Empty;
                self.files_preview_target = None;
                self.files_preview_error = None;
                self.files_preview_source_meta = None;
                self.files_preview_protocol = None;
                self.files_preview_active_request_id = None;
                return;
            };

            if self.tree.nodes[node_id].is_dir {
                self.files_preview_status = FilesPreviewStatus::Unsupported;
                self.files_preview_target = Some(self.tree.nodes[node_id].path.clone());
                self.files_preview_error = Some("Folder preview is not supported".to_string());
                self.files_preview_source_meta = Some(preview::PreviewSourceMeta {
                    kind: PreviewKind::Unsupported,
                    width: 0,
                    height: 0,
                    pdf_page: None,
                });
                self.files_preview_protocol = None;
                self.files_preview_active_request_id = None;
                return;
            }

            let path = self.tree.nodes[node_id].path.clone();
            if self.files_preview_target.as_ref() == Some(&path)
                && matches!(
                    self.files_preview_status,
                    FilesPreviewStatus::Loading
                        | FilesPreviewStatus::Ready
                        | FilesPreviewStatus::Unsupported
                        | FilesPreviewStatus::Error
                )
            {
                return;
            }

            self.request_files_preview_for_path(path);
            return;
        }

        if self.stage == Stage::Naming {
            let Some(row) = self.rename_rows.get(
                self.rename_cursor
                    .min(self.rename_rows.len().saturating_sub(1)),
            ) else {
                self.files_preview_status = FilesPreviewStatus::Empty;
                self.files_preview_target = None;
                self.files_preview_error = None;
                self.files_preview_source_meta = None;
                self.files_preview_protocol = None;
                self.files_preview_active_request_id = None;
                return;
            };
            let path = row.path.clone();
            if self.files_preview_target.as_ref() == Some(&path)
                && matches!(
                    self.files_preview_status,
                    FilesPreviewStatus::Loading
                        | FilesPreviewStatus::Ready
                        | FilesPreviewStatus::Unsupported
                        | FilesPreviewStatus::Error
                )
            {
                return;
            }
            self.request_files_preview_for_path(path);
            return;
        }

        {
            self.files_preview_status = FilesPreviewStatus::Empty;
            self.files_preview_target = None;
            self.files_preview_error = None;
            self.files_preview_source_meta = None;
            self.files_preview_protocol = None;
            self.files_preview_active_request_id = None;
        }
    }

    fn request_files_preview_for_path(&mut self, path: PathBuf) {
        let request_id = self.next_files_preview_request_id();
        self.files_preview_target = Some(path.clone());
        self.files_preview_active_request_id = Some(request_id);
        self.files_preview_status = FilesPreviewStatus::Loading;
        self.files_preview_error = None;
        self.files_preview_source_meta = None;
        self.files_preview_protocol = None;

        if let Some(tx) = &self.files_preview_worker_tx {
            let _ = tx.send(PreviewWorkerCommand::GeneratePreview { request_id, path });
        } else {
            self.files_preview_status = FilesPreviewStatus::Error;
            self.files_preview_error = Some("Preview worker is not available".to_string());
        }
    }

    fn next_files_preview_request_id(&mut self) -> u64 {
        let id = self.files_preview_next_request_id;
        self.files_preview_next_request_id = self.files_preview_next_request_id.saturating_add(1);
        id
    }

    fn node_matches_selected_categories(&self, node_id: usize) -> bool {
        let node = &self.tree.nodes[node_id];
        if node.is_dir {
            return self.category_match_dirs.contains(&node.path);
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
                    if self.ai_settings.popup_open
                        && self.ai_settings.active_field == AiSettingsField::ModelList
                    {
                        self.ai_settings.model_list_cursor =
                            self.ai_settings.model_list_cursor.saturating_sub(1);
                    } else {
                        self.suggestion_cursor = self.suggestion_cursor.saturating_sub(1);
                    }
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
                    if self.ai_settings.popup_open
                        && self.ai_settings.active_field == AiSettingsField::ModelList
                    {
                        let max = self.filtered_ai_models().len();
                        if max > 0 {
                            self.ai_settings.model_list_cursor =
                                (self.ai_settings.model_list_cursor + 1).min(max - 1);
                        }
                    } else if !self.suggestions.is_empty() {
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
        if !self.scope_files_focus_nodes.is_empty() {
            let focus_nodes: Vec<usize> = self.scope_files_focus_nodes.iter().copied().collect();
            let all_selected = focus_nodes
                .iter()
                .all(|node_id| self.node_selected_for_display(*node_id));
            for node_id in focus_nodes {
                if let Err(error) = self.set_node_selected(node_id, !all_selected) {
                    self.push_toast(
                        ToastLevel::Error,
                        format!("Selection update failed: {error}"),
                    );
                    break;
                }
            }
            self.sync_rename_rows();
            return;
        }

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

    pub fn scope_file_node_id_at(&self, row_index: usize) -> Option<usize> {
        let rows = self.scope_files_rows();
        if row_index >= rows.len() {
            return None;
        }
        Some(rows[row_index].node_id)
    }

    pub fn set_scope_file_selection_at(
        &mut self,
        row_index: usize,
        selected: bool,
    ) -> Result<(), String> {
        let Some(node_id) = self.scope_file_node_id_at(row_index) else {
            return Ok(());
        };
        self.set_node_selected(node_id, selected)
    }

    pub fn clear_scope_file_focus_nodes(&mut self) {
        self.scope_files_focus_nodes.clear();
    }

    pub fn add_scope_file_focus_node_at(&mut self, row_index: usize) {
        if let Some(node_id) = self.scope_file_node_id_at(row_index) {
            self.scope_files_focus_nodes.insert(node_id);
        }
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
        self.rebuild_category_match_dirs(&all_files);
        let mut include_paths = HashSet::new();

        for path in &self.selected_by_tree {
            include_paths.insert(path.clone());
        }

        self.filter_cache.clear();
        for file in &all_files {
            let include = self.matches_filters(file);
            self.filter_cache.insert(file.path.clone(), include);
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

            let proposed_name =
                self.propose_name(&entry.path, &stem, ext.as_deref(), override_name.as_deref());
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
        if let Some(set) = &mut self.suggestion_set {
            let paths: HashSet<PathBuf> = self
                .rename_rows
                .iter()
                .map(|row| row.path.clone())
                .collect();
            set.per_path_options.retain(|path, _| paths.contains(path));
            set.per_path_override_index
                .retain(|path, _| paths.contains(path));
        }
        if self.rename_rows.is_empty() {
            self.rename_cursor = 0;
        } else {
            self.rename_cursor = self.rename_cursor.min(self.rename_rows.len() - 1);
        }

        self.recompute_conflicts();
    }

    fn rebuild_category_match_dirs(&mut self, all_files: &[FsEntry]) {
        self.category_match_dirs.clear();
        if self.selected_extensions.is_empty() {
            return;
        }

        for entry in all_files {
            let ext = entry.extension.as_deref().unwrap_or_default();
            if !self.selected_extensions.contains(ext) {
                continue;
            }

            let mut parent = entry.path.parent();
            while let Some(dir) = parent {
                if !dir.starts_with(&self.cwd) {
                    break;
                }
                self.category_match_dirs.insert(dir.to_path_buf());
                if dir == self.cwd {
                    break;
                }
                parent = dir.parent();
            }
        }
    }

    fn propose_name(
        &self,
        path: &Path,
        stem: &str,
        ext: Option<&str>,
        override_name: Option<&str>,
    ) -> String {
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

        if let Some(base) = self.ai_candidate_for_path(path, ext) {
            return mock::format_name(&base, ext, &self.style);
        }

        mock::format_name(stem, ext, &self.style)
    }

    pub fn recompute_proposals(&mut self) {
        for idx in 0..self.rename_rows.len() {
            let path = self.rename_rows[idx].path.clone();
            let (stem, ext) = Self::file_name_parts(&path);
            let override_name = self.rename_rows[idx].override_name.clone();
            self.rename_rows[idx].proposed_name =
                self.propose_name(&path, &stem, ext.as_deref(), override_name.as_deref());
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
        if let Some(set) = &mut self.suggestion_set {
            set.global_option_index = self.suggestion_cursor.min(2);
        } else {
            return;
        }
        self.recompute_proposals();
        self.push_toast(
            ToastLevel::Success,
            format!(
                "Applied suggestion: {}",
                self.suggestions[self.suggestion_cursor]
            ),
        );
    }

    pub fn set_global_suggestion_option(&mut self, option_index: usize) {
        let option_index = option_index.min(2);
        self.suggestion_cursor = option_index;
        if let Some(set) = &mut self.suggestion_set {
            set.global_option_index = option_index;
            self.recompute_proposals();
        }
    }

    pub fn set_row_suggestion_option(&mut self, option_index: usize) {
        let option_index = option_index.min(2);
        if self.rename_rows.is_empty() {
            return;
        }
        let row = self.rename_cursor.min(self.rename_rows.len() - 1);
        if let Some(set) = &mut self.suggestion_set {
            let path = self.rename_rows[row].path.clone();
            set.per_path_override_index.insert(path, option_index);
            self.recompute_proposals();
        }
    }

    pub fn clear_row_suggestion_option(&mut self) {
        if self.rename_rows.is_empty() {
            return;
        }
        let row = self.rename_cursor.min(self.rename_rows.len() - 1);
        if let Some(set) = &mut self.suggestion_set {
            let path = self.rename_rows[row].path.clone();
            set.per_path_override_index.remove(&path);
            self.recompute_proposals();
        }
    }

    pub fn toggle_naming_suggestions_settings(&mut self) {
        self.ai_settings.popup_open = !self.ai_settings.popup_open;
        if !self.ai_settings.popup_open {
            self.mode = InputMode::Normal;
        }
    }

    pub fn focus_ai_settings_field(&mut self, field: AiSettingsField) {
        self.ai_settings.active_field = field;
        self.mode = match field {
            AiSettingsField::ApiKey => InputMode::EditingAiApiKey,
            AiSettingsField::ModelSearch => InputMode::EditingAiModelSearch,
            AiSettingsField::ManualModel => InputMode::EditingAiManualModel,
            AiSettingsField::ModelList | AiSettingsField::Save | AiSettingsField::Discover => {
                InputMode::Normal
            }
        };
    }

    pub fn ai_settings_next_field(&mut self) {
        use AiSettingsField as F;
        let next = match self.ai_settings.active_field {
            F::ApiKey => F::ModelSearch,
            F::ModelSearch => F::ModelList,
            F::ModelList => F::ManualModel,
            F::ManualModel => F::Discover,
            F::Discover => F::Save,
            F::Save => F::ApiKey,
        };
        self.focus_ai_settings_field(next);
    }

    pub fn ai_settings_prev_field(&mut self) {
        use AiSettingsField as F;
        let prev = match self.ai_settings.active_field {
            F::ApiKey => F::Save,
            F::ModelSearch => F::ApiKey,
            F::ModelList => F::ModelSearch,
            F::ManualModel => F::ModelList,
            F::Discover => F::ManualModel,
            F::Save => F::Discover,
        };
        self.focus_ai_settings_field(prev);
    }

    pub fn save_ai_settings_to_disk(&mut self) {
        if !self.ai_settings.api_key_input.trim().is_empty() {
            self.ai_settings.api_key = Some(self.ai_settings.api_key_input.trim().to_string());
        }
        if !self.ai_settings.manual_model_input.trim().is_empty() {
            self.ai_settings.selected_model =
                Some(self.ai_settings.manual_model_input.trim().to_string());
        }

        let config = AppConfig {
            openrouter_api_key: self.ai_settings.api_key.clone(),
            openrouter_model: self.ai_settings.selected_model.clone(),
        };
        match save_app_config(&config) {
            Ok(path) => {
                self.push_toast(
                    ToastLevel::Success,
                    format!("Saved AI settings to {}", path.display()),
                );
            }
            Err(error) => {
                self.push_toast(ToastLevel::Error, error);
            }
        }
        self.naming_ai_status = if self.ai_settings_has_minimum_config() {
            NamingAiStatus::Idle
        } else {
            NamingAiStatus::NeedsConfig
        };
    }

    pub fn clear_ai_api_key(&mut self) {
        self.ai_settings.api_key = None;
        self.ai_settings.api_key_input.clear();
        self.naming_ai_status = NamingAiStatus::NeedsConfig;
        self.push_toast(ToastLevel::Info, "Cleared OpenRouter API key");
    }

    pub fn discover_openrouter_models(&mut self) {
        let request_id = self.next_ai_request_id();
        self.ai_active_request_id = Some(request_id);
        self.naming_ai_status = NamingAiStatus::DiscoveringModels;
        self.naming_ai_error = None;
        if let Some(tx) = &self.ai_worker_tx {
            let _ = tx.send(AiWorkerCommand::DiscoverModels {
                request_id,
                api_key: self.ai_settings.api_key.clone(),
            });
        } else {
            self.naming_ai_status = NamingAiStatus::Error;
            self.naming_ai_error = Some("AI worker is not available".to_string());
        }
    }

    pub fn trigger_naming_suggestions_refresh_if_configured(&mut self) {
        if self.ai_settings_has_minimum_config() {
            self.trigger_naming_suggestions_refresh_with_prompt(None);
        } else {
            self.naming_ai_status = NamingAiStatus::NeedsConfig;
            self.naming_ai_error = None;
        }
    }

    pub fn trigger_naming_suggestions_refresh(&mut self) {
        self.trigger_naming_suggestions_refresh_with_prompt(None);
    }

    pub fn trigger_naming_suggestions_refresh_with_prompt(&mut self, user_prompt: Option<String>) {
        if !self.ai_settings_has_minimum_config() {
            self.naming_ai_status = NamingAiStatus::NeedsConfig;
            self.push_toast(
                ToastLevel::Warning,
                "Open Suggestions settings (m) and configure OpenRouter key + model",
            );
            return;
        }
        if self.rename_rows.is_empty() {
            self.naming_ai_status = NamingAiStatus::Idle;
            return;
        }

        let Some(api_key) = self.ai_settings.api_key.clone() else {
            self.naming_ai_status = NamingAiStatus::NeedsConfig;
            return;
        };
        let Some(model_id) = self.ai_settings.selected_model.clone() else {
            self.naming_ai_status = NamingAiStatus::NeedsConfig;
            return;
        };

        let request_id = self.next_ai_request_id();
        self.ai_active_request_id = Some(request_id);
        self.naming_ai_error = None;
        self.naming_ai_status = NamingAiStatus::AnalyzingFiles;

        let paths = self
            .rename_rows
            .iter()
            .map(|row| row.path.clone())
            .collect::<Vec<_>>();
        if let Some(tx) = &self.ai_worker_tx {
            let _ = tx.send(AiWorkerCommand::GenerateSuggestions {
                request_id,
                api_key,
                model_id,
                paths,
                user_prompt,
            });
        } else {
            self.naming_ai_status = NamingAiStatus::Error;
            self.naming_ai_error = Some("AI worker is not available".to_string());
        }
    }

    fn poll_ai_events(&mut self) {
        loop {
            let next = self.ai_worker_rx.as_ref().map(|rx| rx.try_recv());
            let Some(result) = next else {
                return;
            };
            match result {
                Ok(event) => self.handle_ai_event(event),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.naming_ai_status = NamingAiStatus::Error;
                    self.naming_ai_error = Some("AI worker disconnected".to_string());
                    break;
                }
            }
        }
    }

    fn poll_files_preview_events(&mut self) {
        loop {
            let next = self
                .files_preview_worker_rx
                .as_ref()
                .map(|rx| rx.try_recv());
            let Some(result) = next else {
                return;
            };
            match result {
                Ok(event) => self.handle_files_preview_event(event),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if self.files_preview_visible {
                        self.files_preview_status = FilesPreviewStatus::Error;
                        self.files_preview_error = Some("Preview worker disconnected".to_string());
                    }
                    break;
                }
            }
        }
    }

    fn handle_files_preview_event(&mut self, event: PreviewWorkerEvent) {
        match event {
            PreviewWorkerEvent::PreviewReady {
                request_id,
                path,
                image,
                meta,
            } => {
                if Some(request_id) != self.files_preview_active_request_id {
                    return;
                }
                if self.files_preview_target.as_ref() != Some(&path) {
                    return;
                }

                self.ensure_files_preview_picker();
                let Some(picker) = self.files_preview_picker.as_ref() else {
                    self.files_preview_status = FilesPreviewStatus::Error;
                    self.files_preview_error =
                        Some("Could not initialize terminal image preview".to_string());
                    return;
                };

                self.files_preview_protocol = Some(picker.new_resize_protocol(image));
                self.files_preview_source_meta = Some(meta);
                self.files_preview_status = FilesPreviewStatus::Ready;
                self.files_preview_error = None;
            }
            PreviewWorkerEvent::PreviewUnsupported {
                request_id,
                path,
                reason,
            } => {
                if Some(request_id) != self.files_preview_active_request_id {
                    return;
                }
                if self.files_preview_target.as_ref() != Some(&path) {
                    return;
                }
                self.files_preview_protocol = None;
                self.files_preview_source_meta = Some(preview::PreviewSourceMeta {
                    kind: PreviewKind::Unsupported,
                    width: 0,
                    height: 0,
                    pdf_page: None,
                });
                self.files_preview_status = FilesPreviewStatus::Unsupported;
                self.files_preview_error = Some(reason);
            }
            PreviewWorkerEvent::PreviewError {
                request_id,
                path,
                message,
            } => {
                if Some(request_id) != self.files_preview_active_request_id {
                    return;
                }
                if self.files_preview_target.as_ref() != Some(&path) {
                    return;
                }
                self.append_log(
                    "WARN",
                    &format!("Preview failed for {}: {message}", path.display()),
                );
                self.files_preview_protocol = None;
                self.files_preview_source_meta = None;
                self.files_preview_status = FilesPreviewStatus::Error;
                self.files_preview_error = Some(message);
            }
        }
    }

    fn ensure_files_preview_picker(&mut self) {
        if self.files_preview_picker.is_some() {
            return;
        }

        let picker = match Picker::from_query_stdio() {
            Ok(picker) => picker,
            Err(error) => {
                self.append_log(
                    "WARN",
                    &format!("Preview protocol query failed; falling back to halfblocks: {error}"),
                );
                Picker::halfblocks()
            }
        };
        self.files_preview_picker = Some(picker);
    }

    fn handle_ai_event(&mut self, event: AiWorkerEvent) {
        match event {
            AiWorkerEvent::ProgressUpdate { request_id, stage } => {
                if Some(request_id) != self.ai_active_request_id {
                    return;
                }
                self.naming_ai_status = match stage {
                    WorkerProgressStage::DiscoveringModels => NamingAiStatus::DiscoveringModels,
                    WorkerProgressStage::AnalyzingFiles => NamingAiStatus::AnalyzingFiles,
                    WorkerProgressStage::Generating => NamingAiStatus::Generating,
                };
            }
            AiWorkerEvent::ModelsDiscovered { request_id, models } => {
                if Some(request_id) != self.ai_active_request_id {
                    return;
                }
                self.ai_settings.discovered_models = models;
                self.ai_settings.model_list_cursor = 0;
                self.ai_settings.discovery_error = None;
                self.naming_ai_status = NamingAiStatus::Ready;
                self.naming_ai_error = None;
                if self.ai_settings.selected_model.is_none()
                    && let Some(first) = self.ai_settings.discovered_models.first()
                {
                    self.ai_settings.selected_model = Some(first.id.clone());
                    self.ai_settings.manual_model_input = first.id.clone();
                }
            }
            AiWorkerEvent::SuggestionsReady {
                request_id,
                source_model,
                per_path_options,
                warnings,
                skipped_files,
            } => {
                if Some(request_id) != self.ai_active_request_id {
                    return;
                }
                let mut set = SuggestionSet {
                    request_id,
                    global_option_index: self.suggestion_cursor.min(2),
                    per_path_options,
                    per_path_override_index: self
                        .suggestion_set
                        .as_ref()
                        .map(|s| s.per_path_override_index.clone())
                        .unwrap_or_default(),
                    source_model,
                };
                set.per_path_override_index
                    .retain(|path, _| set.per_path_options.contains_key(path));
                self.suggestion_set = Some(set);
                self.naming_ai_status = NamingAiStatus::Ready;
                self.naming_ai_error = None;
                self.recompute_proposals();
                if skipped_files > 0 {
                    self.push_toast(
                        ToastLevel::Info,
                        format!(
                            "AI analyzed first {} files ({} skipped by cap)",
                            ai::MAX_FILES_PER_RUN,
                            skipped_files
                        ),
                    );
                }
                if !warnings.is_empty() {
                    self.push_toast(
                        ToastLevel::Warning,
                        format!("AI finished with {} warning(s)", warnings.len()),
                    );
                    let total = warnings.len();
                    for (idx, warning) in warnings.into_iter().enumerate() {
                        let one_line = warning.replace('\n', " ");
                        self.append_log(
                            "WARN",
                            &format!("AI warning {}/{}: {}", idx + 1, total, one_line),
                        );
                    }
                }
            }
            AiWorkerEvent::AiError {
                request_id,
                message,
            } => {
                if request_id != 0 && Some(request_id) != self.ai_active_request_id {
                    return;
                }
                self.naming_ai_status = NamingAiStatus::Error;
                self.naming_ai_error = Some(message.clone());
                self.push_toast(ToastLevel::Error, message);
            }
        }
    }

    fn ai_settings_has_minimum_config(&self) -> bool {
        self.ai_settings
            .api_key
            .as_ref()
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
            && self
                .ai_settings
                .selected_model
                .as_ref()
                .map(|v| !v.trim().is_empty())
                .unwrap_or(false)
    }

    fn next_ai_request_id(&mut self) -> u64 {
        let id = self.ai_next_request_id;
        self.ai_next_request_id = self.ai_next_request_id.saturating_add(1);
        id
    }

    fn append_log(&mut self, level: &str, message: &str) {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.logs.push(format!("[{ts}] {level}: {message}"));
        if self.logs.len() > 1000 {
            let extra = self.logs.len() - 1000;
            self.logs.drain(0..extra);
        }
        self.clamp_log_scroll_bounds();
        self.scroll_logs_to_bottom();
    }

    fn scroll_logs_to_bottom(&mut self) {
        let max_start = self.max_log_row_start();
        self.log_scroll = max_start;
    }

    fn max_log_row_start(&self) -> usize {
        self.logs.len().saturating_sub(self.log_view_height.max(1))
    }

    fn max_log_col_start(&self) -> usize {
        self.logs
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0)
            .saturating_sub(self.log_view_width.max(1))
    }

    fn clamp_log_scroll_bounds(&mut self) {
        self.log_scroll = self.log_scroll.min(self.max_log_row_start());
        self.log_col_scroll = self.log_col_scroll.min(self.max_log_col_start());
    }

    pub fn filtered_ai_models(&self) -> Vec<(usize, crate::ai::ModelListItem)> {
        let query = self.ai_settings.model_search_query.trim().to_lowercase();
        self.ai_settings
            .discovered_models
            .iter()
            .enumerate()
            .filter(|(_, model)| {
                query.is_empty()
                    || model.id.to_lowercase().contains(&query)
                    || model.name.to_lowercase().contains(&query)
            })
            .map(|(idx, model)| (idx, model.clone()))
            .collect()
    }

    pub fn select_ai_model_by_filtered_index(&mut self, filtered_index: usize) {
        let filtered = self.filtered_ai_models();
        if filtered_index >= filtered.len() {
            return;
        }
        let model_id = filtered[filtered_index].1.id.clone();
        self.ai_settings.selected_model = Some(model_id.clone());
        self.ai_settings.manual_model_input = model_id;
        self.ai_settings.model_list_cursor = filtered_index;
    }

    fn ai_candidate_for_path(&self, path: &Path, ext: Option<&str>) -> Option<String> {
        let set = self.suggestion_set.as_ref()?;
        let options = set.per_path_options.get(path)?;
        let idx = set
            .per_path_override_index
            .get(path)
            .copied()
            .unwrap_or(set.global_option_index)
            .min(2);
        let mut candidate = options[idx].trim().to_string();
        if candidate.is_empty() {
            return None;
        }
        if let Some(ext) = ext {
            let suffix = format!(".{ext}");
            if candidate.to_lowercase().ends_with(&suffix) {
                let end = candidate.len().saturating_sub(suffix.len());
                candidate = candidate[..end].to_string();
            }
        }
        Some(candidate)
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
        let command = self.command_input.trim().to_string();
        self.reset_command_history_nav();
        self.command_input.clear();
        self.mode = InputMode::Normal;

        if command.is_empty() {
            self.push_toast(
                ToastLevel::Warning,
                "Prompt is empty. Enter a naming instruction and submit.",
            );
            return;
        }

        self.push_prompt_history(&command);
        self.trigger_naming_suggestions_refresh_with_prompt(Some(command));
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

    fn install_flat_file_tree(app: &mut AppState, file_count: usize) {
        let mut nodes = Vec::with_capacity(file_count + 1);
        let child_ids: Vec<usize> = (1..=file_count).collect();
        nodes.push(FileNode {
            parent: None,
            path: PathBuf::from("."),
            name: ".".to_string(),
            is_dir: true,
            children: child_ids.clone(),
            children_loaded: true,
            expanded: true,
            selected: false,
            unreadable: false,
        });
        for idx in 0..file_count {
            nodes.push(FileNode {
                parent: Some(0),
                path: PathBuf::from(format!("./file-{idx}.txt")),
                name: format!("file-{idx}.txt"),
                is_dir: false,
                children: Vec::new(),
                children_loaded: false,
                expanded: false,
                selected: false,
                unreadable: false,
            });
        }
        app.tree = FileTree {
            nodes,
            root: 0,
            cursor: 0,
            scroll: 0,
        };
        app.normalize_scope_files_cursor();
    }

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
    fn rename_rows_require_scope_selection_even_with_matching_filters() {
        let unique = format!(
            "irium-rename-selection-source-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&root).expect("create root");
        let pdf_file = root.join("doc.pdf");
        std::fs::write(&pdf_file, b"x").expect("write pdf");

        let mut app = AppState::new(root.clone());
        app.selected_extensions.insert("pdf".to_string());
        app.sync_rename_rows();
        assert!(app.rename_rows.is_empty());

        app.selected_by_tree.insert(pdf_file.clone());
        app.sync_rename_rows();
        assert_eq!(app.rename_rows.len(), 1);
        assert_eq!(app.rename_rows[0].path, pdf_file);

        std::fs::remove_dir_all(root).expect("cleanup");
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
            assert!(
                app.selected_by_tree
                    .contains(&app.tree.nodes[*node_id].path)
            );
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
            assert!(
                !app.selected_by_tree
                    .contains(&app.tree.nodes[*node_id].path)
            );
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
        app.sync_rename_rows();
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

    #[test]
    fn scope_files_scrollbar_geometry_tracks_top_middle_bottom() {
        let cwd = std::env::current_dir().expect("cwd");
        let mut app = AppState::new(cwd);
        install_flat_file_tree(&mut app, 100);
        let track = Rect::new(0, 5, 1, 20);

        app.tree.scroll = 0;
        app.set_scope_files_scrollbar_geometry(track, 100, 10);
        let top_y = app.scope_files_scrollbar.expect("geometry").thumb_area.y;

        app.tree.scroll = 45;
        app.set_scope_files_scrollbar_geometry(track, 100, 10);
        let mid_y = app.scope_files_scrollbar.expect("geometry").thumb_area.y;

        app.tree.scroll = 90;
        app.set_scope_files_scrollbar_geometry(track, 100, 10);
        let bottom = app.scope_files_scrollbar.expect("geometry");
        let bottom_y = bottom.thumb_area.y;

        assert_eq!(top_y, track.y);
        assert!(mid_y > top_y);
        assert!(bottom_y > mid_y);
        assert_eq!(
            bottom.thumb_area.y + bottom.thumb_area.height,
            track.y + track.height
        );
    }

    #[test]
    fn scope_files_scrollbar_track_mapping_is_clamped() {
        let cwd = std::env::current_dir().expect("cwd");
        let mut app = AppState::new(cwd);
        install_flat_file_tree(&mut app, 100);
        let track = Rect::new(0, 10, 1, 20);
        app.set_scope_files_scrollbar_geometry(track, 100, 10);
        let geometry = app.scope_files_scrollbar.expect("geometry");

        let above = geometry.scroll_for_track_row_centered(track.y.saturating_sub(5));
        let below = geometry.scroll_for_track_row_centered(track.y + track.height + 5);

        assert_eq!(above, 0);
        assert_eq!(below, geometry.max_scroll);
    }
}
