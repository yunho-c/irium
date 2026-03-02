use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::Deserialize;

use crate::ai::ModelListItem;

const OPENROUTER_BASE: &str = "https://openrouter.ai/api/v1";

#[derive(Debug, Deserialize)]
struct OpenRouterModelList {
    data: Vec<OpenRouterModel>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterModel {
    id: String,
    name: Option<String>,
    context_length: Option<u64>,
    pricing: Option<OpenRouterPricing>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterPricing {
    prompt: Option<serde_json::Value>,
    completion: Option<serde_json::Value>,
}

pub async fn discover_models(api_key: Option<&str>) -> Result<Vec<ModelListItem>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| format!("Failed to initialize HTTP client: {e}"))?;

    if let Some(key) = api_key.filter(|k| !k.trim().is_empty()) {
        let user_url = format!("{OPENROUTER_BASE}/models/user");
        if let Ok(models) = fetch_models(&client, &user_url, Some(key)).await
            && !models.is_empty()
        {
            return Ok(models);
        }
    }

    let public_url = format!("{OPENROUTER_BASE}/models");
    fetch_models(
        &client,
        &public_url,
        api_key.filter(|k| !k.trim().is_empty()),
    )
    .await
}

async fn fetch_models(
    client: &reqwest::Client,
    url: &str,
    api_key: Option<&str>,
) -> Result<Vec<ModelListItem>, String> {
    let mut headers = HeaderMap::new();
    if let Some(key) = api_key {
        let token = format!("Bearer {key}");
        let value = HeaderValue::from_str(&token)
            .map_err(|e| format!("Invalid API key for header: {e}"))?;
        headers.insert(AUTHORIZATION, value);
    }

    let response = client
        .get(url)
        .headers(headers)
        .send()
        .await
        .map_err(|e| format!("OpenRouter request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<no body>".to_string());
        return Err(format!("OpenRouter responded with {status}: {body}"));
    }

    let parsed: OpenRouterModelList = response
        .json()
        .await
        .map_err(|e| format!("Could not parse OpenRouter model response: {e}"))?;

    let mut items: Vec<ModelListItem> = parsed
        .data
        .into_iter()
        .map(|model| ModelListItem {
            id: model.id.clone(),
            name: model.name.unwrap_or_else(|| model.id.clone()),
            context_length: model.context_length,
            pricing_prompt: model
                .pricing
                .as_ref()
                .and_then(|p| p.prompt.as_ref())
                .map(value_to_string),
            pricing_completion: model
                .pricing
                .as_ref()
                .and_then(|p| p.completion.as_ref())
                .map(value_to_string),
        })
        .collect();

    items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(items)
}

fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => value.to_string(),
    }
}
