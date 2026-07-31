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
mod md;
mod storage;
mod ui;

use app::{App, Screen};
use md::visual_to_logical;

fn handle_list_key(key: KeyEvent, app: &mut App) {
    match key.code {
        KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => match c {
            'c' | 'q' => app.should_quit = true,
            'n' => app.start_new_note(),
            'd' => app.start_delete(),
            's' => app.start_settings(),
            'r' => app.cycle_sort(),
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
            '1' | '2' | '3' | '4' | '5' | '6' if !app.editor.focus_title => {
                app.editor.set_heading(c.to_digit(10).unwrap_or(0) as u8);
            }
            '0' if !app.editor.focus_title => {
                app.editor.set_heading(0);
            }
            'u' if !app.editor.focus_title => {
                app.editor.toggle_bullet();
            }
            'o' if !app.editor.focus_title => {
                app.editor.toggle_ordered();
            }
            't' if !app.editor.focus_title => {
                app.editor.insert_table();
            }
            _ => {}
        },
        KeyCode::Tab => {
            if app.editor.focus_title || !app.editor.tab() {
                app.editor.focus_title = !app.editor.focus_title;
            }
        }
        KeyCode::BackTab => {
            if !app.editor.focus_title {
                app.editor.shift_tab();
            }
        }
        KeyCode::Enter => {
            if app.editor.focus_title {
                app.editor.focus_title = false;
            } else {
                app.editor.insert_newline_smart();
            }
        }
        KeyCode::Backspace => {
            if app.editor.focus_title {
                app.editor.title_backspace();
            } else {
                app.editor.content_backspace();
            }
        }
        KeyCode::Delete => {
            if app.editor.focus_title {
                app.editor.title_delete();
            } else {
                app.editor.content_delete();
            }
        }
        KeyCode::Left => {
            if app.editor.focus_title {
                app.editor.title_move_left();
            } else {
                app.editor.move_cursor_left();
            }
        }
        KeyCode::Right => {
            if app.editor.focus_title {
                app.editor.title_move_right();
            } else {
                app.editor.move_cursor_right();
            }
        }
        KeyCode::Up => {
            if key.modifiers.contains(KeyModifiers::ALT) && !app.editor.focus_title {
                app.editor.move_line_up();
            } else if app.editor.focus_title {
                // do nothing, already at the top field
            } else if app.editor.cursor_line == 0 {
                app.editor.focus_title = true;
            } else {
                app.editor.move_cursor_up();
            }
        }
        KeyCode::Down => {
            if key.modifiers.contains(KeyModifiers::ALT) && !app.editor.focus_title {
                app.editor.move_line_down();
            } else if app.editor.focus_title {
                app.editor.focus_title = false;
            } else {
                app.editor.move_cursor_down();
            }
        }
        KeyCode::Home => {
            if app.editor.focus_title {
                app.editor.title_cursor = 0;
            } else {
                app.editor.move_cursor_home();
            }
        }
        KeyCode::End => {
            if app.editor.focus_title {
                app.editor.title_cursor = app.editor.title.len();
            } else {
                app.editor.move_cursor_end();
            }
        }
        KeyCode::Esc => {
            app.screen = Screen::List;
        }
        KeyCode::Char(c) => {
            if app.editor.focus_title {
                app.editor.title_insert_char(c);
            } else {
                app.editor.insert_char(c);
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
    if row > title_area.y && row < title_area.y + title_area.height - 1
        && col > title_area.x && col < title_area.x + title_area.width - 1
    {
        app.editor.focus_title = true;
        let title_col = (col - title_area.x - 1) as usize;
        let title_inner = title_area.width.saturating_sub(2) as usize;
        app.editor.title_cursor = visual_to_logical(&app.editor.title, title_col, None, title_inner);
        return;
    }

    // Check if click is in content area (inside border)
    if row > content_area.y && row < content_area.y + content_area.height - 1
        && col > content_area.x && col < content_area.x + content_area.width - 1
    {
        app.editor.focus_title = false;
        let content_line = (row - content_area.y - 1) as usize;
        let content_col = (col - content_area.x - 1) as usize;
        app.editor.cursor_line = (app.editor.scroll_offset + content_line).min(app.editor.lines.len().saturating_sub(1));
        let content_width = content_area.width.saturating_sub(2) as usize;
        let widths = md::table_block_widths(&app.editor.lines, app.editor.cursor_line);
        app.editor.cursor_col = visual_to_logical(
            &app.editor.lines[app.editor.cursor_line],
            content_col,
            widths.as_deref(),
            content_width,
        );
    }
}

fn handle_settings_key(key: KeyEvent, app: &mut App) {
    match key.code {
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.confirm_settings();
        }
        KeyCode::Enter => app.confirm_settings(),
        KeyCode::Esc => app.screen = Screen::List,
        KeyCode::Left => app.settings.move_left(),
        KeyCode::Right => app.settings.move_right(),
        KeyCode::Home => app.settings.home(),
        KeyCode::End => app.settings.end(),
        KeyCode::Backspace => app.settings.backspace(),
        KeyCode::Delete => app.settings.delete(),
        KeyCode::Char(c) => app.settings.insert_char(c),
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
        Screen::Settings => handle_settings_key(key, app),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("notes_rs_main_{}_{}", std::process::id(), name))
    }

    fn setup() {
        std::env::set_var("NOTRS_DATA_FILE", temp_file("data.json"));
        std::env::set_var("NOTRS_CONFIG_FILE", temp_file("config.json"));
        let _ = std::fs::remove_file(temp_file("data.json"));
        let _ = std::fs::remove_file(temp_file("config.json"));
    }

    fn teardown() {
        let _ = std::fs::remove_file(temp_file("data.json"));
        let _ = std::fs::remove_file(temp_file("config.json"));
        std::env::remove_var("NOTRS_DATA_FILE");
        std::env::remove_var("NOTRS_CONFIG_FILE");
    }

    #[test]
    fn test_ctrl_r_cycles_sort_from_list() {
        let _guard = storage::tests::lock();
        setup();

        let mut app = App::new();
        assert_eq!(app.sort_order, app::SortOrder::UpdatedDesc);
        handle_list_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL), &mut app);
        assert_eq!(app.sort_order, app::SortOrder::UpdatedAsc);
        assert!(app.status_message.is_some());

        teardown();
    }
}
