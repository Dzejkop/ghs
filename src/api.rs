use color_eyre::eyre;
use reqwest::{Method, Request, Url};

use crate::results::CodeResults;

const GITHUB_BASE_URI: &str = "https://api.github.com";

fn read_env_var(var_name: &str) -> String {
    let err = format!("Missing environment variable: {var_name}");
    std::env::var(var_name).expect(&err)
}

#[derive(Debug, Clone)]
pub struct PaginationInfo {
    pub prev: Option<String>,
    pub next: Option<String>,
    pub first: Option<String>,
    pub last: Option<String>,
}

impl PaginationInfo {
    fn from_link_header(link_header: &str) -> Self {
        let mut prev = None;
        let mut next = None;
        let mut first = None;
        let mut last = None;

        for part in link_header.split(',') {
            let part = part.trim();
            if let Some((url_part, rel_part)) = part.split_once(';') {
                let url = url_part
                    .trim()
                    .trim_start_matches('<')
                    .trim_end_matches('>')
                    .to_string();
                let rel = rel_part.trim();

                if rel.contains("prev") {
                    prev = Some(url);
                } else if rel.contains("next") {
                    next = Some(url);
                } else if rel.contains("first") {
                    first = Some(url);
                } else if rel.contains("last") {
                    last = Some(url);
                }
            }
        }

        Self {
            prev,
            next,
            first,
            last,
        }
    }

    pub fn get_last_page_number(&self) -> Option<u32> {
        self.last.as_ref().and_then(|url| {
            // Parse URL: "...?q=query&page=34"
            url.split("page=")
                .nth(1)
                .and_then(|s| s.split('&').next())
                .and_then(|s| s.parse::<u32>().ok())
        })
    }
}

#[derive(Debug, Clone)]
pub struct CodeResultsWithPagination {
    pub results: CodeResults,
    pub pagination: Option<PaginationInfo>,
}

pub async fn fetch_code_results(
    query: &str,
    page: Option<u32>,
) -> eyre::Result<CodeResultsWithPagination> {
    let url = format!("{GITHUB_BASE_URI}/search/code");
    let mut url = Url::parse(&url)?;

    let mut query_string = format!("q={}", urlencoding::encode(query));
    if let Some(page) = page {
        query_string.push_str(&format!("&page={}", page));
    }
    url.set_query(Some(&query_string));

    let mut req = Request::new(Method::GET, url);
    req.headers_mut().insert(
        "Authorization",
        format!("Bearer {}", read_env_var("GITHUB_TOKEN"))
            .parse()
            .unwrap(),
    );
    req.headers_mut().insert(
        "Accept",
        "application/vnd.github.text-match+json".parse().unwrap(),
    );
    req.headers_mut()
        .insert("User-Agent", "ghs".parse().unwrap());

    let client = reqwest::Client::new();

    let response = client.execute(req).await?;

    let pagination = response
        .headers()
        .get("link")
        .and_then(|v| v.to_str().ok())
        .map(PaginationInfo::from_link_header);

    let body = response.text().await?;
    let results: CodeResults = serde_json::from_str(&body)?;

    Ok(CodeResultsWithPagination {
        results,
        pagination,
    })
}
