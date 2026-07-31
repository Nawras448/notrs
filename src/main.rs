use std::io;
use std::panic;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

mod app;
mod storage;
mod ui;

use app::{App, Screen};

fn handle_list_key(key: KeyEvent, app: &mut App) {
    match key.code {
        KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => match c {
            'c' | 'q' => app.should_quit = true,
            'n' => app.start_new_note(),
            'd' => app.start_delete(),
            _ => {}
        },
        KeyCode::Char(c) => match c {
            'q' => app.should_quit = true,
            'n' => app.start_new_note(),
            'j' | 'J' => {
                if app.selected_index + 1 < app.filtered_notes.len() {
                    app.selected_index += 1;
                }
            }
            'k' | 'K' => {
                if app.selected_index > 0 {
                    app.selected_index = app.selected_index.saturating_sub(1);
                }
            }
            'g' => app.selected_index = 0,
            'G' => app.selected_index = app.filtered_notes.len().saturating_sub(1),
            c => {
                app.search_query.push(c);
                app.apply_search();
            }
        },
        KeyCode::Up => {
            if app.selected_index > 0 {
                app.selected_index -= 1;
            }
        }
        KeyCode::Down => {
            if app.selected_index + 1 < app.filtered_notes.len() {
                app.selected_index += 1;
            }
        }
        KeyCode::Enter => {
            if let Some(note) = app.selected_note().cloned() {
                app.start_edit(&note);
            }
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            app.apply_search();
        }
        KeyCode::Esc => {
            if !app.search_query.is_empty() {
                app.search_query.clear();
                app.apply_search();
            } else {
                app.should_quit = true;
            }
        }
        _ => {}
    }
}

fn handle_editor_key(key: KeyEvent, app: &mut App) {
    match key.code {
        KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => match c {
            's' => app.save_note(),
            'd' => {
                if matches!(app.screen, Screen::Editor(Some(_))) {
                    app.start_delete();
                }
            }
            _ => {}
        },
        KeyCode::Tab => {
            app.editor_focus_title = !app.editor_focus_title;
        }
        KeyCode::Enter => {
            if app.editor_focus_title {
                app.editor_focus_title = false;
            } else {
                app.editor_content.push('\n');
            }
        }
        KeyCode::Backspace => {
            if app.editor_focus_title {
                app.editor_title.pop();
            } else {
                app.editor_content.pop();
            }
        }
        KeyCode::Esc => {
            app.screen = Screen::List;
        }
        KeyCode::Char(c) => {
            if app.editor_focus_title {
                app.editor_title.push(c);
            } else {
                app.editor_content.push(c);
            }
        }
        _ => {}
    }
}

fn handle_confirm_key(key: KeyEvent, app: &mut App) {
    match key.code {
        KeyCode::Char('y' | 'Y') | KeyCode::Enter => app.confirm_delete(),
        KeyCode::Char('n' | 'N') | KeyCode::Esc => {
            app.screen = Screen::List;
        }
        _ => {}
    }
}

fn handle_key(key: KeyEvent, app: &mut App) {
    match app.screen {
        Screen::List => handle_list_key(key, app),
        Screen::Editor(_) => handle_editor_key(key, app),
        Screen::ConfirmDelete(_) => handle_confirm_key(key, app),
    }
}

fn init_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    Terminal::new(backend)
}

fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn run() -> io::Result<()> {
    let mut terminal = init_terminal()?;
    let mut app = App::new();

    while !app.should_quit {
        terminal.draw(|f| ui::render(f, &app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                handle_key(key, &mut app);
            }
        }
    }

    restore_terminal()?;
    Ok(())
}

fn main() {
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_terminal();
        original_hook(panic_info);
    }));

    if let Err(e) = run() {
        let _ = restore_terminal();
        eprintln!("Error: {e}");
    }
}
