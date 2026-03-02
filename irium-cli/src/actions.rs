use std::collections::HashSet;

use crate::model::{
    AiSettingsField, AppState, ClickTarget, FilesDragMode, FilesDragState, FocusPane, InputMode,
    NamingTab, ScopeTab, Stage, ToastLevel,
};

#[derive(Debug, Clone)]
pub enum Action {
    Tick,
    Quit,
    ToggleLogOverlay,
    ScrollLogUp,
    ScrollLogDown,
    ScrollLogLeft,
    ScrollLogRight,
    ToggleHelp,
    NextStage,
    PrevStage,
    NextScopeTab,
    PrevScopeTab,
    ToggleNamingTab,
    NextFocus,
    PrevFocus,
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    ExpandAll,
    CollapseAll,
    ToggleSelect,
    SelectAllVisibleFiles,
    ToggleFilesSettingsPopup,
    ToggleShowSelectedCategoriesOnly,
    TriggerNamingSuggestionsRefresh,
    ToggleNamingSuggestionsSettings,
    DiscoverModels,
    SaveAiSettings,
    ClearAiApiKey,
    FocusAiSettingsField(AiSettingsField),
    SelectAiModelByFilteredIndex(usize),
    SetGlobalSuggestionOption(usize),
    SetRowSuggestionOption(usize),
    ClearRowSuggestionOption,
    NextAiSettingsField,
    PrevAiSettingsField,
    ToggleShowHidden,
    SavePreset(u8),
    LoadPreset(u8),
    ApplyMarketplace(usize),
    Submit { stay: bool },
    StartOverrideEdit,
    StartCommandInput,
    StartNewCategoryInput,
    StartGroupAssign,
    AssignGroup(u8),
    InputChar(char),
    Backspace,
    CommitInput,
    CancelInput,
    Autocomplete,
    MouseDown(u16, u16),
    MouseDrag(u16, u16),
    MouseUp(u16, u16),
    MouseScrollUp(u16, u16),
    MouseScrollDown(u16, u16),
    MouseScrollLeft(u16, u16),
    MouseScrollRight(u16, u16),
}

pub fn reduce(app: &mut AppState, action: Action) {
    match action {
        Action::Tick => app.tick(),
        Action::Quit => app.should_quit = true,
        Action::ToggleLogOverlay => app.toggle_log_overlay(),
        Action::ScrollLogUp => app.scroll_log_up(),
        Action::ScrollLogDown => app.scroll_log_down(),
        Action::ScrollLogLeft => app.scroll_log_left(),
        Action::ScrollLogRight => app.scroll_log_right(),
        Action::ToggleHelp => app.show_help = !app.show_help,
        Action::NextStage => app.set_stage(app.stage.next()),
        Action::PrevStage => app.set_stage(app.stage.prev()),
        Action::NextScopeTab => {
            app.next_scope_tab();
        }
        Action::PrevScopeTab => {
            app.prev_scope_tab();
        }
        Action::ToggleNamingTab => {
            app.naming_tab = app.naming_tab.next();
        }
        Action::NextFocus => app.next_focus(),
        Action::PrevFocus => app.prev_focus(),
        Action::MoveUp => app.move_up(),
        Action::MoveDown => app.move_down(),
        Action::MoveLeft => handle_left(app),
        Action::MoveRight => handle_right(app),
        Action::ExpandAll => app.expand_all_current(),
        Action::CollapseAll => app.collapse_all_current(),
        Action::ToggleSelect => handle_toggle_select(app),
        Action::SelectAllVisibleFiles => {
            if app.stage == Stage::Scope
                && app.scope_tab == ScopeTab::Files
                && app.focus == FocusPane::ScopeFiles
            {
                app.select_all_visible_files();
            }
        }
        Action::ToggleFilesSettingsPopup => {
            if app.stage == Stage::Scope
                && app.scope_tab == ScopeTab::Files
                && app.focus == FocusPane::ScopeFiles
            {
                app.toggle_files_settings_popup();
            }
        }
        Action::ToggleShowSelectedCategoriesOnly => {
            if app.stage == Stage::Scope
                && app.scope_tab == ScopeTab::Files
                && app.focus == FocusPane::ScopeFiles
            {
                app.toggle_show_selected_categories_only();
            }
        }
        Action::TriggerNamingSuggestionsRefresh => {
            if app.stage == Stage::Naming {
                app.trigger_naming_suggestions_refresh();
            }
        }
        Action::ToggleNamingSuggestionsSettings => {
            if app.stage == Stage::Naming {
                app.toggle_naming_suggestions_settings();
            }
        }
        Action::DiscoverModels => {
            if app.stage == Stage::Naming {
                app.discover_openrouter_models();
            }
        }
        Action::SaveAiSettings => {
            app.save_ai_settings_to_disk();
        }
        Action::ClearAiApiKey => {
            app.clear_ai_api_key();
        }
        Action::FocusAiSettingsField(field) => {
            app.focus_ai_settings_field(field);
        }
        Action::SelectAiModelByFilteredIndex(idx) => {
            app.select_ai_model_by_filtered_index(idx);
        }
        Action::SetGlobalSuggestionOption(idx) => {
            app.set_global_suggestion_option(idx);
        }
        Action::SetRowSuggestionOption(idx) => {
            app.set_row_suggestion_option(idx);
        }
        Action::ClearRowSuggestionOption => {
            app.clear_row_suggestion_option();
        }
        Action::NextAiSettingsField => app.ai_settings_next_field(),
        Action::PrevAiSettingsField => app.ai_settings_prev_field(),
        Action::ToggleShowHidden => app.toggle_show_hidden(),
        Action::SavePreset(slot) => app.save_preset(slot),
        Action::LoadPreset(slot) => app.load_preset(slot),
        Action::ApplyMarketplace(index) => app.apply_marketplace(index),
        Action::Submit { stay } => {
            if app.stage == Stage::Apply {
                app.submit_simulated(stay);
            }
        }
        Action::StartOverrideEdit => {
            if app.stage == Stage::Naming && app.mode == InputMode::Normal {
                app.begin_override_edit();
            }
        }
        Action::StartCommandInput => {
            if app.stage == Stage::Naming {
                app.mode = InputMode::Command;
                app.set_focus(FocusPane::NamingCommand);
            }
        }
        Action::StartNewCategoryInput => {
            if app.stage == Stage::Scope && app.scope_tab == ScopeTab::Select {
                app.mode = InputMode::NewCategory;
                app.new_category_input.clear();
            }
        }
        Action::StartGroupAssign => {
            if app.stage == Stage::Naming {
                app.mode = InputMode::AwaitGroup;
                app.push_toast(crate::model::ToastLevel::Info, "Group mode: press 1-9");
            }
        }
        Action::AssignGroup(group) => {
            if app.mode == InputMode::AwaitGroup {
                app.assign_group(group);
            }
        }
        Action::InputChar(ch) => match app.mode {
            InputMode::EditingOverride => app.override_input.push(ch),
            InputMode::Command => app.command_input.push(ch),
            InputMode::NewCategory => app.new_category_input.push(ch),
            InputMode::EditingAiApiKey => app.ai_settings.api_key_input.push(ch),
            InputMode::EditingAiModelSearch => app.ai_settings.model_search_query.push(ch),
            InputMode::EditingAiManualModel => app.ai_settings.manual_model_input.push(ch),
            InputMode::AwaitGroup | InputMode::Normal => {}
        },
        Action::Backspace => match app.mode {
            InputMode::EditingOverride => {
                app.override_input.pop();
            }
            InputMode::Command => {
                app.command_input.pop();
            }
            InputMode::NewCategory => {
                app.new_category_input.pop();
            }
            InputMode::EditingAiApiKey => {
                app.ai_settings.api_key_input.pop();
            }
            InputMode::EditingAiModelSearch => {
                app.ai_settings.model_search_query.pop();
            }
            InputMode::EditingAiManualModel => {
                app.ai_settings.manual_model_input.pop();
            }
            InputMode::AwaitGroup | InputMode::Normal => {}
        },
        Action::CommitInput => handle_commit(app),
        Action::CancelInput => match app.mode {
            InputMode::EditingOverride => app.cancel_override_edit(),
            InputMode::Command
            | InputMode::NewCategory
            | InputMode::AwaitGroup
            | InputMode::EditingAiApiKey
            | InputMode::EditingAiModelSearch
            | InputMode::EditingAiManualModel => {
                app.mode = InputMode::Normal;
            }
            InputMode::Normal => {
                if app.stage == Stage::Naming && app.ai_settings.popup_open {
                    app.ai_settings.popup_open = false;
                } else {
                    app.show_help = false;
                }
            }
        },
        Action::Autocomplete => {
            if app.mode == InputMode::EditingOverride {
                app.autocomplete_override();
            }
        }
        Action::MouseDown(col, row) => handle_mouse_down(app, col, row),
        Action::MouseDrag(col, row) => handle_mouse_drag(app, col, row),
        Action::MouseUp(col, row) => handle_mouse_up(app, col, row),
        Action::MouseScrollUp(col, row) => handle_mouse_scroll(app, col, row, true),
        Action::MouseScrollDown(col, row) => handle_mouse_scroll(app, col, row, false),
        Action::MouseScrollLeft(col, row) => handle_mouse_hscroll(app, col, row, true),
        Action::MouseScrollRight(col, row) => handle_mouse_hscroll(app, col, row, false),
    }
}

fn handle_left(app: &mut AppState) {
    match app.stage {
        Stage::Scope => {
            if app.scope_tab == ScopeTab::Files {
                app.collapse_current();
            } else {
                app.prev_scope_tab();
            }
        }
        Stage::Naming => {
            if app.focus == FocusPane::NamingRight && app.naming_tab == NamingTab::Style {
                app.apply_style_cursor_delta(-1);
            } else {
                app.naming_tab = app.naming_tab.next();
            }
        }
        Stage::Apply => app.set_focus(FocusPane::ApplyReview),
    }
}

fn handle_right(app: &mut AppState) {
    match app.stage {
        Stage::Scope => {
            if app.scope_tab == ScopeTab::Files {
                app.expand_current();
            } else {
                app.next_scope_tab();
            }
        }
        Stage::Naming => {
            if app.focus == FocusPane::NamingRight && app.naming_tab == NamingTab::Style {
                app.apply_style_cursor_delta(1);
            } else {
                app.naming_tab = app.naming_tab.next();
            }
        }
        Stage::Apply => app.set_focus(FocusPane::ApplyHistory),
    }
}

fn handle_toggle_select(app: &mut AppState) {
    if app.mode != InputMode::Normal {
        return;
    }

    match app.stage {
        Stage::Scope => {
            if app.scope_tab == ScopeTab::Files {
                app.toggle_scope_file_selection();
            } else {
                app.toggle_current_scope_filter();
            }
        }
        Stage::Naming => {
            if app.focus == FocusPane::NamingTable {
                app.toggle_current_naming_row();
            }
        }
        Stage::Apply => {}
    }
}

fn handle_commit(app: &mut AppState) {
    match app.mode {
        InputMode::EditingOverride => app.commit_override_edit(),
        InputMode::Command => app.apply_command_input(),
        InputMode::NewCategory => app.add_custom_category_from_input(),
        InputMode::EditingAiApiKey => {
            let value = app.ai_settings.api_key_input.trim();
            app.ai_settings.api_key = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            };
            app.mode = InputMode::Normal;
        }
        InputMode::EditingAiModelSearch => {
            app.mode = InputMode::Normal;
        }
        InputMode::EditingAiManualModel => {
            let value = app.ai_settings.manual_model_input.trim();
            app.ai_settings.selected_model = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            };
            app.mode = InputMode::Normal;
        }
        InputMode::AwaitGroup => {
            app.mode = InputMode::Normal;
        }
        InputMode::Normal => match app.stage {
            Stage::Scope => app.toggle_current_scope_filter(),
            Stage::Naming => {
                if app.focus == FocusPane::NamingRight && app.naming_tab == NamingTab::Suggestions {
                    app.apply_suggestion();
                }
            }
            Stage::Apply => app.submit_simulated(false),
        },
    }
}

fn handle_mouse_down(app: &mut AppState, col: u16, row: u16) {
    if app.show_log_overlay {
        if let Some(target) = app.click_target_at_topmost(col, row) {
            handle_click_target(app, target);
        }
        return;
    }
    let Some(target) = app.click_target_at(col, row) else {
        app.scope_files_drag = None;
        app.clear_scope_file_focus_nodes();
        return;
    };

    match target {
        ClickTarget::ScopeFileCheckbox(index) => {
            app.set_scope_tab(ScopeTab::Files);
            app.set_focus(FocusPane::ScopeFiles);
            app.tree.cursor = index;
            app.normalize_scope_files_cursor();
            app.clear_scope_file_focus_nodes();
            let target_selected = app
                .scope_file_node_id_at(index)
                .map(|node_id| !app.node_selected_for_display(node_id))
                .unwrap_or(false);
            if let Err(error) = app.set_scope_file_selection_at(index, target_selected) {
                app.push_toast(
                    ToastLevel::Error,
                    format!("Selection update failed: {error}"),
                );
            }
            app.scope_files_drag = Some(FilesDragState {
                mode: FilesDragMode::CheckboxSelect {
                    target_selected,
                    visited_rows: HashSet::from([index]),
                },
                start_index: index,
            });
        }
        ClickTarget::ScopeFileRow(index) => {
            app.set_scope_tab(ScopeTab::Files);
            app.set_focus(FocusPane::ScopeFiles);
            app.tree.cursor = index;
            app.normalize_scope_files_cursor();
            app.clear_scope_file_focus_nodes();
            app.add_scope_file_focus_node_at(index);
            app.scope_files_drag = Some(FilesDragState {
                mode: FilesDragMode::RowFocus {
                    visited_rows: HashSet::from([index]),
                },
                start_index: index,
            });
        }
        _ => {
            app.scope_files_drag = None;
            app.clear_scope_file_focus_nodes();
            handle_click_target(app, target);
        }
    }
}

fn handle_mouse_drag(app: &mut AppState, col: u16, row: u16) {
    if app.show_log_overlay {
        return;
    }
    let Some(mut drag) = app.scope_files_drag.take() else {
        return;
    };

    let target = app.click_target_at(col, row);
    match (&mut drag.mode, target) {
        (
            FilesDragMode::CheckboxSelect {
                target_selected,
                visited_rows,
            },
            Some(ClickTarget::ScopeFileCheckbox(index)),
        ) => {
            app.set_scope_tab(ScopeTab::Files);
            app.set_focus(FocusPane::ScopeFiles);
            app.tree.cursor = index;
            app.normalize_scope_files_cursor();
            if visited_rows.insert(index)
                && let Err(error) = app.set_scope_file_selection_at(index, *target_selected)
            {
                app.push_toast(
                    ToastLevel::Error,
                    format!("Selection update failed: {error}"),
                );
            }
        }
        (FilesDragMode::RowFocus { visited_rows }, Some(ClickTarget::ScopeFileRow(index)))
        | (FilesDragMode::RowFocus { visited_rows }, Some(ClickTarget::ScopeFileCheckbox(index))) =>
        {
            app.set_scope_tab(ScopeTab::Files);
            app.set_focus(FocusPane::ScopeFiles);
            app.tree.cursor = index;
            app.normalize_scope_files_cursor();
            if visited_rows.insert(index) {
                app.add_scope_file_focus_node_at(index);
            }
        }
        _ => {}
    }

    app.scope_files_drag = Some(drag);
}

fn handle_mouse_up(app: &mut AppState, _col: u16, _row: u16) {
    if app.show_log_overlay {
        return;
    }
    let Some(drag) = app.scope_files_drag.take() else {
        return;
    };

    match drag.mode {
        FilesDragMode::CheckboxSelect { .. } => {
            app.sync_rename_rows();
        }
        FilesDragMode::RowFocus { visited_rows } => {
            if visited_rows.len() <= 1 {
                app.clear_scope_file_focus_nodes();
                handle_click_target(app, ClickTarget::ScopeFileRow(drag.start_index));
            }
        }
    }
}

fn handle_click_target(app: &mut AppState, target: ClickTarget) {
    match target {
        ClickTarget::StageTab(stage) => app.set_stage(stage),
        ClickTarget::ScopeTab(tab) => app.set_scope_tab(tab),
        ClickTarget::ScopeFileRow(index) => {
            app.set_scope_tab(ScopeTab::Files);
            app.tree.cursor = index;
            app.normalize_scope_files_cursor();
            if let Some(node_id) = app.current_scope_file_node_id()
                && app.tree.nodes[node_id].is_dir
            {
                if app.tree.nodes[node_id].expanded {
                    app.collapse_current();
                } else {
                    app.expand_current();
                }
            }
        }
        ClickTarget::ScopeFileCheckbox(index) => {
            app.set_scope_tab(ScopeTab::Files);
            app.set_focus(FocusPane::ScopeFiles);
            app.tree.cursor = index;
            app.normalize_scope_files_cursor();
            app.clear_scope_file_focus_nodes();
            app.toggle_scope_file_selection();
        }
        ClickTarget::ScopeFilesSettingsButton => {
            app.set_scope_tab(ScopeTab::Files);
            app.set_focus(FocusPane::ScopeFiles);
            app.toggle_files_settings_popup();
        }
        ClickTarget::ScopeFilesSettingShowSelectedCategoriesOnly => {
            app.set_scope_tab(ScopeTab::Files);
            app.set_focus(FocusPane::ScopeFiles);
            app.toggle_show_selected_categories_only();
        }
        ClickTarget::ScopeCategoryRow(index) => {
            app.set_scope_tab(ScopeTab::Select);
            app.select_cursor = index;
            app.toggle_current_scope_filter();
        }
        ClickTarget::ScopeConstraintRow(index) => {
            app.set_scope_tab(ScopeTab::Constraint);
            app.constraint_cursor = index;
            app.toggle_current_scope_filter();
        }
        ClickTarget::ScopePresetRow(index) => {
            app.set_scope_tab(ScopeTab::Preset);
            app.preset_cursor = index;
            app.load_preset((index + 1) as u8);
        }
        ClickTarget::ScopeMarketplaceRow(index) => {
            app.set_scope_tab(ScopeTab::Marketplace);
            app.marketplace_cursor = index;
            app.apply_marketplace(index);
        }
        ClickTarget::ScopePane(focus) => app.set_focus(focus),
        ClickTarget::NamingPane(focus) => {
            app.set_focus(focus);
            if focus == FocusPane::NamingCommand {
                app.mode = InputMode::Command;
            }
        }
        ClickTarget::NamingRow(index) => {
            app.set_focus(FocusPane::NamingTable);
            app.rename_cursor = index;
            app.toggle_current_naming_row();
        }
        ClickTarget::NamingSuggestion(index) => {
            app.set_focus(FocusPane::NamingRight);
            app.naming_tab = NamingTab::Suggestions;
            app.suggestion_cursor = index;
            app.apply_suggestion();
        }
        ClickTarget::NamingSuggestionsSettingsButton => {
            app.set_focus(FocusPane::NamingRight);
            app.naming_tab = NamingTab::Suggestions;
            app.toggle_naming_suggestions_settings();
        }
        ClickTarget::NamingSuggestionsRefreshButton => {
            app.set_focus(FocusPane::NamingRight);
            app.naming_tab = NamingTab::Suggestions;
            app.trigger_naming_suggestions_refresh();
        }
        ClickTarget::NamingAiSettingsApiKeyField => {
            app.focus_ai_settings_field(AiSettingsField::ApiKey);
        }
        ClickTarget::NamingAiSettingsModelSearchField => {
            app.focus_ai_settings_field(AiSettingsField::ModelSearch);
        }
        ClickTarget::NamingAiSettingsManualModelField => {
            app.focus_ai_settings_field(AiSettingsField::ManualModel);
        }
        ClickTarget::NamingAiSettingsSaveConfig => app.save_ai_settings_to_disk(),
        ClickTarget::NamingAiSettingsClearApiKey => app.clear_ai_api_key(),
        ClickTarget::NamingAiSettingsToggleReveal => {
            app.ai_settings.reveal_api_key = !app.ai_settings.reveal_api_key
        }
        ClickTarget::NamingAiSettingsDiscoverModels => app.discover_openrouter_models(),
        ClickTarget::NamingAiSettingsModelRow(index) => {
            app.select_ai_model_by_filtered_index(index)
        }
        ClickTarget::NamingRowSuggestionOption {
            row_index,
            option_index,
        } => {
            app.rename_cursor = row_index.min(app.rename_rows.len().saturating_sub(1));
            app.set_row_suggestion_option(option_index);
        }
        ClickTarget::NamingStyleRow(index) => {
            app.set_focus(FocusPane::NamingRight);
            app.naming_tab = NamingTab::Style;
            app.style_cursor = index;
        }
        ClickTarget::ApplyPane(focus) => app.set_focus(focus),
        ClickTarget::ApplyRow(index) => {
            app.set_focus(FocusPane::ApplyReview);
            app.apply_cursor = index;
        }
        ClickTarget::ApplyHistoryRow(index) => {
            app.set_focus(FocusPane::ApplyHistory);
            app.history_cursor = index;
        }
        ClickTarget::LogCopyLine(index) => app.copy_log_line(index),
        ClickTarget::LogOverlay => {}
    }
}

fn handle_mouse_scroll(app: &mut AppState, col: u16, row: u16, scroll_up: bool) {
    let delta: isize = if scroll_up { -1 } else { 1 };

    if app.show_log_overlay {
        if scroll_up {
            app.scroll_log_up();
        } else {
            app.scroll_log_down();
        }
        return;
    }

    let maybe_target = app.click_target_at(col, row);

    let Some(target) = maybe_target else {
        if scroll_up {
            app.move_up();
        } else {
            app.move_down();
        }
        return;
    };

    match target {
        ClickTarget::StageTab(_) | ClickTarget::ScopeTab(_) => {}
        ClickTarget::ScopeFileRow(_) => {
            app.set_scope_tab(ScopeTab::Files);
            app.normalize_scope_files_cursor();
            app.tree.cursor = scroll_index(app.tree.cursor, app.scope_files_rows_len(), delta);
            app.normalize_scope_files_cursor();
        }
        ClickTarget::ScopeFileCheckbox(_) => {
            app.set_scope_tab(ScopeTab::Files);
            app.normalize_scope_files_cursor();
            app.tree.cursor = scroll_index(app.tree.cursor, app.scope_files_rows_len(), delta);
            app.normalize_scope_files_cursor();
        }
        ClickTarget::ScopeFilesSettingsButton
        | ClickTarget::ScopeFilesSettingShowSelectedCategoriesOnly => {}
        ClickTarget::ScopeCategoryRow(_) => {
            app.set_scope_tab(ScopeTab::Select);
            app.select_cursor = scroll_index(app.select_cursor, app.category_filters.len(), delta);
        }
        ClickTarget::ScopeConstraintRow(_) => {
            app.set_scope_tab(ScopeTab::Constraint);
            let max_len =
                crate::model::TimeConstraint::ALL.len() + crate::model::SizeConstraint::ALL.len();
            app.constraint_cursor = scroll_index(app.constraint_cursor, max_len, delta);
        }
        ClickTarget::ScopePresetRow(_) => {
            app.set_scope_tab(ScopeTab::Preset);
            app.preset_cursor = scroll_index(app.preset_cursor, 9, delta);
        }
        ClickTarget::ScopeMarketplaceRow(_) => {
            app.set_scope_tab(ScopeTab::Marketplace);
            app.marketplace_cursor =
                scroll_index(app.marketplace_cursor, app.marketplace_presets.len(), delta);
        }
        ClickTarget::ScopePane(focus) => {
            app.set_focus(focus);
            match focus {
                FocusPane::ScopeCategory => {
                    app.select_cursor =
                        scroll_index(app.select_cursor, app.category_filters.len(), delta);
                }
                FocusPane::ScopeFiles => {
                    app.tree.cursor =
                        scroll_index(app.tree.cursor, app.scope_files_rows_len(), delta);
                    app.normalize_scope_files_cursor();
                }
                FocusPane::ScopeOptions => {
                    let max_len = match app.scope_right_tab {
                        ScopeTab::Constraint => {
                            crate::model::TimeConstraint::ALL.len()
                                + crate::model::SizeConstraint::ALL.len()
                        }
                        ScopeTab::Preset => 9,
                        ScopeTab::Marketplace => app.marketplace_presets.len(),
                        ScopeTab::Files | ScopeTab::Select => 0,
                    };
                    match app.scope_right_tab {
                        ScopeTab::Constraint => {
                            app.constraint_cursor =
                                scroll_index(app.constraint_cursor, max_len, delta);
                        }
                        ScopeTab::Preset => {
                            app.preset_cursor = scroll_index(app.preset_cursor, max_len, delta);
                        }
                        ScopeTab::Marketplace => {
                            app.marketplace_cursor =
                                scroll_index(app.marketplace_cursor, max_len, delta);
                        }
                        ScopeTab::Files | ScopeTab::Select => {}
                    }
                }
                _ => {}
            }
        }
        ClickTarget::NamingPane(focus) => {
            app.set_focus(focus);
            match focus {
                FocusPane::NamingTable => {
                    app.rename_cursor =
                        scroll_index(app.rename_cursor, app.rename_rows.len(), delta);
                }
                FocusPane::NamingRight => {
                    if app.naming_tab == NamingTab::Suggestions {
                        app.suggestion_cursor =
                            scroll_index(app.suggestion_cursor, app.suggestions.len(), delta);
                    } else {
                        app.style_cursor = scroll_index(app.style_cursor, 5, delta);
                    }
                }
                FocusPane::NamingCommand => {}
                _ => {}
            }
        }
        ClickTarget::NamingRow(_) => {
            app.set_focus(FocusPane::NamingTable);
            app.rename_cursor = scroll_index(app.rename_cursor, app.rename_rows.len(), delta);
        }
        ClickTarget::NamingSuggestion(_) => {
            app.set_focus(FocusPane::NamingRight);
            app.naming_tab = NamingTab::Suggestions;
            app.suggestion_cursor =
                scroll_index(app.suggestion_cursor, app.suggestions.len(), delta);
        }
        ClickTarget::NamingAiSettingsModelRow(_) => {
            let max = app.filtered_ai_models().len();
            app.ai_settings.model_list_cursor =
                scroll_index(app.ai_settings.model_list_cursor, max, delta);
        }
        ClickTarget::NamingSuggestionsSettingsButton
        | ClickTarget::NamingSuggestionsRefreshButton
        | ClickTarget::NamingAiSettingsApiKeyField
        | ClickTarget::NamingAiSettingsModelSearchField
        | ClickTarget::NamingAiSettingsManualModelField
        | ClickTarget::NamingAiSettingsSaveConfig
        | ClickTarget::NamingAiSettingsClearApiKey
        | ClickTarget::NamingAiSettingsToggleReveal
        | ClickTarget::NamingAiSettingsDiscoverModels
        | ClickTarget::NamingRowSuggestionOption { .. } => {}
        ClickTarget::NamingStyleRow(_) => {
            app.set_focus(FocusPane::NamingRight);
            app.naming_tab = NamingTab::Style;
            app.style_cursor = scroll_index(app.style_cursor, 5, delta);
        }
        ClickTarget::ApplyPane(focus) => {
            app.set_focus(focus);
            match focus {
                FocusPane::ApplyReview => {
                    app.apply_cursor = scroll_index(
                        app.apply_cursor,
                        app.rename_rows.iter().filter(|row| row.selected).count(),
                        delta,
                    );
                }
                FocusPane::ApplyHistory => {
                    app.history_cursor =
                        scroll_index(app.history_cursor, app.undo_history.len(), delta);
                }
                _ => {}
            }
        }
        ClickTarget::ApplyRow(_) => {
            app.set_focus(FocusPane::ApplyReview);
            app.apply_cursor = scroll_index(
                app.apply_cursor,
                app.rename_rows.iter().filter(|row| row.selected).count(),
                delta,
            );
        }
        ClickTarget::ApplyHistoryRow(_) => {
            app.set_focus(FocusPane::ApplyHistory);
            app.history_cursor = scroll_index(app.history_cursor, app.undo_history.len(), delta);
        }
        ClickTarget::LogCopyLine(_) => {}
        ClickTarget::LogOverlay => {}
    }
}

fn handle_mouse_hscroll(app: &mut AppState, _col: u16, _row: u16, scroll_left: bool) {
    if !app.show_log_overlay {
        return;
    }

    if scroll_left {
        app.scroll_log_left();
    } else {
        app.scroll_log_right();
    }
}

fn scroll_index(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    if delta < 0 {
        current.saturating_sub((-delta) as usize)
    } else {
        (current + delta as usize).min(len - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_navigation_cycles() {
        let cwd = std::env::current_dir().expect("cwd");
        let mut app = AppState::new(cwd);

        reduce(&mut app, Action::NextStage);
        assert_eq!(app.stage, Stage::Naming);
        reduce(&mut app, Action::NextStage);
        assert_eq!(app.stage, Stage::Apply);
        reduce(&mut app, Action::NextStage);
        assert_eq!(app.stage, Stage::Scope);
    }

    #[test]
    fn command_mode_commits_changes() {
        let cwd = std::env::current_dir().expect("cwd");
        let mut app = AppState::new(cwd);
        app.set_stage(Stage::Naming);

        reduce(&mut app, Action::StartCommandInput);
        reduce(&mut app, Action::InputChar('s'));
        reduce(&mut app, Action::InputChar('h'));
        reduce(&mut app, Action::InputChar('o'));
        reduce(&mut app, Action::InputChar('r'));
        reduce(&mut app, Action::InputChar('t'));
        reduce(&mut app, Action::CommitInput);

        assert_eq!(app.mode, InputMode::Normal);
    }
}
