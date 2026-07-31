use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub content: String,
    pub updated_at: String,
}

fn notes_path() -> PathBuf {
    let mut path = std::env::current_dir().unwrap_or_default();
    path.push("notes.json");
    path
}

pub fn load_notes() -> Vec<Note> {
    let path = notes_path();
    if !path.exists() {
        return vec![];
    }
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    #[derive(Deserialize)]
    struct Wrapper {
        notes: Vec<Note>,
    }
    match serde_json::from_str::<Wrapper>(&content) {
        Ok(w) => w.notes,
        Err(_) => vec![],
    }
}

fn save_notes(notes: &[Note]) {
    #[derive(Serialize)]
    struct Wrapper<'a> {
        notes: &'a [Note],
    }
    let path = notes_path();
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let wrapper = Wrapper { notes };
    if let Ok(json) = serde_json::to_string_pretty(&wrapper) {
        let _ = fs::write(&path, json);
    }
}

pub fn add_note(title: &str, content: &str) -> Note {
    let mut notes = load_notes();
    let note = Note {
        id: Uuid::new_v4().to_string()[..8].to_string(),
        title: title.to_string(),
        content: content.to_string(),
        updated_at: Utc::now().to_rfc3339(),
    };
    notes.insert(0, note.clone());
    save_notes(&notes);
    note
}

pub fn update_note(id: &str, title: &str, content: &str) -> Option<Note> {
    let mut notes = load_notes();
    let found = notes.iter_mut().find(|n| n.id == id)?;
    found.title = title.to_string();
    found.content = content.to_string();
    found.updated_at = Utc::now().to_rfc3339();
    let note = found.clone();
    save_notes(&notes);
    Some(note)
}

pub fn delete_note(id: &str) {
    let notes = load_notes();
    let notes: Vec<Note> = notes.into_iter().filter(|n| n.id != id).collect();
    save_notes(&notes);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clear_all_notes() {
        for note in load_notes() {
            delete_note(&note.id);
        }
    }

    #[test]
    fn test_crud() {
        clear_all_notes();

        let note = add_note("Test Title", "Test Content");
        assert_eq!(note.title, "Test Title");
        assert_eq!(note.content, "Test Content");

        let notes = load_notes();
        assert_eq!(notes.len(), 1);

        update_note(&note.id, "Updated", "Updated Content");
        let notes = load_notes();
        assert_eq!(notes[0].title, "Updated");

        delete_note(&note.id);
        assert_eq!(load_notes().len(), 0);
    }

    #[test]
    fn test_search() {
        clear_all_notes();

        add_note("Grocery List", "milk, eggs");
        add_note("Meeting Notes", "discuss project");
        add_note("Python Ideas", "build TUI");

        assert_eq!(search_notes("grocery").len(), 1);
        assert_eq!(search_notes("Grocery").len(), 1);
        assert_eq!(search_notes("project").len(), 1);
        assert_eq!(search_notes("").len(), 3);
        assert_eq!(search_notes("zzz").len(), 0);

        clear_all_notes();
        assert_eq!(load_notes().len(), 0);
    }
}

pub fn search_notes(query: &str) -> Vec<Note> {
    if query.is_empty() {
        return load_notes();
    }
    let q = query.to_lowercase();
    let notes = load_notes();
    notes
        .into_iter()
        .filter(|n| n.title.to_lowercase().contains(&q) || n.content.to_lowercase().contains(&q))
        .collect()
}
