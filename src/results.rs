use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeResults {
    pub items: Vec<ItemResult>,
}

impl CodeResults {
    pub fn count(&self) -> usize {
        self.items.iter().map(|ir| ir.text_matches.len()).sum()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemResult {
    pub name: String,
    pub path: String,
    pub html_url: String,
    pub text_matches: Vec<TextMatch>,
    pub repository: ItemRepository,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemRepository {
    pub name: String,
    pub full_name: String,
    pub owner: RepositoryOwner,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryOwner {
    pub login: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextMatch {
    pub fragment: String,
    pub matches: Vec<MatchSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchSegment {
    pub indices: (usize, usize),
    pub text: String,
}
