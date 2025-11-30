use serde::{Deserialize, Serialize};

pub fn mock() -> CodeResults {
    let s = std::fs::read_to_string("resp.json").unwrap();

    serde_json::from_str(&s).unwrap()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeResults {
    pub items: Vec<ItemResult>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_resp_json() {
        let resp_json = std::fs::read_to_string("resp.json").unwrap();

        let resp: CodeResults = serde_json::from_str(&resp_json).unwrap();

        for item in &resp.items {
            println!();
            println!("{}", item.name);
            for text_match in &item.text_matches {
                println!("```");
                for line in text_match.fragment.lines() {
                    println!("{line}");
                }
                println!("```");
                println!();
            }
        }
    }
}
