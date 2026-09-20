use crate::model::AppState;
use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::fs;
use std::path::PathBuf;

pub fn state_path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("", "", "deepin-sticky-notes")
        .context("cannot determine XDG data directory")?;
    Ok(dirs.data_dir().join("notes.json"))
}

pub fn load() -> Result<AppState> {
    let path = state_path()?;
    if !path.exists() {
        return Ok(AppState::default());
    }
    let data = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&data).with_context(|| format!("parse {}", path.display()))
}

pub fn save(state: &AppState) -> Result<()> {
    let path = state_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(state)?;
    let temp_path = path.with_extension("json.tmp");
    fs::write(&temp_path, data)?;
    fs::rename(temp_path, path)?;
    Ok(())
}
