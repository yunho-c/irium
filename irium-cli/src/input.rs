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
        Event::Mouse(mouse) => match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                vec![Action::MouseClick(mouse.column, mouse.row)]
            }
            MouseEventKind::ScrollUp => vec![Action::MouseScrollUp(mouse.column, mouse.row)],
            MouseEventKind::ScrollDown => vec![Action::MouseScrollDown(mouse.column, mouse.row)],
            _ => Vec::new(),
        },
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

    match app.mode {
        InputMode::EditingOverride | InputMode::Command | InputMode::NewCategory => {
            map_text_input_keys(app, key)
        }
        InputMode::AwaitGroup => map_group_keys(key),
        InputMode::Normal => map_normal_keys(app, key),
    }
}

fn map_text_input_keys(app: &AppState, key: KeyEvent) -> Vec<Action> {
    match key.code {
        KeyCode::Esc => vec![Action::CancelInput],
        KeyCode::Enter => vec![Action::CommitInput],
        KeyCode::Backspace => vec![Action::Backspace],
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
        && matches!(key.code, KeyCode::Char('a') | KeyCode::Char('A'))
        && app.stage == Stage::Scope
        && app.scope_tab == ScopeTab::Files
        && app.focus == FocusPane::ScopeFiles
    {
        return vec![Action::SelectAllVisibleFiles];
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
        KeyCode::Enter => vec![Action::CommitInput],
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
        assert!(!matches!(actions.as_slice(), [Action::SelectAllVisibleFiles]));
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
        assert!(matches!(actions.as_slice(), [Action::SelectAllVisibleFiles]));
    }
}
