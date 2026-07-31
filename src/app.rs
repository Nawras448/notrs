use crate::md;
use crate::storage::{self, Note};

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    List,
    Editor(Option<Note>),
    ConfirmDelete(String),
    Settings,
}

pub struct Editor {
    pub title: Vec<char>,
    pub lines: Vec<Vec<char>>,
    pub focus_title: bool,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub title_cursor: usize,
    pub scroll_offset: usize,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            title: Vec::new(),
            lines: vec![Vec::new()],
            focus_title: true,
            cursor_line: 0,
            cursor_col: 0,
            title_cursor: 0,
            scroll_offset: 0,
        }
    }

    fn reset_position(&mut self) {
        self.cursor_line = 0;
        self.cursor_col = 0;
        self.title_cursor = 0;
        self.scroll_offset = 0;
        self.focus_title = true;
    }

    pub fn load(&mut self, title: &str, content: &str) {
        self.title = title.chars().collect();
        self.lines = content.split('\n').map(|s| s.chars().collect()).collect();
        if self.lines.is_empty() {
            self.lines = vec![Vec::new()];
        }
        self.reset_position();
    }

    pub fn title_text(&self) -> String {
        self.title.iter().collect()
    }

    pub fn content_text(&self) -> String {
        self.lines
            .iter()
            .map(|line| line.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    // --- Cursor movement ---

    pub fn move_cursor_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
            let len = self.lines[self.cursor_line].len();
            if self.cursor_col > len {
                self.cursor_col = len;
            }
        }
    }

    pub fn move_cursor_down(&mut self) {
        if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            let len = self.lines[self.cursor_line].len();
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
            self.cursor_col = self.lines[self.cursor_line].len();
        }
    }

    pub fn move_cursor_right(&mut self) {
        let line_len = self.lines[self.cursor_line].len();
        if self.cursor_col < line_len {
            self.cursor_col += 1;
        } else if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            self.cursor_col = 0;
        }
    }

    pub fn move_cursor_home(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor_col = self.lines[self.cursor_line].len();
    }

    // --- Title editing ---

    pub fn title_move_left(&mut self) {
        if self.title_cursor > 0 {
            self.title_cursor -= 1;
        }
    }

    pub fn title_move_right(&mut self) {
        if self.title_cursor < self.title.len() {
            self.title_cursor += 1;
        }
    }

    pub fn title_insert_char(&mut self, c: char) {
        self.title.insert(self.title_cursor, c);
        self.title_cursor += 1;
    }

    pub fn title_backspace(&mut self) {
        if self.title_cursor > 0 {
            self.title_cursor -= 1;
            self.title.remove(self.title_cursor);
        }
    }

    pub fn title_delete(&mut self) {
        if self.title_cursor < self.title.len() {
            self.title.remove(self.title_cursor);
        }
    }

    // --- Content editing ---

    pub fn insert_char(&mut self, c: char) {
        self.lines[self.cursor_line].insert(self.cursor_col, c);
        self.cursor_col += 1;
    }

    pub fn content_backspace(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
            self.lines[self.cursor_line].remove(self.cursor_col);
        } else if self.cursor_line > 0 {
            let prev_len = self.lines[self.cursor_line - 1].len();
            let current = self.lines.remove(self.cursor_line);
            self.lines[self.cursor_line - 1].extend(current);
            self.cursor_line -= 1;
            self.cursor_col = prev_len;
        }
    }

    pub fn content_delete(&mut self) {
        let line_len = self.lines[self.cursor_line].len();
        if self.cursor_col < line_len {
            self.lines[self.cursor_line].remove(self.cursor_col);
        } else if self.cursor_line + 1 < self.lines.len() {
            let next = self.lines.remove(self.cursor_line + 1);
            self.lines[self.cursor_line].extend(next);
        }
    }

    pub fn insert_newline(&mut self) {
        let current = &mut self.lines[self.cursor_line];
        let rest = current.split_off(self.cursor_col);
        self.cursor_line += 1;
        self.cursor_col = 0;
        self.lines.insert(self.cursor_line, rest);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListKind {
    None,
    Bullet,
    Ordered,
    Checkbox,
}

#[derive(Debug, Clone, Copy)]
struct ListInfo {
    kind: ListKind,
    indent: usize,
    marker_len: usize,
    num: Option<usize>,
    checked: Option<bool>,
}

fn list_info(line: &[char]) -> ListInfo {
    let indent = line
        .iter()
        .position(|c| !c.is_whitespace())
        .unwrap_or(line.len());
    let rest = &line[indent..];
    if rest.is_empty() {
        return ListInfo {
            kind: ListKind::None,
            indent,
            marker_len: 0,
            num: None,
            checked: None,
        };
    }
    let is_bullet_char = matches!(rest[0], '-' | '*' | '+');
    if is_bullet_char
        && rest.get(1) == Some(&' ')
        && rest.get(2) == Some(&'[')
        && rest.get(4) == Some(&']')
        && rest.get(5) == Some(&' ')
        && matches!(rest.get(3), Some(' ') | Some('x') | Some('X'))
    {
        return ListInfo {
            kind: ListKind::Checkbox,
            indent,
            marker_len: indent + 6,
            num: None,
            checked: Some(rest[3] != ' '),
        };
    }
    if is_bullet_char && rest.get(1) == Some(&' ') {
        return ListInfo {
            kind: ListKind::Bullet,
            indent,
            marker_len: indent + 2,
            num: None,
            checked: None,
        };
    }
    if rest[0].is_ascii_digit() {
        let mut j = 1;
        while j < rest.len() && rest[j].is_ascii_digit() {
            j += 1;
        }
        if j < rest.len() && matches!(rest[j], '.' | ')') && rest.get(j + 1) == Some(&' ') {
            let num = rest[..j].iter().collect::<String>().parse().unwrap_or(0);
            return ListInfo {
                kind: ListKind::Ordered,
                indent,
                marker_len: indent + j + 2,
                num: Some(num),
                checked: None,
            };
        }
    }
    ListInfo {
        kind: ListKind::None,
        indent,
        marker_len: 0,
        num: None,
        checked: None,
    }
}

fn heading_prefix_len(line: &[char]) -> usize {
    let mut n = 0;
    while n < line.len() && n < 6 && line[n] == '#' {
        n += 1;
    }
    if n == 0 {
        return 0;
    }
    if line.get(n) == Some(&' ') {
        n + 1
    } else {
        n
    }
}

impl Editor {
    fn replace_prefix(&mut self, marker_len: usize, new_prefix: &str) {
        let line = self.lines[self.cursor_line].clone();
        let rest: Vec<char> = line[marker_len.min(line.len())..].to_vec();
        let mut new_line: Vec<char> = new_prefix.chars().collect();
        new_line.extend(rest);
        let new_len = new_prefix.chars().count();
        self.lines[self.cursor_line] = new_line;
        self.cursor_col = self
            .cursor_col
            .saturating_sub(marker_len)
            .saturating_add(new_len)
            .min(self.lines[self.cursor_line].len());
    }

    pub fn set_heading(&mut self, level: u8) {
        let line = self.lines[self.cursor_line].clone();
        let h = heading_prefix_len(&line);
        let body: Vec<char> = line[h..].to_vec();
        let prefix: String = if level > 0 {
            format!("{} ", "#".repeat(level as usize))
        } else {
            String::new()
        };
        let new_prefix_len = prefix.chars().count();
        let rel = self.cursor_col.saturating_sub(h);
        let mut new_line: Vec<char> = prefix.chars().collect();
        new_line.extend(body);
        let len = new_line.len();
        self.lines[self.cursor_line] = new_line;
        self.cursor_col = (new_prefix_len + rel).min(len);
    }

    pub fn toggle_bullet(&mut self) {
        let info = list_info(&self.lines[self.cursor_line]);
        match info.kind {
            ListKind::Checkbox => {
                let i = info.indent + 3;
                let checked = info.checked.unwrap_or(false);
                self.lines[self.cursor_line][i] = if checked { ' ' } else { 'x' };
                self.cursor_col = info.indent + 6;
            }
            ListKind::Bullet => self.replace_prefix(info.marker_len, ""),
            ListKind::Ordered => {
                let prefix = format!("{}- ", " ".repeat(info.indent));
                self.replace_prefix(info.marker_len, &prefix);
            }
            ListKind::None => {
                let prefix = format!("{}- ", " ".repeat(info.indent));
                self.replace_prefix(0, &prefix);
            }
        }
    }

    pub fn toggle_ordered(&mut self) {
        let info = list_info(&self.lines[self.cursor_line]);
        match info.kind {
            ListKind::Ordered => self.replace_prefix(info.marker_len, ""),
            ListKind::Bullet | ListKind::Checkbox => {
                let prefix = format!("{}1. ", " ".repeat(info.indent));
                self.replace_prefix(info.marker_len, &prefix);
            }
            ListKind::None => {
                let prefix = format!("{}1. ", " ".repeat(info.indent));
                self.replace_prefix(0, &prefix);
            }
        }
    }

    pub fn insert_table(&mut self) {
        let rows: Vec<Vec<char>> = ["| a | b |", "|---|---|", "|   |   |"]
            .iter()
            .map(|s| s.chars().collect())
            .collect();
        let line_idx = self.cursor_line;
        self.lines.splice(line_idx..line_idx, rows);
        self.cursor_line = line_idx + 2;
        self.cursor_col = 2;
    }

    fn table_cell_positions(&self, line_idx: usize) -> Vec<usize> {
        let line = &self.lines[line_idx];
        let cells = md::table_cells(line);
        cells
            .into_iter()
            .map(|span| {
                (span.raw_start..span.raw_start + span.raw_len)
                    .find(|&i| !line[i].is_whitespace())
                    .unwrap_or(span.raw_start + span.raw_len)
            })
            .collect()
    }

    fn table_next_cell(&mut self) -> bool {
        let positions = self.table_cell_positions(self.cursor_line);
        let cur = self.cursor_col;
        match positions.iter().position(|&p| p > cur) {
            Some(i) => {
                self.cursor_col = positions[i];
                true
            }
            None => {
                if self.cursor_line + 1 < self.lines.len()
                    && md::is_table_row(&self.lines[self.cursor_line + 1])
                {
                    self.cursor_line += 1;
                    let p = self.table_cell_positions(self.cursor_line);
                    self.cursor_col = p.first().copied().unwrap_or(0);
                    true
                } else {
                    false
                }
            }
        }
    }

    fn table_prev_cell(&mut self) -> bool {
        let positions = self.table_cell_positions(self.cursor_line);
        let cur = self.cursor_col;
        match positions.iter().rev().find(|&&p| p < cur) {
            Some(&p) => {
                self.cursor_col = p;
                true
            }
            None => {
                if self.cursor_line > 0 && md::is_table_row(&self.lines[self.cursor_line - 1]) {
                    self.cursor_line -= 1;
                    let p = self.table_cell_positions(self.cursor_line);
                    self.cursor_col = p.last().copied().unwrap_or(0);
                    true
                } else {
                    false
                }
            }
        }
    }

    pub fn tab(&mut self) -> bool {
        let info = list_info(&self.lines[self.cursor_line]);
        match info.kind {
            ListKind::Bullet | ListKind::Ordered | ListKind::Checkbox => {
                self.lines[self.cursor_line].insert(0, ' ');
                self.lines[self.cursor_line].insert(0, ' ');
                self.cursor_col += 2;
                true
            }
            ListKind::None if md::is_table_row(&self.lines[self.cursor_line]) => {
                self.table_next_cell()
            }
            _ => false,
        }
    }

    pub fn shift_tab(&mut self) -> bool {
        let info = list_info(&self.lines[self.cursor_line]);
        match info.kind {
            ListKind::Bullet | ListKind::Ordered | ListKind::Checkbox => {
                let mut removed = 0;
                let line = &mut self.lines[self.cursor_line];
                while removed < 2 && line.first() == Some(&' ') {
                    line.remove(0);
                    removed += 1;
                }
                self.cursor_col = self.cursor_col.saturating_sub(removed);
                true
            }
            ListKind::None if md::is_table_row(&self.lines[self.cursor_line]) => {
                self.table_prev_cell()
            }
            _ => false,
        }
    }

    pub fn move_line_up(&mut self) {
        if self.cursor_line > 0 {
            let i = self.cursor_line;
            self.lines.swap(i, i - 1);
            self.cursor_line -= 1;
        }
    }

    pub fn move_line_down(&mut self) {
        if self.cursor_line + 1 < self.lines.len() {
            let i = self.cursor_line;
            self.lines.swap(i, i + 1);
            self.cursor_line += 1;
        }
    }

    fn renumber_ordered_from(&mut self, start: usize, indent: usize, next_num: usize) {
        let mut start = start;
        let mut next_num = next_num;
        while start < self.lines.len() {
            let li = list_info(&self.lines[start]);
            if li.kind != ListKind::Ordered || li.indent != indent {
                break;
            }
            let marker_len = li.marker_len;
            let rest: Vec<char> = self.lines[start][marker_len..].to_vec();
            let mut new_line: Vec<char> =
                " ".repeat(indent).chars().chain(format!("{}. ", next_num).chars()).collect();
            new_line.extend(rest);
            self.lines[start] = new_line;
            next_num += 1;
            start += 1;
        }
    }

    pub fn insert_newline_smart(&mut self) {
        let info = list_info(&self.lines[self.cursor_line]);
        match info.kind {
            ListKind::None => self.insert_newline(),
            ListKind::Bullet => {
                let rest_is_empty = self.lines[self.cursor_line].len() <= info.marker_len;
                if rest_is_empty {
                    self.lines[self.cursor_line] = Vec::new();
                    self.cursor_col = 0;
                } else {
                    let rest = self.lines[self.cursor_line].split_off(self.cursor_col);
                    let mut new_line: Vec<char> =
                        " ".repeat(info.indent).chars().chain("- ".chars()).collect();
                    new_line.extend(rest);
                    self.cursor_line += 1;
                    self.cursor_col = info.indent + 2;
                    self.lines.insert(self.cursor_line, new_line);
                }
            }
            ListKind::Checkbox => {
                let rest_is_empty = self.lines[self.cursor_line].len() <= info.marker_len;
                if rest_is_empty {
                    self.lines[self.cursor_line] = Vec::new();
                    self.cursor_col = 0;
                } else {
                    let rest = self.lines[self.cursor_line].split_off(self.cursor_col);
                    let mut new_line: Vec<char> = " ".repeat(info.indent)
                        .chars()
                        .chain("- [ ] ".chars())
                        .collect();
                    new_line.extend(rest);
                    self.cursor_line += 1;
                    self.cursor_col = info.indent + 6;
                    self.lines.insert(self.cursor_line, new_line);
                }
            }
            ListKind::Ordered => {
                let rest_is_empty = self.lines[self.cursor_line].len() <= info.marker_len;
                let next_num = info.num.unwrap_or(1) + 1;
                if rest_is_empty {
                    self.lines[self.cursor_line] = Vec::new();
                    self.cursor_col = 0;
                    self.renumber_ordered_from(self.cursor_line + 1, info.indent, 1);
                } else {
                    let rest = self.lines[self.cursor_line].split_off(self.cursor_col);
                    let mut new_line: Vec<char> = " ".repeat(info.indent)
                        .chars()
                        .chain(format!("{}. ", next_num).chars())
                        .collect();
                    new_line.extend(rest);
                    self.cursor_line += 1;
                    self.cursor_col = info.indent + format!("{}. ", next_num).chars().count();
                    self.lines.insert(self.cursor_line, new_line);
                    self.renumber_ordered_from(self.cursor_line + 1, info.indent, next_num + 1);
                }
            }
        }
    }
}
pub struct SettingsState {
    pub input: Vec<char>,
    pub cursor: usize,
    pub error: Option<String>,
}

impl SettingsState {
    pub fn new() -> Self {
        Self {
            input: Vec::new(),
            cursor: 0,
            error: None,
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor < self.input.len() {
            self.cursor += 1;
        }
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.input.len();
    }

    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor, c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.input.remove(self.cursor);
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.input.len() {
            self.input.remove(self.cursor);
        }
    }

    pub fn text(&self) -> String {
        self.input.iter().collect()
    }
}

pub struct App {
    pub notes: Vec<Note>,
    pub filtered_notes: Vec<Note>,
    pub selected_index: usize,
    pub screen: Screen,
    pub search_query: String,
    pub editor: Editor,
    pub settings: SettingsState,
    pub error_message: Option<String>,
    pub status_message: Option<String>,
    pub sort_order: SortOrder,
    pub should_quit: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    UpdatedDesc,
    UpdatedAsc,
    TitleAsc,
    TitleDesc,
}

impl SortOrder {
    fn cycle(self) -> Self {
        match self {
            Self::UpdatedDesc => Self::UpdatedAsc,
            Self::UpdatedAsc => Self::TitleAsc,
            Self::TitleAsc => Self::TitleDesc,
            Self::TitleDesc => Self::UpdatedDesc,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::UpdatedDesc => "last edited",
            Self::UpdatedAsc => "oldest first",
            Self::TitleAsc => "title A-Z",
            Self::TitleDesc => "title Z-A",
        }
    }
}

fn sort_notes(notes: &mut [Note], order: SortOrder) {
    notes.sort_by(|a, b| match order {
        SortOrder::UpdatedDesc => b.updated_at.cmp(&a.updated_at),
        SortOrder::UpdatedAsc => a.updated_at.cmp(&b.updated_at),
        SortOrder::TitleAsc => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
        SortOrder::TitleDesc => b.title.to_lowercase().cmp(&a.title.to_lowercase()),
    });
}

impl App {
    pub fn new() -> Self {
        storage::ensure_configured_path();
        let mut notes = storage::load_notes();
        sort_notes(&mut notes, SortOrder::UpdatedDesc);
        let filtered_notes = notes.clone();
        Self {
            notes,
            filtered_notes,
            selected_index: 0,
            screen: Screen::List,
            search_query: String::new(),
            editor: Editor::new(),
            settings: SettingsState::new(),
            error_message: None,
            status_message: None,
            sort_order: SortOrder::UpdatedDesc,
            should_quit: false,
        }
    }

    pub fn refresh(&mut self) {
        self.notes = storage::load_notes();
        sort_notes(&mut self.notes, self.sort_order);
        self.apply_search();
    }

    pub fn apply_search(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_notes = self.notes.clone();
        } else {
            self.filtered_notes = storage::search_notes(&self.search_query);
        }
        sort_notes(&mut self.filtered_notes, self.sort_order);
        if self.selected_index >= self.filtered_notes.len() {
            self.selected_index = self.filtered_notes.len().saturating_sub(1);
        }
    }

    pub fn cycle_sort(&mut self) {
        self.sort_order = self.sort_order.cycle();
        self.refresh();
        self.selected_index = 0;
        self.status_message = Some(format!("Sorted by {}", self.sort_order.label()));
    }

    pub fn selected_note(&self) -> Option<&Note> {
        self.filtered_notes.get(self.selected_index)
    }

    pub fn start_new_note(&mut self) {
        self.error_message = None;
        self.status_message = None;
        self.editor = Editor::new();
        self.screen = Screen::Editor(None);
    }

    pub fn start_edit(&mut self, note: &Note) {
        self.error_message = None;
        self.status_message = None;
        self.editor.load(&note.title, &note.content);
        self.screen = Screen::Editor(Some(note.clone()));
    }

    pub fn save_note(&mut self) {
        let title = self.editor.title_text().trim().to_string();
        if title.is_empty() {
            return;
        }
        let content = self.editor.content_text();
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
        self.settings.input = storage::display_notes_path().chars().collect();
        self.settings.cursor = self.settings.input.len();
        self.settings.error = None;
        self.screen = Screen::Settings;
    }

    pub fn confirm_settings(&mut self) {
        let raw = self.settings.text();
        let trimmed = raw.trim();
        let new_path = if trimmed.is_empty() {
            None
        } else {
            let normalized = storage::normalize_notes_path(trimmed);
            if !normalized.is_absolute() {
                self.settings.error = Some(
                    "Path must be absolute (e.g. /home/you/notes.json)".to_string(),
                );
                return;
            }
            Some(normalized)
        };
        match storage::set_notes_path(new_path) {
            Ok(effective) => {
                self.settings.error = None;
                self.error_message = None;
                self.status_message = Some(format!(
                    "Notes file: {}",
                    storage::display_path(&effective)
                ));
                self.refresh();
                self.screen = Screen::List;
            }
            Err(e) => {
                self.settings.error = Some(format!("Could not save settings: {e}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("notes_rs_app_{}_{}", std::process::id(), name))
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

    fn clear_all_notes() {
        for note in storage::load_notes() {
            storage::delete_note(&note.id).unwrap();
        }
    }

    #[test]
    fn test_editor_insert_and_backspace() {
        let mut ed = Editor::new();
        ed.insert_char('h');
        ed.insert_char('i');
        assert_eq!(ed.content_text(), "hi");
        ed.content_backspace();
        assert_eq!(ed.content_text(), "h");
        ed.move_cursor_home();
        ed.insert_char('x');
        assert_eq!(ed.content_text(), "xh");
    }

    #[test]
    fn test_editor_newline_split() {
        let mut ed = Editor::new();
        for c in "abcd".chars() {
            ed.insert_char(c);
        }
        ed.move_cursor_home();
        ed.move_cursor_right();
        ed.move_cursor_right();
        ed.insert_newline();
        assert_eq!(ed.lines.len(), 2);
        assert_eq!(ed.content_text(), "ab\ncd");
        ed.move_cursor_up();
        ed.move_cursor_end();
        ed.content_delete();
        assert_eq!(ed.content_text(), "abcd");
    }

    #[test]
    fn test_editor_cursor_navigation() {
        let mut ed = Editor::new();
        for c in "abc".chars() {
            ed.insert_char(c);
        }
        ed.insert_newline();
        for c in "de".chars() {
            ed.insert_char(c);
        }
        ed.move_cursor_up();
        assert_eq!(ed.cursor_line, 0);
        assert_eq!(ed.cursor_col, 2);
        ed.move_cursor_end();
        assert_eq!(ed.cursor_col, 3);
        ed.move_cursor_down();
        assert_eq!(ed.cursor_line, 1);
        assert_eq!(ed.cursor_col, 2);
        ed.move_cursor_home();
        ed.move_cursor_left();
        assert_eq!(ed.cursor_line, 0);
        assert_eq!(ed.cursor_col, 3);
        ed.move_cursor_end();
        ed.move_cursor_right();
        assert_eq!(ed.cursor_line, 1);
        assert_eq!(ed.cursor_col, 0);
    }

    #[test]
    fn test_editor_load_and_title() {
        let mut ed = Editor::new();
        ed.load("My Title", "line one\nline two");
        assert_eq!(ed.title_text(), "My Title");
        assert_eq!(ed.content_text(), "line one\nline two");
        assert_eq!(ed.lines.len(), 2);
        assert!(ed.focus_title);
        assert_eq!(ed.cursor_line, 0);
        assert_eq!(ed.cursor_col, 0);

        ed.title_insert_char('!');
        assert_eq!(ed.title_text(), "!My Title");
        ed.title_backspace();
        assert_eq!(ed.title_text(), "My Title");
        ed.title_move_right();
        ed.title_delete();
        assert_eq!(ed.title_text(), "M Title");
        ed.title_move_left();
        ed.title_backspace();
        assert_eq!(ed.title_text(), "M Title");
    }

    #[test]
    fn test_settings_state_editing() {
        let mut s = SettingsState::new();
        s.insert_char('a');
        s.insert_char('b');
        s.insert_char('c');
        assert_eq!(s.text(), "abc");
        s.move_left();
        s.backspace();
        assert_eq!(s.text(), "ac");
        s.move_left();
        s.delete();
        assert_eq!(s.text(), "c");
        s.home();
        s.insert_char('x');
        assert_eq!(s.text(), "xc");
        s.end();
        assert_eq!(s.cursor, 2);
    }

    #[test]
    fn test_app_new_note_and_save() {
        let _guard = crate::storage::tests::lock();
        setup();
        clear_all_notes();

        let mut app = App::new();
        app.start_new_note();
        assert!(matches!(app.screen, Screen::Editor(None)));
        app.editor.title_insert_char('T');
        app.editor.focus_title = false;
        app.editor.insert_char('h');
        app.editor.insert_char('i');
        app.save_note();
        assert!(matches!(app.screen, Screen::List));
        assert_eq!(app.notes.len(), 1);
        assert_eq!(app.notes[0].title, "T");
        assert_eq!(app.notes[0].content, "hi");

        teardown();
    }

    #[test]
    fn test_app_edit_existing_note() {
        let _guard = crate::storage::tests::lock();
        setup();
        clear_all_notes();

        let note = storage::add_note("Old", "old content").unwrap();
        let mut app = App::new();
        app.start_edit(&note);
        assert!(matches!(&app.screen, Screen::Editor(Some(n)) if n.id == note.id));
        assert_eq!(app.editor.title_text(), "Old");
        assert_eq!(app.editor.content_text(), "old content");

        app.editor.title_insert_char('!');
        app.save_note();
        assert_eq!(app.notes.len(), 1);
        assert_eq!(app.notes[0].title, "!Old");

        teardown();
    }

    #[test]
    fn test_app_search_and_navigation() {
        let _guard = crate::storage::tests::lock();
        setup();
        clear_all_notes();

        storage::add_note("Grocery", "milk").unwrap();
        storage::add_note("Work", "meeting").unwrap();

        let mut app = App::new();
        assert_eq!(app.filtered_notes.len(), 2);
        app.search_query = "groc".to_string();
        app.apply_search();
        assert_eq!(app.filtered_notes.len(), 1);
        assert_eq!(app.filtered_notes[0].title, "Grocery");
        app.search_query.clear();
        app.apply_search();
        assert_eq!(app.filtered_notes.len(), 2);

        app.selected_index = 0;
        assert_eq!(app.selected_note().unwrap().title, "Work");
        app.selected_index = 1;
        assert_eq!(app.selected_note().unwrap().title, "Grocery");
        app.selected_index = app.filtered_notes.len();
        app.apply_search();
        assert!(app.selected_index < app.filtered_notes.len());

        teardown();
    }

    #[test]
    fn test_app_delete_flow() {
        let _guard = crate::storage::tests::lock();
        setup();
        clear_all_notes();

        storage::add_note("Temp", "x").unwrap();
        let mut app = App::new();
        app.start_delete();
        assert!(matches!(app.screen, Screen::ConfirmDelete(_)));
        app.confirm_delete();
        assert!(matches!(app.screen, Screen::List));
        assert_eq!(app.notes.len(), 0);

        teardown();
    }

    #[test]
    fn test_app_save_empty_title_ignored() {
        let _guard = crate::storage::tests::lock();
        setup();
        clear_all_notes();

        let mut app = App::new();
        app.start_new_note();
        app.editor.focus_title = false;
        app.editor.insert_char('c');
        app.save_note();
        assert!(matches!(app.screen, Screen::Editor(_)));
        assert_eq!(app.notes.len(), 0);

        teardown();
    }

    #[test]
    fn test_editor_headings() {
        let mut ed = Editor::new();
        ed.focus_title = false;
        for c in "hello".chars() {
            ed.insert_char(c);
        }
        ed.set_heading(2);
        assert_eq!(ed.content_text(), "## hello");
        assert_eq!(ed.cursor_col, 8);
        ed.set_heading(1);
        assert_eq!(ed.content_text(), "# hello");
        ed.set_heading(0);
        assert_eq!(ed.content_text(), "hello");
        ed.load("", "");
        ed.focus_title = false;
        ed.set_heading(3);
        assert_eq!(ed.content_text(), "### ");
    }

    #[test]
    fn test_editor_bullet_toggle() {
        let mut ed = Editor::new();
        ed.focus_title = false;
        for c in "item".chars() {
            ed.insert_char(c);
        }
        ed.toggle_bullet();
        assert_eq!(ed.content_text(), "- item");
        ed.toggle_bullet();
        assert_eq!(ed.content_text(), "item");
        ed.load("", "1. first");
        ed.focus_title = false;
        ed.cursor_col = 2;
        ed.toggle_bullet();
        assert_eq!(ed.content_text(), "- first");
    }

    #[test]
    fn test_editor_ordered_toggle() {
        let mut ed = Editor::new();
        ed.focus_title = false;
        for c in "item".chars() {
            ed.insert_char(c);
        }
        ed.toggle_ordered();
        assert_eq!(ed.content_text(), "1. item");
        ed.toggle_ordered();
        assert_eq!(ed.content_text(), "item");
        ed.load("", "- first");
        ed.focus_title = false;
        ed.cursor_col = 1;
        ed.toggle_ordered();
        assert_eq!(ed.content_text(), "1. first");
    }

    #[test]
    fn test_editor_checkbox_toggle() {
        let mut ed = Editor::new();
        ed.load("", "- [ ] todo");
        ed.focus_title = false;
        ed.toggle_bullet();
        assert_eq!(ed.content_text(), "- [x] todo");
        ed.toggle_bullet();
        assert_eq!(ed.content_text(), "- [ ] todo");
    }

    #[test]
    fn test_editor_smart_newline_bullet() {
        let mut ed = Editor::new();
        ed.load("", "- hello");
        ed.focus_title = false;
        ed.move_cursor_end();
        ed.insert_newline_smart();
        assert_eq!(ed.content_text(), "- hello\n- ");
        ed.insert_newline_smart();
        assert_eq!(ed.content_text(), "- hello\n");
    }

    #[test]
    fn test_editor_smart_newline_ordered() {
        let mut ed = Editor::new();
        ed.load("", "1. alpha\n3. gamma");
        ed.focus_title = false;
        ed.move_cursor_end();
        ed.insert_newline_smart();
        assert_eq!(ed.content_text(), "1. alpha\n2. \n3. gamma");
        ed.insert_newline_smart();
        assert_eq!(ed.content_text(), "1. alpha\n\n1. gamma");
    }

    #[test]
    fn test_editor_move_line() {
        let mut ed = Editor::new();
        ed.load("", "one\ntwo\nthree");
        ed.focus_title = false;
        ed.cursor_line = 1;
        ed.move_line_up();
        assert_eq!(ed.content_text(), "two\none\nthree");
        assert_eq!(ed.cursor_line, 0);
        ed.move_line_down();
        ed.move_line_down();
        assert_eq!(ed.content_text(), "one\nthree\ntwo");
        assert_eq!(ed.cursor_line, 2);
    }

    #[test]
    fn test_editor_tab_indent() {
        let mut ed = Editor::new();
        ed.load("", "- x");
        ed.focus_title = false;
        assert!(ed.tab());
        assert_eq!(ed.content_text(), "  - x");
        assert!(ed.shift_tab());
        assert_eq!(ed.content_text(), "- x");
        let mut ed2 = Editor::new();
        ed2.load("", "plain");
        ed2.focus_title = false;
        assert!(!ed2.tab());
    }

    #[test]
    fn test_editor_table_insert_and_nav() {
        let mut ed = Editor::new();
        ed.load("", "hello");
        ed.focus_title = false;
        ed.insert_table();
        assert_eq!(ed.content_text(), "| a | b |\n|---|---|\n|   |   |\nhello");
        assert_eq!(ed.cursor_line, 2);
        assert_eq!(ed.cursor_col, 2);
        assert!(ed.tab());
        assert_eq!(ed.cursor_col, 4);
        assert!(ed.tab());
        assert_eq!(ed.cursor_col, 8);
        assert!(!ed.tab());
    }

    #[test]
    fn test_app_sort_orders() {
        let _guard = crate::storage::tests::lock();
        setup();
        clear_all_notes();

        let _ = storage::add_note("Zebra", "b");
        let _ = storage::add_note("apple", "a");

        let mut app = App::new();
        app.sort_order = SortOrder::TitleAsc;
        app.refresh();
        assert_eq!(app.filtered_notes[0].title, "apple");
        app.sort_order = SortOrder::TitleDesc;
        app.refresh();
        assert_eq!(app.filtered_notes[0].title, "Zebra");
        app.cycle_sort();
        assert_eq!(app.sort_order, SortOrder::UpdatedDesc);
        assert!(app.status_message.is_some());

        teardown();
    }
}

