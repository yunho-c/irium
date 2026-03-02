mod actions;
mod ai;
mod app;
mod config;
mod fs_scan;
mod input;
mod mock;
mod model;
mod theme;
mod ui;

use std::{io, panic, time::Duration};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, prelude::CrosstermBackend};

use crate::{
    actions::{Action, reduce},
    input::map_event,
    model::AppState,
};

type Tui = Terminal<CrosstermBackend<io::Stdout>>;

fn main() -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    install_panic_hook();
    let mut app = AppState::new(std::env::current_dir()?);
    let result = run_app(&mut terminal, &mut app);
    app.shutdown_ai_worker();
    restore_terminal(&mut terminal)?;
    result
}

fn setup_terminal() -> io::Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    Terminal::new(CrosstermBackend::new(stdout))
}

fn restore_terminal(terminal: &mut Tui) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn install_panic_hook() {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, DisableMouseCapture, LeaveAlternateScreen);
        default_hook(panic_info);
    }));
}

fn run_app(terminal: &mut Tui, app: &mut AppState) -> io::Result<()> {
    let tick_rate = Duration::from_millis(120);
    while !app.should_quit {
        terminal.draw(|frame| ui::draw(frame, app))?;

        if event::poll(tick_rate)? {
            let event = event::read()?;
            let actions = map_event(app, event);
            for action in actions {
                reduce(app, action);
                if app.should_quit {
                    break;
                }
            }
        } else {
            reduce(app, Action::Tick);
        }
    }

    Ok(())
}
