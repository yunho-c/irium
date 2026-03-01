use crate::{
    model::{AppState, FocusPane, InputMode, NamingTab, ScopeTab, Stage},
};

#[derive(Debug, Clone)]
pub enum Action {
    Tick,
    Quit,
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
    MouseClick(u16, u16),
    MouseScrollUp,
    MouseScrollDown,
}

pub fn reduce(app: &mut AppState, action: Action) {
    match action {
        Action::Tick => app.tick(),
        Action::Quit => app.should_quit = true,
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
                app.focus = FocusPane::NamingCommand;
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
            InputMode::AwaitGroup | InputMode::Normal => {}
        },
        Action::CommitInput => handle_commit(app),
        Action::CancelInput => match app.mode {
            InputMode::EditingOverride => app.cancel_override_edit(),
            InputMode::Command | InputMode::NewCategory | InputMode::AwaitGroup => {
                app.mode = InputMode::Normal;
            }
            InputMode::Normal => app.show_help = false,
        },
        Action::Autocomplete => {
            if app.mode == InputMode::EditingOverride {
                app.autocomplete_override();
            }
        }
        Action::MouseClick(col, row) => handle_mouse_click(app, col, row),
        Action::MouseScrollUp => app.move_up(),
        Action::MouseScrollDown => app.move_down(),
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
        Stage::Apply => app.focus = FocusPane::ApplyReview,
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
        Stage::Apply => app.focus = FocusPane::ApplyHistory,
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

fn contains(rect: ratatui::layout::Rect, col: u16, row: u16) -> bool {
    col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

fn handle_mouse_click(app: &mut AppState, col: u16, row: u16) {
    if let Some(stage) = app
        .ui_map
        .stage_tabs
        .iter()
        .find(|tab| contains(tab.rect, col, row))
        .map(|tab| tab.value)
    {
        app.set_stage(stage);
        return;
    }

    match app.stage {
        Stage::Scope => {
            if let Some(tab) = app
                .ui_map
                .scope_tabs
                .iter()
                .find(|tab| contains(tab.rect, col, row))
                .map(|tab| tab.value)
            {
                app.set_scope_tab(tab);
                return;
            }

            if let Some(index) = app
                .ui_map
                .file_rows
                .iter()
                .find(|hit| contains(hit.rect, col, row))
                .map(|hit| hit.index)
            {
                app.set_scope_tab(ScopeTab::Files);
                app.tree.cursor = index;
                app.tree.fix_cursor();
                app.toggle_scope_file_selection();
                return;
            }

            if let Some(index) = app
                .ui_map
                .select_rows
                .iter()
                .find(|hit| contains(hit.rect, col, row))
                .map(|hit| hit.index)
            {
                app.set_scope_tab(ScopeTab::Select);
                app.select_cursor = index;
                app.toggle_current_scope_filter();
                return;
            }

            if let Some(index) = app
                .ui_map
                .constraint_rows
                .iter()
                .find(|hit| contains(hit.rect, col, row))
                .map(|hit| hit.index)
            {
                app.set_scope_tab(ScopeTab::Constraint);
                app.constraint_cursor = index;
                app.toggle_current_scope_filter();
                return;
            }

            if let Some(index) = app
                .ui_map
                .preset_rows
                .iter()
                .find(|hit| contains(hit.rect, col, row))
                .map(|hit| hit.index)
            {
                app.set_scope_tab(ScopeTab::Preset);
                app.preset_cursor = index;
                app.load_preset((index + 1) as u8);
                return;
            }

            if let Some(index) = app
                .ui_map
                .marketplace_rows
                .iter()
                .find(|hit| contains(hit.rect, col, row))
                .map(|hit| hit.index)
            {
                app.set_scope_tab(ScopeTab::Marketplace);
                app.marketplace_cursor = index;
                app.apply_marketplace(index);
            }
        }
        Stage::Naming => {
            if let Some(tab) = app
                .ui_map
                .naming_tabs
                .iter()
                .find(|tab| contains(tab.rect, col, row))
                .map(|tab| tab.value)
            {
                app.naming_tab = tab;
                return;
            }

            if contains(app.ui_map.naming_table, col, row) {
                app.focus = FocusPane::NamingTable;
            } else if contains(app.ui_map.naming_right, col, row) {
                app.focus = FocusPane::NamingRight;
            } else if contains(app.ui_map.naming_command, col, row) {
                app.focus = FocusPane::NamingCommand;
                app.mode = InputMode::Command;
            }

            if let Some(hit) = app
                .ui_map
                .rename_rows
                .iter()
                .find(|hit| contains(hit.rect, col, row))
            {
                app.rename_cursor = hit.index;
                app.toggle_current_naming_row();
                return;
            }

            if app.naming_tab == NamingTab::Suggestions
                && let Some(hit) = app
                    .ui_map
                    .suggestion_rows
                    .iter()
                    .find(|hit| contains(hit.rect, col, row))
            {
                app.suggestion_cursor = hit.index;
                app.apply_suggestion();
                return;
            }

            if app.naming_tab == NamingTab::Style
                && let Some(hit) = app
                    .ui_map
                    .style_rows
                    .iter()
                    .find(|hit| contains(hit.rect, col, row))
            {
                app.style_cursor = hit.index;
            }
        }
        Stage::Apply => {
            if contains(app.ui_map.apply_review, col, row) {
                app.focus = FocusPane::ApplyReview;
            }
            if contains(app.ui_map.apply_history, col, row) {
                app.focus = FocusPane::ApplyHistory;
            }

            if let Some(hit) = app
                .ui_map
                .apply_rows
                .iter()
                .find(|hit| contains(hit.rect, col, row))
            {
                app.apply_cursor = hit.index;
            }

            if let Some(hit) = app
                .ui_map
                .history_rows
                .iter()
                .find(|hit| contains(hit.rect, col, row))
            {
                app.history_cursor = hit.index;
            }
        }
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
