use std::{collections::HashMap, path::PathBuf};

use rig::{completion::Prompt, prelude::CompletionClient, providers::openrouter};
use serde::{Deserialize, Serialize};

use crate::ai::AnalyzedFileContext;

#[derive(Debug, Serialize)]
struct PromptPayload {
    files: Vec<PromptFile>,
}

#[derive(Debug, Serialize)]
struct PromptFile {
    path: String,
    current_name: String,
    extension: Option<String>,
    size: u64,
    modified_unix: Option<i64>,
    mime_type: Option<String>,
    detected_languages: Vec<String>,
    excerpt: String,
}

#[derive(Debug, Deserialize)]
struct SuggestionResponse {
    files: Vec<SuggestionRow>,
}

#[derive(Debug, Deserialize)]
struct SuggestionRow {
    path: String,
    suggestions: Vec<String>,
}

pub async fn generate_filename_suggestions(
    api_key: &str,
    model_id: &str,
    contexts: &[AnalyzedFileContext],
    user_prompt: Option<&str>,
) -> Result<HashMap<PathBuf, [String; 3]>, String> {
    let client: openrouter::Client = openrouter::Client::new(api_key)
        .map_err(|e| format!("OpenRouter client initialization failed: {e}"))?;
    let agent = client
        .agent(model_id)
        .preamble(
            "You are a file-renaming assistant. Return ONLY strict JSON (no markdown fences, no prose). \
             For each file, generate exactly three distinct filename suggestions focused on clarity and utility.",
        )
        .temperature(0.2)
        .max_tokens(2_048)
        .build();

    let prompt_files: Vec<PromptFile> = contexts
        .iter()
        .map(|c| PromptFile {
            path: c.path.to_string_lossy().to_string(),
            current_name: c.current_name.clone(),
            extension: c.extension.clone(),
            size: c.size,
            modified_unix: c.modified_unix,
            mime_type: c.mime_type.clone(),
            detected_languages: c.detected_languages.clone(),
            excerpt: c.excerpt.clone(),
        })
        .collect();
    let payload = PromptPayload {
        files: prompt_files,
    };
    let payload_json = serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("Failed to build prompt payload: {e}"))?;

    let user_guidance = user_prompt
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("");
    let prompt = if user_guidance.is_empty() {
        format!(
            "Return JSON with shape:\n\
             {{\"files\":[{{\"path\":\"<exact path>\",\"suggestions\":[\"name1\",\"name2\",\"name3\"]}}]}}\n\
             Requirements:\n\
             - Include every input file path exactly once.\n\
             - Keep filenames concise, human-readable, and deterministic from context.\n\
             - No directory separators or absolute paths inside suggestions.\n\
             - Prefer preserving semantic meaning from content and metadata.\n\
             Input:\n{}",
            payload_json
        )
    } else {
        format!(
            "Return JSON with shape:\n\
             {{\"files\":[{{\"path\":\"<exact path>\",\"suggestions\":[\"name1\",\"name2\",\"name3\"]}}]}}\n\
             Requirements:\n\
             - Include every input file path exactly once.\n\
             - Keep filenames concise, human-readable, and deterministic from context.\n\
             - No directory separators or absolute paths inside suggestions.\n\
             - Prefer preserving semantic meaning from content and metadata.\n\
             - Apply this user request while preserving safety and filename validity: {}\n\
             Input:\n{}",
            user_guidance, payload_json
        )
    };

    let raw = agent
        .prompt(prompt)
        .await
        .map_err(|e| format!("OpenRouter generation failed: {e}"))?;

    let json = extract_json_block(&raw)
        .ok_or_else(|| "Model response did not contain a JSON object".to_string())?;
    let parsed: SuggestionResponse = serde_json::from_str(json)
        .map_err(|e| format!("Could not parse model JSON response: {e}"))?;

    let mut by_input_path: HashMap<PathBuf, &AnalyzedFileContext> = HashMap::new();
    for ctx in contexts {
        by_input_path.insert(ctx.path.clone(), ctx);
    }

    let mut out = HashMap::new();
    for row in parsed.files {
        let path = PathBuf::from(&row.path);
        let Some(ctx) = by_input_path.get(&path) else {
            continue;
        };
        let suggestions = normalize_suggestions(&row.suggestions, ctx);
        out.insert(path, suggestions);
    }

    for ctx in contexts {
        out.entry(ctx.path.clone())
            .or_insert_with(|| fallback_suggestions(ctx));
    }

    Ok(out)
}

fn normalize_suggestions(raw: &[String], ctx: &AnalyzedFileContext) -> [String; 3] {
    let mut normalized = Vec::new();
    for candidate in raw {
        let cleaned = sanitize_filename(candidate, ctx);
        if !cleaned.is_empty() && !normalized.iter().any(|v| v == &cleaned) {
            normalized.push(cleaned);
        }
        if normalized.len() == 3 {
            break;
        }
    }

    if normalized.len() < 3 {
        let fallback = fallback_suggestions(ctx);
        for candidate in fallback {
            if !normalized.iter().any(|v| v == &candidate) {
                normalized.push(candidate);
            }
            if normalized.len() == 3 {
                break;
            }
        }
    }

    [
        normalized
            .first()
            .cloned()
            .unwrap_or_else(|| "file".to_string()),
        normalized
            .get(1)
            .cloned()
            .unwrap_or_else(|| "file-2".to_string()),
        normalized
            .get(2)
            .cloned()
            .unwrap_or_else(|| "file-3".to_string()),
    ]
}

fn fallback_suggestions(ctx: &AnalyzedFileContext) -> [String; 3] {
    let stem = ctx
        .current_name
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(ctx.current_name.as_str())
        .trim();
    let base = if stem.is_empty() { "file" } else { stem };
    [
        sanitize_filename(base, ctx),
        sanitize_filename(&format!("{base} copy"), ctx),
        sanitize_filename(&format!("{base} final"), ctx),
    ]
}

fn sanitize_filename(candidate: &str, ctx: &AnalyzedFileContext) -> String {
    let mut value = candidate.trim().to_string();
    if let Some(ext) = &ctx.extension {
        let suffix = format!(".{ext}");
        if value.to_lowercase().ends_with(&suffix) {
            value = value[..value.len().saturating_sub(suffix.len())].to_string();
        }
    }

    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        let illegal = matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|');
        if illegal || ch.is_control() {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    let out = out.trim().trim_matches('.').trim().to_string();
    if out.is_empty() {
        "file".to_string()
    } else {
        out
    }
}

fn extract_json_block(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end < start {
        return None;
    }
    raw.get(start..=end)
}
