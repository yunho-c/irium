use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};

use crate::{
    actions::Action,
    model::{AppState, FocusPane, InputMode, ScopeTab, Stage},
};

pub fn map_event(app: &AppState, event: Event) -> Vec<Action> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => map_key(app, key),
        Event::Mouse(mouse) => {
            let shift = mouse.modifiers.contains(KeyModifiers::SHIFT);
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    vec![Action::MouseDown(mouse.column, mouse.row)]
                }
                MouseEventKind::Drag(MouseButton::Left) => {
                    vec![Action::MouseDrag(mouse.column, mouse.row)]
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    vec![Action::MouseUp(mouse.column, mouse.row)]
                }
                MouseEventKind::ScrollUp if shift => {
                    vec![Action::MouseScrollLeft(mouse.column, mouse.row)]
                }
                MouseEventKind::ScrollDown if shift => {
                    vec![Action::MouseScrollRight(mouse.column, mouse.row)]
                }
                MouseEventKind::ScrollUp => vec![Action::MouseScrollUp(mouse.column, mouse.row)],
                MouseEventKind::ScrollDown => {
                    vec![Action::MouseScrollDown(mouse.column, mouse.row)]
                }
                MouseEventKind::ScrollLeft => {
                    vec![Action::MouseScrollLeft(mouse.column, mouse.row)]
                }
                MouseEventKind::ScrollRight => {
                    vec![Action::MouseScrollRight(mouse.column, mouse.row)]
                }
                _ => Vec::new(),
            }
        }
        Event::Resize(_, _) => Vec::new(),
        _ => Vec::new(),
    }
}

fn map_key(app: &AppState, key: KeyEvent) -> Vec<Action> {
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
    {
        return vec![Action::Quit];
    }
    if app.show_log_overlay {
        return map_log_overlay_keys(key);
    }
    if app.show_prompt_history_overlay {
        return map_prompt_history_keys(key);
    }

    match app.mode {
        InputMode::EditingOverride
        | InputMode::Command
        | InputMode::NewCategory
        | InputMode::EditingAiApiKey
        | InputMode::EditingAiModelSearch
        | InputMode::EditingAiManualModel => map_text_input_keys(app, key),
        InputMode::AwaitGroup => map_group_keys(key),
        InputMode::Normal => map_normal_keys(app, key),
    }
}

fn map_log_overlay_keys(key: KeyEvent) -> Vec<Action> {
    if key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
    {
        return Vec::new();
    }
    match key.code {
        KeyCode::Char('L') | KeyCode::Char('l') => vec![Action::ToggleLogOverlay],
        KeyCode::Esc => vec![Action::ToggleLogOverlay],
        KeyCode::Left if key.modifiers.contains(KeyModifiers::SHIFT) => {
            vec![Action::ScrollLogLeft]
        }
        KeyCode::Right if key.modifiers.contains(KeyModifiers::SHIFT) => {
            vec![Action::ScrollLogRight]
        }
        KeyCode::Up | KeyCode::Char('k') => vec![Action::ScrollLogUp],
        KeyCode::Down | KeyCode::Char('j') => vec![Action::ScrollLogDown],
        KeyCode::PageUp => vec![
            Action::ScrollLogUp,
            Action::ScrollLogUp,
            Action::ScrollLogUp,
        ],
        KeyCode::PageDown => {
            vec![
                Action::ScrollLogDown,
                Action::ScrollLogDown,
                Action::ScrollLogDown,
            ]
        }
        _ => Vec::new(),
    }
}

fn map_prompt_history_keys(key: KeyEvent) -> Vec<Action> {
    if key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
    {
        return Vec::new();
    }
    match key.code {
        KeyCode::Char('p') | KeyCode::Char('P') => vec![Action::TogglePromptHistoryOverlay],
        KeyCode::Esc => vec![Action::TogglePromptHistoryOverlay],
        KeyCode::Enter => vec![Action::SelectPromptHistoryEntry],
        KeyCode::Up | KeyCode::Char('k') => vec![Action::PromptHistoryUp],
        KeyCode::Down | KeyCode::Char('j') => vec![Action::PromptHistoryDown],
        KeyCode::PageUp => vec![
            Action::PromptHistoryUp,
            Action::PromptHistoryUp,
            Action::PromptHistoryUp,
        ],
        KeyCode::PageDown => vec![
            Action::PromptHistoryDown,
            Action::PromptHistoryDown,
            Action::PromptHistoryDown,
        ],
        _ => Vec::new(),
    }
}

fn map_text_input_keys(app: &AppState, key: KeyEvent) -> Vec<Action> {
    match key.code {
        KeyCode::Esc => vec![Action::CancelInput],
        KeyCode::Enter => vec![Action::CommitInput],
        KeyCode::Backspace => vec![Action::Backspace],
        KeyCode::Up if app.mode == InputMode::Command => vec![Action::CommandHistoryPrev],
        KeyCode::Down if app.mode == InputMode::Command => vec![Action::CommandHistoryNext],
        KeyCode::Tab
            if matches!(
                app.mode,
                InputMode::EditingAiApiKey
                    | InputMode::EditingAiModelSearch
                    | InputMode::EditingAiManualModel
            ) =>
        {
            vec![Action::NextAiSettingsField]
        }
        KeyCode::BackTab
            if matches!(
                app.mode,
                InputMode::EditingAiApiKey
                    | InputMode::EditingAiModelSearch
                    | InputMode::EditingAiManualModel
            ) =>
        {
            vec![Action::PrevAiSettingsField]
        }
        KeyCode::Tab if app.mode == InputMode::EditingOverride => vec![Action::Autocomplete],
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            vec![Action::InputChar(ch)]
        }
        _ => Vec::new(),
    }
}

fn map_group_keys(key: KeyEvent) -> Vec<Action> {
    match key.code {
        KeyCode::Esc => vec![Action::CancelInput],
        KeyCode::Char(ch @ '1'..='9') => vec![Action::AssignGroup((ch as u8) - b'0')],
        _ => Vec::new(),
    }
}

fn map_normal_keys(app: &AppState, key: KeyEvent) -> Vec<Action> {
    if !key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        && matches!(key.code, KeyCode::Char('L') | KeyCode::Char('l'))
    {
        return vec![Action::ToggleLogOverlay];
    }
    if app.stage == Stage::Naming && app.ai_settings.popup_open {
        if key.code == KeyCode::Tab {
            return vec![Action::NextAiSettingsField];
        }
        if key.code == KeyCode::BackTab {
            return vec![Action::PrevAiSettingsField];
        }
    }
    if !key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        && matches!(key.code, KeyCode::Char('m') | KeyCode::Char('M'))
        && app.stage == Stage::Naming
    {
        return vec![Action::ToggleNamingSuggestionsSettings];
    }
    if !key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        && matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R'))
        && app.stage == Stage::Naming
    {
        return vec![Action::TriggerNamingSuggestionsRefresh];
    }
    if !key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        && app.stage == Stage::Naming
        && app.focus == FocusPane::NamingTable
    {
        match key.code {
            KeyCode::Char('1') => return vec![Action::SetRowSuggestionOption(0)],
            KeyCode::Char('2') => return vec![Action::SetRowSuggestionOption(1)],
            KeyCode::Char('3') => return vec![Action::SetRowSuggestionOption(2)],
            KeyCode::Char('0') => return vec![Action::ClearRowSuggestionOption],
            _ => {}
        }
    }
    if !key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        && app.stage == Stage::Naming
        && app.focus == FocusPane::NamingRight
        && app.naming_tab == crate::model::NamingTab::Suggestions
    {
        match key.code {
            KeyCode::Char('1') | KeyCode::Char('!') => {
                return vec![Action::SetGlobalSuggestionOption(0)];
            }
            KeyCode::Char('2') | KeyCode::Char('@') => {
                return vec![Action::SetGlobalSuggestionOption(1)];
            }
            KeyCode::Char('3') | KeyCode::Char('#') => {
                return vec![Action::SetGlobalSuggestionOption(2)];
            }
            KeyCode::Char('d') | KeyCode::Char('D') if app.ai_settings.popup_open => {
                return vec![Action::DiscoverModels];
            }
            KeyCode::Char('s') | KeyCode::Char('S') if app.ai_settings.popup_open => {
                return vec![Action::SaveAiSettings];
            }
            KeyCode::Char('c') | KeyCode::Char('C') if app.ai_settings.popup_open => {
                return vec![Action::ClearAiApiKey];
            }
            _ => {}
        }
    }
    if !key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        && matches!(key.code, KeyCode::Char('a') | KeyCode::Char('A'))
        && app.stage == Stage::Scope
        && app.scope_tab == ScopeTab::Files
        && app.focus == FocusPane::ScopeFiles
    {
        return vec![Action::SelectAllVisibleFiles];
    }
    if !key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        && matches!(key.code, KeyCode::Char('p') | KeyCode::Char('P'))
        && app.stage == Stage::Naming
    {
        return vec![Action::TogglePromptHistoryOverlay];
    }
    if !key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        && matches!(key.code, KeyCode::Char('p') | KeyCode::Char('P'))
        && app.stage == Stage::Scope
        && app.scope_tab == ScopeTab::Files
        && app.focus == FocusPane::ScopeFiles
    {
        return vec![Action::ToggleFilesSettingsPopup];
    }
    if !key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        && matches!(key.code, KeyCode::Char('v') | KeyCode::Char('V'))
        && ((app.stage == Stage::Scope
            && app.scope_tab == ScopeTab::Files
            && app.focus == FocusPane::ScopeFiles)
            || app.stage == Stage::Naming)
    {
        return vec![Action::ToggleFilesPreview];
    }
    if !key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        && matches!(key.code, KeyCode::Char('f') | KeyCode::Char('F'))
        && app.stage == Stage::Scope
        && app.scope_tab == ScopeTab::Files
        && app.focus == FocusPane::ScopeFiles
    {
        return vec![Action::ToggleShowSelectedCategoriesOnly];
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return map_ctrl_keys(app, key);
    }

    match key.code {
        KeyCode::Char('q') => vec![Action::Quit],
        KeyCode::Char('?') => vec![Action::ToggleHelp],
        KeyCode::Char('[') => vec![Action::PrevStage],
        KeyCode::Char(']') => vec![Action::NextStage],
        KeyCode::Tab => vec![Action::NextFocus],
        KeyCode::BackTab => vec![Action::PrevFocus],
        KeyCode::Esc => vec![Action::CancelInput],
        KeyCode::Up => vec![Action::MoveUp],
        KeyCode::Down => vec![Action::MoveDown],
        KeyCode::Left => vec![Action::MoveLeft],
        KeyCode::Right => vec![Action::MoveRight],
        KeyCode::Enter => {
            if app.ai_settings.popup_open && app.stage == Stage::Naming {
                match app.ai_settings.active_field {
                    crate::model::AiSettingsField::ApiKey => {
                        vec![Action::FocusAiSettingsField(
                            crate::model::AiSettingsField::ApiKey,
                        )]
                    }
                    crate::model::AiSettingsField::ModelSearch => {
                        vec![Action::FocusAiSettingsField(
                            crate::model::AiSettingsField::ModelSearch,
                        )]
                    }
                    crate::model::AiSettingsField::ModelList => {
                        vec![Action::SelectAiModelByFilteredIndex(
                            app.ai_settings.model_list_cursor,
                        )]
                    }
                    crate::model::AiSettingsField::ManualModel => {
                        vec![Action::FocusAiSettingsField(
                            crate::model::AiSettingsField::ManualModel,
                        )]
                    }
                    crate::model::AiSettingsField::Save => vec![Action::SaveAiSettings],
                    crate::model::AiSettingsField::Discover => vec![Action::DiscoverModels],
                }
            } else {
                vec![Action::CommitInput]
            }
        }
        KeyCode::Char(' ') => vec![Action::ToggleSelect],
        KeyCode::Char('h') => vec![Action::ToggleShowHidden],
        KeyCode::Char('t') => {
            if app.stage == Stage::Scope {
                vec![Action::NextScopeTab]
            } else if app.stage == Stage::Naming {
                vec![Action::ToggleNamingTab]
            } else {
                Vec::new()
            }
        }
        KeyCode::Char('T') => {
            if app.stage == Stage::Scope {
                vec![Action::PrevScopeTab]
            } else {
                Vec::new()
            }
        }
        KeyCode::Char('e') => vec![Action::StartOverrideEdit],
        KeyCode::Char('g') => vec![Action::StartGroupAssign],
        KeyCode::Char('/') => vec![Action::StartCommandInput],
        KeyCode::Char('n') => vec![Action::StartNewCategoryInput],
        KeyCode::Char('a')
            if app.stage == Stage::Scope && app.scope_tab == ScopeTab::Marketplace =>
        {
            vec![Action::ApplyMarketplace(app.marketplace_cursor)]
        }
        KeyCode::Char(ch @ '1'..='9') => {
            if app.stage == Stage::Scope && app.scope_tab == ScopeTab::Preset {
                vec![Action::LoadPreset((ch as u8) - b'0')]
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

fn map_ctrl_keys(app: &AppState, key: KeyEvent) -> Vec<Action> {
    match key.code {
        KeyCode::Right => vec![Action::ExpandAll],
        KeyCode::Left => vec![Action::CollapseAll],
        KeyCode::Enter => vec![Action::Submit { stay: true }],
        KeyCode::Char(ch @ '1'..='9') => {
            if app.stage == Stage::Scope {
                vec![Action::SavePreset((ch as u8) - b'0')]
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    use super::map_event;
    use crate::{
        actions::Action,
        model::{FocusPane, ScopeTab, Stage},
    };

    #[test]
    fn ctrl_a_does_not_map_to_select_all_in_scope_files() {
        let cwd = std::env::current_dir().expect("cwd");
        let mut app = crate::model::AppState::new(cwd);
        app.set_stage(Stage::Scope);
        app.set_scope_tab(ScopeTab::Files);
        app.set_focus(FocusPane::ScopeFiles);

        let event = Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
        let actions = map_event(&app, event);
        assert!(!matches!(
            actions.as_slice(),
            [Action::SelectAllVisibleFiles]
        ));
    }

    #[test]
    fn a_maps_to_select_all_in_scope_files() {
        let cwd = std::env::current_dir().expect("cwd");
        let mut app = crate::model::AppState::new(cwd);
        app.set_stage(Stage::Scope);
        app.set_scope_tab(ScopeTab::Files);
        app.set_focus(FocusPane::ScopeFiles);

        let event = Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        let actions = map_event(&app, event);
        assert!(matches!(
            actions.as_slice(),
            [Action::SelectAllVisibleFiles]
        ));
    }

    #[test]
    fn p_maps_to_toggle_files_settings_in_scope_files() {
        let cwd = std::env::current_dir().expect("cwd");
        let mut app = crate::model::AppState::new(cwd);
        app.set_stage(Stage::Scope);
        app.set_scope_tab(ScopeTab::Files);
        app.set_focus(FocusPane::ScopeFiles);

        let event = Event::Key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
        let actions = map_event(&app, event);
        assert!(matches!(
            actions.as_slice(),
            [Action::ToggleFilesSettingsPopup]
        ));
    }

    #[test]
    fn v_maps_to_toggle_files_preview_in_scope_files() {
        let cwd = std::env::current_dir().expect("cwd");
        let mut app = crate::model::AppState::new(cwd);
        app.set_stage(Stage::Scope);
        app.set_scope_tab(ScopeTab::Files);
        app.set_focus(FocusPane::ScopeFiles);

        let event = Event::Key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE));
        let actions = map_event(&app, event);
        assert!(matches!(actions.as_slice(), [Action::ToggleFilesPreview]));
    }

    #[test]
    fn v_maps_to_toggle_files_preview_in_naming_stage() {
        let cwd = std::env::current_dir().expect("cwd");
        let mut app = crate::model::AppState::new(cwd);
        app.set_stage(Stage::Naming);
        app.set_focus(FocusPane::NamingTable);

        let event = Event::Key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE));
        let actions = map_event(&app, event);
        assert!(matches!(actions.as_slice(), [Action::ToggleFilesPreview]));
    }

    #[test]
    fn f_maps_to_toggle_selected_categories_visibility_in_scope_files() {
        let cwd = std::env::current_dir().expect("cwd");
        let mut app = crate::model::AppState::new(cwd);
        app.set_stage(Stage::Scope);
        app.set_scope_tab(ScopeTab::Files);
        app.set_focus(FocusPane::ScopeFiles);

        let event = Event::Key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
        let actions = map_event(&app, event);
        assert!(matches!(
            actions.as_slice(),
            [Action::ToggleShowSelectedCategoriesOnly]
        ));
    }
}
