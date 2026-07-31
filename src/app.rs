use crate::storage::{self, Note};

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    List,
    Editor(Option<Note>),
    ConfirmDelete(String),
}

pub struct App {
    pub notes: Vec<Note>,
    pub filtered_notes: Vec<Note>,
    pub selected_index: usize,
    pub screen: Screen,
    pub search_query: String,
    pub editor_title: String,
    pub editor_content: String,
    pub editor_focus_title: bool,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        let notes = storage::load_notes();
        let filtered_notes = notes.clone();
        Self {
            notes,
            filtered_notes,
            selected_index: 0,
            screen: Screen::List,
            search_query: String::new(),
            editor_title: String::new(),
            editor_content: String::new(),
            editor_focus_title: true,
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
        self.editor_title.clear();
        self.editor_content.clear();
        self.editor_focus_title = true;
        self.screen = Screen::Editor(None);
    }

    pub fn start_edit(&mut self, note: &Note) {
        self.editor_title = note.title.clone();
        self.editor_content = note.content.clone();
        self.editor_focus_title = true;
        self.screen = Screen::Editor(Some(note.clone()));
    }

    pub fn save_note(&mut self) {
        let title = self.editor_title.trim().to_string();
        let content = self.editor_content.clone();
        if title.is_empty() {
            return;
        }
        match &self.screen {
            Screen::Editor(Some(note)) => {
                storage::update_note(&note.id, &title, &content);
            }
            _ => {
                storage::add_note(&title, &content);
            }
        }
        self.refresh();
        self.screen = Screen::List;
    }

    pub fn start_delete(&mut self) {
        if let Some(note) = self.selected_note().cloned() {
            self.screen = Screen::ConfirmDelete(note.title.clone());
        }
    }

    pub fn confirm_delete(&mut self) {
        if let Screen::ConfirmDelete(title) = &self.screen {
            if let Some(note) = self.notes.iter().find(|n| n.title == *title) {
                storage::delete_note(&note.id);
            }
        }
        self.refresh();
        self.screen = Screen::List;
    }
}
