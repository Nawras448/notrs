use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub content: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub notes_path: Option<String>,
}

fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("NOTRS_CONFIG_FILE") {
        return PathBuf::from(p);
    }
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config"));
    base.join("notes_rs").join("config.json")
}

pub fn expand_tilde(path: &str) -> PathBuf {
    let trimmed = path.trim();
    let home = std::env::var("HOME").unwrap_or_default();
    if trimmed == "~" {
        return PathBuf::from(home);
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        return PathBuf::from(home).join(rest);
    }
    PathBuf::from(trimmed)
}

pub fn normalize_notes_path(input: &str) -> PathBuf {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return std::env::current_dir().unwrap_or_default().join("notes.json");
    }
    let expanded = expand_tilde(trimmed);
    let is_json_file = expanded.extension().map(|e| e == "json").unwrap_or(false);
    if expanded.is_file() || is_json_file {
        return expanded;
    }
    expanded.join("notes.json")
}

pub fn display_path(path: &Path) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    if home.is_empty() {
        return path.to_string_lossy().to_string();
    }
    match path.strip_prefix(&home) {
        Ok(rel) => {
            let rel = rel.to_string_lossy();
            if rel.is_empty() {
                "~".to_string()
            } else {
                format!("~/{}", rel.trim_start_matches('/'))
            }
        }
        Err(_) => path.to_string_lossy().to_string(),
    }
}

pub fn display_notes_path() -> String {
    display_path(&notes_path())
}

pub fn notes_path() -> PathBuf {
    if let Ok(p) = std::env::var("NOTRS_DATA_FILE") {
        return PathBuf::from(p);
    }
    match load_config().notes_path {
        Some(p) if !p.trim().is_empty() => expand_tilde(&p),
        _ => std::env::current_dir().unwrap_or_default().join("notes.json"),
    }
}

pub fn load_config() -> Config {
    let content = match fs::read_to_string(config_path()) {
        Ok(c) => c,
        Err(_) => return Config::default(),
    };
    serde_json::from_str(&content).unwrap_or_default()
}

pub fn save_config(config: &Config) -> io::Result<()> {
    let path = config_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(&path, json)
}

pub fn migrate_notes(from: &Path, to: &Path) -> io::Result<bool> {
    if to.exists() || !from.exists() {
        return Ok(false);
    }
    if let Some(dir) = to.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::copy(from, to)?;
    Ok(true)
}

pub fn migrate_legacy_notes(target: &Path) {
    let legacy = std::env::current_dir().unwrap_or_default().join("notes.json");
    let _ = migrate_notes(&legacy, target);
}

pub fn ensure_configured_path() {
    migrate_legacy_notes(&notes_path());
}

pub fn set_notes_path(new_path: Option<PathBuf>) -> io::Result<PathBuf> {
    let old = notes_path();
    let config = Config {
        notes_path: new_path.map(|p| p.to_string_lossy().to_string()),
    };
    save_config(&config)?;
    let new = notes_path();
    if new != old {
        migrate_notes(&old, &new)?;
    }
    if let Some(dir) = new.parent() {
        fs::create_dir_all(dir)?;
    }
    Ok(new)
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

fn save_notes(notes: &[Note]) -> io::Result<()> {
    #[derive(Serialize)]
    struct Wrapper<'a> {
        notes: &'a [Note],
    }
    let path = notes_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let wrapper = Wrapper { notes };
    let json = serde_json::to_string_pretty(&wrapper)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(&path, json)
}

pub fn add_note(title: &str, content: &str) -> io::Result<Note> {
    let mut notes = load_notes();
    let note = Note {
        id: Uuid::new_v4().to_string()[..8].to_string(),
        title: title.to_string(),
        content: content.to_string(),
        updated_at: Utc::now().to_rfc3339(),
    };
    notes.insert(0, note.clone());
    save_notes(&notes)?;
    Ok(note)
}

pub fn update_note(id: &str, title: &str, content: &str) -> io::Result<Note> {
    let mut notes = load_notes();
    let found = notes
        .iter_mut()
        .find(|n| n.id == id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "note not found"))?;
    found.title = title.to_string();
    found.content = content.to_string();
    found.updated_at = Utc::now().to_rfc3339();
    let note = found.clone();
    save_notes(&notes)?;
    Ok(note)
}

pub fn delete_note(id: &str) -> io::Result<()> {
    let notes = load_notes();
    let notes: Vec<Note> = notes.into_iter().filter(|n| n.id != id).collect();
    save_notes(&notes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn temp_data_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("notes_rs_{}_{}", std::process::id(), name))
    }

    fn setup() -> (PathBuf, PathBuf) {
        let data_file = temp_data_file("data.json");
        let config_file = temp_data_file("config.json");
        std::env::set_var("NOTRS_DATA_FILE", &data_file);
        std::env::set_var("NOTRS_CONFIG_FILE", &config_file);
        let _ = fs::remove_file(&data_file);
        let _ = fs::remove_file(&config_file);
        (data_file, config_file)
    }

    fn teardown(data_file: &Path, config_file: &Path) {
        let _ = fs::remove_file(data_file);
        let _ = fs::remove_file(config_file);
        std::env::remove_var("NOTRS_DATA_FILE");
        std::env::remove_var("NOTRS_CONFIG_FILE");
    }

    fn clear_all_notes() {
        for note in load_notes() {
            delete_note(&note.id).unwrap();
        }
    }

    #[test]
    fn test_crud() {
        let _guard = TEST_LOCK.lock().unwrap();
        let (data, config) = setup();
        clear_all_notes();

        let note = add_note("Test Title", "Test Content").unwrap();
        assert_eq!(note.title, "Test Title");
        assert_eq!(note.content, "Test Content");

        let notes = load_notes();
        assert_eq!(notes.len(), 1);

        update_note(&note.id, "Updated", "Updated Content").unwrap();
        let notes = load_notes();
        assert_eq!(notes[0].title, "Updated");

        delete_note(&note.id).unwrap();
        assert_eq!(load_notes().len(), 0);
        teardown(&data, &config);
    }

    #[test]
    fn test_search() {
        let _guard = TEST_LOCK.lock().unwrap();
        let (data, config) = setup();
        clear_all_notes();

        add_note("Grocery List", "milk, eggs").unwrap();
        add_note("Meeting Notes", "discuss project").unwrap();
        add_note("Python Ideas", "build TUI").unwrap();

        assert_eq!(search_notes("grocery").len(), 1);
        assert_eq!(search_notes("Grocery").len(), 1);
        assert_eq!(search_notes("project").len(), 1);
        assert_eq!(search_notes("").len(), 3);
        assert_eq!(search_notes("zzz").len(), 0);

        clear_all_notes();
        assert_eq!(load_notes().len(), 0);
        teardown(&data, &config);
    }

    #[test]
    fn test_config_roundtrip() {
        let _guard = TEST_LOCK.lock().unwrap();
        let (data, config) = setup();

        let custom = "/tmp/my_notes.json";
        save_config(&Config {
            notes_path: Some(custom.to_string()),
        })
        .unwrap();
        let loaded = load_config();
        assert_eq!(loaded.notes_path.as_deref(), Some(custom));

        save_config(&Config::default()).unwrap();
        assert_eq!(load_config().notes_path, None);
        teardown(&data, &config);
    }

    #[test]
    fn test_expand_tilde() {
        let home = std::env::var("HOME").unwrap_or_default();
        assert_eq!(expand_tilde("~").to_string_lossy(), home);
        assert_eq!(expand_tilde("~/x").to_string_lossy(), format!("{home}/x"));
        assert_eq!(expand_tilde("/abs/path").to_string_lossy(), "/abs/path");
    }

    #[test]
    fn test_normalize_notes_path() {
        let home = std::env::var("HOME").unwrap_or_default();
        assert_eq!(
            normalize_notes_path("~").to_string_lossy(),
            format!("{home}/notes.json")
        );
        assert_eq!(
            normalize_notes_path("~/x").to_string_lossy(),
            format!("{home}/x/notes.json")
        );
        assert_eq!(
            normalize_notes_path("/tmp/n.json").to_string_lossy(),
            "/tmp/n.json"
        );
        assert_eq!(
            normalize_notes_path("/tmp/n").to_string_lossy(),
            "/tmp/n/notes.json"
        );
    }

    #[test]
    fn test_display_path() {
        let home = std::env::var("HOME").unwrap_or_default();
        assert_eq!(
            display_path(&PathBuf::from(&home).join("x/notes.json")),
            "~/x/notes.json"
        );
        assert_eq!(display_path(Path::new(&home)), "~");
        assert_eq!(display_path(Path::new("/etc/passwd")), "/etc/passwd");
    }

    #[test]
    fn test_migrate_notes() {
        let _guard = TEST_LOCK.lock().unwrap();
        let (data, config) = setup();

        let from = temp_data_file("from.json");
        let to = temp_data_file("to.json");
        let _ = fs::remove_file(&from);
        let _ = fs::remove_file(&to);
        fs::write(&from, "{\"notes\":[]}").unwrap();

        migrate_notes(&from, &to).unwrap();
        assert!(to.exists());
        assert_eq!(fs::read_to_string(&to).unwrap(), "{\"notes\":[]}");

        migrate_notes(&from, &to).unwrap();
        assert!(to.exists());

        let _ = fs::remove_file(&from);
        let _ = fs::remove_file(&to);
        teardown(&data, &config);
    }

    #[test]
    fn test_relocate_migrates_data() {
        let _guard = TEST_LOCK.lock().unwrap();

        let config_file = temp_data_file("reloc_cfg.json");
        let from = temp_data_file("from_reloc.json");
        let to = temp_data_file("to_reloc.json");
        std::env::remove_var("NOTRS_DATA_FILE");
        std::env::set_var("NOTRS_CONFIG_FILE", &config_file);
        let _ = fs::remove_file(&config_file);
        let _ = fs::remove_file(&from);
        let _ = fs::remove_file(&to);

        set_notes_path(Some(from.clone())).unwrap();
        add_note("Alpha", "beta").unwrap();
        assert!(from.exists());

        let effective = set_notes_path(Some(to.clone())).unwrap();
        assert_eq!(effective, to);
        assert!(to.exists());
        assert_eq!(load_notes().len(), 1);
        assert_eq!(load_notes()[0].title, "Alpha");

        let _ = fs::remove_file(&from);
        let _ = fs::remove_file(&to);
        let _ = fs::remove_file(&config_file);
        std::env::remove_var("NOTRS_CONFIG_FILE");
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
