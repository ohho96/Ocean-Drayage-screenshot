use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{Check, Evidence, Receiving, Shift};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Saved {
    #[serde(default)]
    pub container: String,
    #[serde(default)]
    pub shift: Shift,
    #[serde(default)]
    pub checks: HashMap<String, Check>,
    #[serde(default)]
    pub log: Vec<Evidence>,
    #[serde(default)]
    pub next_id: u64,
    /// Containers Playwright should check, one per line.
    #[serde(default)]
    pub containers: String,
    #[serde(default)]
    pub receivings: Vec<Receiving>,
}

impl Default for Saved {
    fn default() -> Self {
        Self {
            container: String::new(),
            shift: Shift::First,
            checks: HashMap::new(),
            log: Vec::new(),
            next_id: 1,
            containers: String::new(),
            receivings: Vec::new(),
        }
    }
}

pub fn state_path() -> PathBuf {
    let dir = if cfg!(target_os = "macos") {
        std::env::var("HOME")
            .map(|home| PathBuf::from(home).join("Library/Application Support/EmptyReturnDesk"))
            .unwrap_or_else(|_| PathBuf::from("data"))
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".local/share/empty-return-desk")
    } else {
        PathBuf::from("data")
    };
    dir.join("state.json")
}

pub fn load(path: &Path) -> Result<Saved, String> {
    if !path.exists() {
        return Ok(Saved::default());
    }
    let text = fs::read_to_string(path).map_err(|err| err.to_string())?;
    match serde_json::from_str(&text) {
        Ok(saved) => Ok(saved),
        Err(err) => {
            let backup = path.with_extension("json.bak");
            let _ = fs::copy(path, &backup);
            Err(format!(
                "Could not read {}. A copy is at {}. ({err})",
                path.display(),
                backup.display()
            ))
        }
    }
}

pub fn save(path: &Path, saved: &Saved) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let json = serde_json::to_string_pretty(saved).map_err(|err| err.to_string())?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, json).map_err(|err| err.to_string())?;
    fs::rename(&temporary, path).map_err(|err| err.to_string())?;
    Ok(())
}
