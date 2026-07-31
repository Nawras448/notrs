use std::io;
use std::panic;
use std::time::Duration;

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::cursor::{self, SetCursorStyle};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
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
                app.insert_newline();
            }
        }
        KeyCode::Backspace => {
            if app.editor_focus_title {
                app.title_backspace();
            } else {
                app.content_backspace();
            }
        }
        KeyCode::Delete => {
            if app.editor_focus_title {
                app.title_delete();
            } else {
                app.content_delete();
            }
        }
        KeyCode::Left => {
            if app.editor_focus_title {
                app.title_move_left();
            } else {
                app.move_cursor_left();
            }
        }
        KeyCode::Right => {
            if app.editor_focus_title {
                app.title_move_right();
            } else {
                app.move_cursor_right();
            }
        }
        KeyCode::Up => {
            if app.editor_focus_title {
                // do nothing, already at the top field
            } else if app.cursor_line == 0 {
                app.editor_focus_title = true;
            } else {
                app.move_cursor_up();
            }
        }
        KeyCode::Down => {
            if app.editor_focus_title {
                app.editor_focus_title = false;
            } else {
                app.move_cursor_down();
            }
        }
        KeyCode::Home => {
            if app.editor_focus_title {
                app.title_cursor = 0;
            } else {
                app.move_cursor_home();
            }
        }
        KeyCode::End => {
            if app.editor_focus_title {
                app.title_cursor = app.editor_title.len();
            } else {
                app.move_cursor_end();
            }
        }
        KeyCode::Esc => {
            app.screen = Screen::List;
        }
        KeyCode::Char(c) => {
            if app.editor_focus_title {
                app.title_insert_char(c);
            } else {
                app.insert_char(c);
            }
        }
        _ => {}
    }
}

fn handle_editor_mouse(event: MouseEvent, app: &mut App, terminal_width: u16, terminal_height: u16) {
    if event.kind != MouseEventKind::Down(MouseButton::Left) {
        return;
    }
    let area = ratatui::layout::Rect::new(0, 0, terminal_width, terminal_height);
    let layout = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(1),
    ]);
    let rows = layout.split(area);
    let title_area = rows[1];
    let content_area = rows[2];

    let col = event.column;
    let row = event.row;

    // Check if click is in title area (inside border)
    if row >= title_area.y + 1 && row < title_area.y + title_area.height - 1
        && col >= title_area.x + 1 && col < title_area.x + title_area.width - 1
    {
        app.editor_focus_title = true;
        let title_col = (col - title_area.x - 1) as usize;
        app.title_cursor = title_col.min(app.editor_title.len());
        return;
    }

    // Check if click is in content area (inside border)
    if row >= content_area.y + 1 && row < content_area.y + content_area.height - 1
        && col >= content_area.x + 1 && col < content_area.x + content_area.width - 1
    {
        app.editor_focus_title = false;
        let content_line = (row - content_area.y - 1) as usize;
        let content_col = (col - content_area.x - 1) as usize;
        app.cursor_line = (app.scroll_offset + content_line).min(app.editor_lines.len().saturating_sub(1));
        let line_len = app.editor_lines[app.cursor_line].len();
        app.cursor_col = content_col.min(line_len);
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
    io::stdout().execute(EnableMouseCapture)?;
    io::stdout().execute(cursor::Show)?;
    io::stdout().execute(SetCursorStyle::BlinkingBlock)?;
    let backend = CrosstermBackend::new(io::stdout());
    Terminal::new(backend)
}

fn restore_terminal() -> io::Result<()> {
    io::stdout().execute(DisableMouseCapture)?;
    io::stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

fn run() -> io::Result<()> {
    let mut terminal = init_terminal()?;
    let mut app = App::new();

    while !app.should_quit {
        terminal.draw(|f| ui::render(f, &mut app))?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => handle_key(key, &mut app),
                Event::Mouse(mouse) => {
                    if matches!(app.screen, Screen::Editor(_)) {
                        let size = terminal.size()?;
                        let (width, height) = (size.width, size.height);
                        handle_editor_mouse(mouse, &mut app, width, height);
                    }
                }
                _ => {}
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
