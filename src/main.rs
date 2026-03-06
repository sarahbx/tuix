/// tuix — Terminal session multiplexer TUI.
///
/// Manages N concurrent terminal sessions in a tiled overview with
/// one-action switching to full interactive mode.
///
/// Usage: tuix <session>... [--env KEY=VALUE]
/// Sessions: "command@path" or "command" (uses cwd)
///
/// Controls:
///   Tile view:  Click tile or press Enter to focus
///               Arrow keys / Tab to navigate
///               0-9 to focus by index
///               Ctrl+b to toggle blur (SEC-003)
///               Ctrl+h to toggle help screen
///               Ctrl+q to quit
///   Focus view: Ctrl+] or click [X] to unfocus
///               All other input forwarded to session

use clap::Parser;
use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use std::io;
use tuix::{app, config};

fn main() {
    let config = config::Config::parse();

    // Validate sessions before entering TUI mode so errors are
    // visible on the normal terminal (not swallowed by raw mode).
    let defs = match config::validate(&config) {
        Ok(defs) => defs,
        Err(e) => {
            eprintln!("tuix: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = run(defs, config.scrollback) {
        eprintln!("tuix: {e}");
        std::process::exit(1);
    }
}

fn run(defs: Vec<config::SessionDef>, scrollback: usize) -> Result<(), String> {
    // AUD-003: enable_raw_mode is called first; cleanup always runs
    // regardless of where subsequent setup or execution fails.
    enable_raw_mode().map_err(|e| format!("raw mode: {e}"))?;

    let result = run_inner(defs, scrollback);

    // Restore terminal (always, even on error — AUD-003)
    let _ = disable_raw_mode();
    let _ = ratatui::crossterm::execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);

    result
}

fn run_inner(defs: Vec<config::SessionDef>, scrollback: usize) -> Result<(), String> {
    let mut stdout = io::stdout();
    ratatui::crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .map_err(|e| format!("terminal setup: {e}"))?;

    let backend = ratatui::backend::CrosstermBackend::new(io::stdout());
    let mut terminal = ratatui::Terminal::new(backend)
        .map_err(|e| format!("terminal init: {e}"))?;

    app::App::new(defs, scrollback, &terminal)
        .and_then(|mut app| app.run(&mut terminal))
}
