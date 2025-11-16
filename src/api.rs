use color_eyre::eyre;
use reqwest::{Method, Request, Url};

const GITHUB_BASE_URI: &str = "https://api.github.com";

fn read_env_var(var_name: &str) -> String {
    let err = format!("Missing environment variable: {var_name}");
    std::env::var(var_name).expect(&err)
}

pub async fn fetch_code_results() -> eyre::Result<()> {
    let url = format!("{GITHUB_BASE_URI}/search/code");

    let mut url = Url::parse(&url)?;
    let query = format!(
        "q={}",
        urlencoding::encode("org:worldcoin identity_commitment")
    );
    url.set_query(Some(query.as_ref()));

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
    let body = response.text().await?;

    let response_json_body = serde_json::from_str::<serde_json::Value>(&body)?;
    let response_pretty = serde_json::to_string_pretty(&response_json_body)?;

    std::fs::write("resp.json", response_pretty).unwrap();

    Ok(())
}
