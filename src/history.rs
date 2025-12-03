use color_eyre::eyre;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

const MAX_HISTORY_SIZE: usize = 100;

#[derive(Debug, Clone, Default)]
pub struct SearchHistory {
    pub searches: Vec<String>,
    pub selected_idx: Option<usize>,
}

impl SearchHistory {
    pub fn new(searches: Vec<String>) -> Self {
        Self {
            searches,
            selected_idx: None,
        }
    }

    pub fn add_search(&mut self, query: String) {
        // Remove existing occurrence if present
        self.searches.retain(|s| s != &query);

        // Add to front
        self.searches.insert(0, query);

        // Limit size
        if self.searches.len() > MAX_HISTORY_SIZE {
            self.searches.truncate(MAX_HISTORY_SIZE);
        }
    }

    pub fn select_next(&mut self) {
        if self.searches.is_empty() {
            return;
        }

        self.selected_idx = Some(match self.selected_idx {
            None => 0,
            Some(idx) => (idx + 1).min(self.searches.len() - 1),
        });
    }

    pub fn select_prev(&mut self) {
        if self.searches.is_empty() {
            return;
        }

        self.selected_idx = Some(match self.selected_idx {
            None => 0,
            Some(idx) => idx.saturating_sub(1),
        });
    }

    pub fn get_selected(&self) -> Option<&String> {
        self.selected_idx
            .and_then(|idx| self.searches.get(idx))
    }

    pub fn clear_selection(&mut self) {
        self.selected_idx = None;
    }
}

fn get_history_path() -> eyre::Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| eyre::eyre!("Could not find config directory"))?;

    let ghs_dir = config_dir.join("ghs");
    Ok(ghs_dir.join("history.json"))
}

pub async fn load_history() -> eyre::Result<SearchHistory> {
    let path = get_history_path()?;

    if !path.exists() {
        return Ok(SearchHistory::default());
    }

    let contents = fs::read_to_string(&path).await?;
    let searches: Vec<String> = serde_json::from_str(&contents)?;

    Ok(SearchHistory::new(searches))
}

pub async fn save_history(history: &SearchHistory) -> eyre::Result<()> {
    let path = get_history_path()?;

    // Create parent directory if it doesn't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let contents = serde_json::to_string_pretty(&history.searches)?;
    fs::write(&path, contents).await?;

    Ok(())
}
