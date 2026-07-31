use crate::storage::{self, Note};

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    List,
    Editor(Option<Note>),
    ConfirmDelete(String),
    Settings,
}

pub struct App {
    pub notes: Vec<Note>,
    pub filtered_notes: Vec<Note>,
    pub selected_index: usize,
    pub screen: Screen,
    pub search_query: String,
    pub editor_title: Vec<char>,
    pub editor_lines: Vec<Vec<char>>,
    pub editor_focus_title: bool,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub title_cursor: usize,
    pub scroll_offset: usize,
    pub settings_input: Vec<char>,
    pub settings_cursor: usize,
    pub settings_error: Option<String>,
    pub error_message: Option<String>,
    pub status_message: Option<String>,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        storage::ensure_configured_path();
        let notes = storage::load_notes();
        let filtered_notes = notes.clone();
        Self {
            notes,
            filtered_notes,
            selected_index: 0,
            screen: Screen::List,
            search_query: String::new(),
            editor_title: Vec::new(),
            editor_lines: vec![Vec::new()],
            editor_focus_title: true,
            cursor_line: 0,
            cursor_col: 0,
            title_cursor: 0,
            scroll_offset: 0,
            settings_input: Vec::new(),
            settings_cursor: 0,
            settings_error: None,
            error_message: None,
            status_message: None,
            should_quit: false,
        }
    }

    pub fn refresh(&mut self) {
        self.notes = storage::load_notes();
        self.apply_search();
    }

    pub fn apply_search(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_notes = self.notes.clone();
        } else {
            self.filtered_notes = storage::search_notes(&self.search_query);
        }
        if self.selected_index >= self.filtered_notes.len() {
            self.selected_index = self.filtered_notes.len().saturating_sub(1);
        }
    }

    pub fn selected_note(&self) -> Option<&Note> {
        self.filtered_notes.get(self.selected_index)
    }

    pub fn start_new_note(&mut self) {
        self.error_message = None;
        self.status_message = None;
        self.editor_title.clear();
        self.editor_lines = vec![Vec::new()];
        self.cursor_line = 0;
        self.cursor_col = 0;
        self.title_cursor = 0;
        self.scroll_offset = 0;
        self.editor_focus_title = true;
        self.screen = Screen::Editor(None);
    }

    pub fn start_edit(&mut self, note: &Note) {
        self.error_message = None;
        self.status_message = None;
        self.editor_title = note.title.chars().collect();
        self.editor_lines = note
            .content
            .split('\n')
            .map(|s| s.chars().collect())
            .collect();
        if self.editor_lines.is_empty() {
            self.editor_lines = vec![Vec::new()];
        }
        self.cursor_line = 0;
        self.cursor_col = 0;
        self.title_cursor = 0;
        self.scroll_offset = 0;
        self.editor_focus_title = true;
        self.screen = Screen::Editor(Some(note.clone()));
    }

    pub fn save_note(&mut self) {
        let title: String = self.editor_title.iter().collect();
        let title = title.trim().to_string();
        if title.is_empty() {
            return;
        }
        let content: String = self
            .editor_lines
            .iter()
            .map(|line| line.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        match &self.screen {
            Screen::Editor(Some(note)) => {
                storage::update_note(&note.id, &title, &content)
            }
            _ => storage::add_note(&title, &content),
        }
        .map_err(|e| format!("Could not save note: {e}"))
        .map(|_| {
            self.error_message = None;
            self.status_message = Some("Note saved.".to_string());
            self.refresh();
            self.screen = Screen::List;
        })
        .unwrap_or_else(|msg| {
            self.error_message = Some(msg);
        });
    }

    pub fn start_delete(&mut self) {
        self.status_message = None;
        if let Some(note) = self.selected_note().cloned() {
            self.screen = Screen::ConfirmDelete(note.title.clone());
        }
    }

    pub fn confirm_delete(&mut self) {
        if let Screen::ConfirmDelete(title) = &self.screen {
            if let Some(note) = self.notes.iter().find(|n| n.title == *title) {
                if let Err(e) = storage::delete_note(&note.id) {
                    self.error_message = Some(format!("Could not delete note: {e}"));
                }
            }
        }
        self.refresh();
        self.screen = Screen::List;
    }

    // --- Settings ---

    pub fn start_settings(&mut self) {
        self.error_message = None;
        self.status_message = None;
        self.settings_input = storage::display_notes_path().chars().collect();
        self.settings_cursor = self.settings_input.len();
        self.settings_error = None;
        self.screen = Screen::Settings;
    }

    pub fn settings_move_left(&mut self) {
        if self.settings_cursor > 0 {
            self.settings_cursor -= 1;
        }
    }

    pub fn settings_move_right(&mut self) {
        if self.settings_cursor < self.settings_input.len() {
            self.settings_cursor += 1;
        }
    }

    pub fn settings_home(&mut self) {
        self.settings_cursor = 0;
    }

    pub fn settings_end(&mut self) {
        self.settings_cursor = self.settings_input.len();
    }

    pub fn settings_insert_char(&mut self, c: char) {
        self.settings_input.insert(self.settings_cursor, c);
        self.settings_cursor += 1;
    }

    pub fn settings_backspace(&mut self) {
        if self.settings_cursor > 0 {
            self.settings_cursor -= 1;
            self.settings_input.remove(self.settings_cursor);
        }
    }

    pub fn settings_delete(&mut self) {
        if self.settings_cursor < self.settings_input.len() {
            self.settings_input.remove(self.settings_cursor);
        }
    }

    pub fn confirm_settings(&mut self) {
        let raw: String = self.settings_input.iter().collect();
        let trimmed = raw.trim();
        let new_path = if trimmed.is_empty() {
            None
        } else {
            let normalized = storage::normalize_notes_path(trimmed);
            if !normalized.is_absolute() {
                self.settings_error = Some(
                    "Path must be absolute (e.g. /home/you/notes.json)".to_string(),
                );
                return;
            }
            Some(normalized)
        };
        match storage::set_notes_path(new_path) {
            Ok(effective) => {
                self.settings_error = None;
                self.error_message = None;
                self.status_message = Some(format!(
                    "Notes file: {}",
                    storage::display_path(&effective)
                ));
                self.refresh();
                self.screen = Screen::List;
            }
            Err(e) => {
                self.settings_error = Some(format!("Could not save settings: {e}"));
            }
        }
    }

    // --- Cursor movement ---

    pub fn move_cursor_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
            let len = self.editor_lines[self.cursor_line].len();
            if self.cursor_col > len {
                self.cursor_col = len;
            }
        }
    }

    pub fn move_cursor_down(&mut self) {
        if self.cursor_line + 1 < self.editor_lines.len() {
            self.cursor_line += 1;
            let len = self.editor_lines[self.cursor_line].len();
            if self.cursor_col > len {
                self.cursor_col = len;
            }
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.editor_lines[self.cursor_line].len();
        }
    }

    pub fn move_cursor_right(&mut self) {
        let line_len = self.editor_lines[self.cursor_line].len();
        if self.cursor_col < line_len {
            self.cursor_col += 1;
        } else if self.cursor_line + 1 < self.editor_lines.len() {
            self.cursor_line += 1;
            self.cursor_col = 0;
        }
    }

    pub fn move_cursor_home(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor_col = self.editor_lines[self.cursor_line].len();
    }

    // --- Title editing ---

    pub fn title_move_left(&mut self) {
        if self.title_cursor > 0 {
            self.title_cursor -= 1;
        }
    }

    pub fn title_move_right(&mut self) {
        if self.title_cursor < self.editor_title.len() {
            self.title_cursor += 1;
        }
    }

    pub fn title_insert_char(&mut self, c: char) {
        self.editor_title.insert(self.title_cursor, c);
        self.title_cursor += 1;
    }

    pub fn title_backspace(&mut self) {
        if self.title_cursor > 0 {
            self.title_cursor -= 1;
            self.editor_title.remove(self.title_cursor);
        }
    }

    pub fn title_delete(&mut self) {
        if self.title_cursor < self.editor_title.len() {
            self.editor_title.remove(self.title_cursor);
        }
    }

    // --- Content editing ---

    pub fn insert_char(&mut self, c: char) {
        self.editor_lines[self.cursor_line].insert(self.cursor_col, c);
        self.cursor_col += 1;
    }

    pub fn content_backspace(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
            self.editor_lines[self.cursor_line].remove(self.cursor_col);
        } else if self.cursor_line > 0 {
            let prev_len = self.editor_lines[self.cursor_line - 1].len();
            let current = self.editor_lines.remove(self.cursor_line);
            self.editor_lines[self.cursor_line - 1].extend(current);
            self.cursor_line -= 1;
            self.cursor_col = prev_len;
        }
    }

    pub fn content_delete(&mut self) {
        let line_len = self.editor_lines[self.cursor_line].len();
        if self.cursor_col < line_len {
            self.editor_lines[self.cursor_line].remove(self.cursor_col);
        } else if self.cursor_line + 1 < self.editor_lines.len() {
            let next = self.editor_lines.remove(self.cursor_line + 1);
            self.editor_lines[self.cursor_line].extend(next);
        }
    }

    pub fn insert_newline(&mut self) {
        let current = &mut self.editor_lines[self.cursor_line];
        let rest = current.split_off(self.cursor_col);
        self.cursor_line += 1;
        self.cursor_col = 0;
        self.editor_lines.insert(self.cursor_line, rest);
    }
}
