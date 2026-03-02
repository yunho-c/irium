use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, UNIX_EPOCH},
};

use kreuzberg::{ExtractionConfig, extract_file};
use tokio::{sync::Semaphore, task::JoinSet, time::timeout};

use crate::ai::{
    AnalyzeOutput, AnalyzedFileContext, MAX_EXCERPT_CHARS_PER_FILE, MAX_FILES_PER_RUN,
    MAX_TOTAL_CONTEXT_CHARS,
};

const EXTRACT_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_CONCURRENT_EXTRACTS: usize = 4;

#[derive(Debug)]
struct SingleAnalyzeResult {
    context: AnalyzedFileContext,
    warning: Option<String>,
}

pub async fn analyze_files(paths: Vec<PathBuf>) -> AnalyzeOutput {
    let original_len = paths.len();
    let limited: Vec<PathBuf> = paths.into_iter().take(MAX_FILES_PER_RUN).collect();
    let skipped_count = original_len.saturating_sub(limited.len());
    let mut out = AnalyzeOutput {
        skipped_count,
        warnings: Vec::new(),
        files: Vec::new(),
    };

    if limited.is_empty() {
        return out;
    }

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_EXTRACTS));
    let mut set = JoinSet::new();
    for path in limited {
        let semaphore = semaphore.clone();
        set.spawn(async move {
            let _permit = semaphore.acquire_owned().await.ok();
            analyze_one(path).await
        });
    }

    while let Some(joined) = set.join_next().await {
        match joined {
            Ok(single) => {
                if let Some(w) = single.warning {
                    out.warnings.push(w);
                }
                out.files.push(single.context);
            }
            Err(error) => {
                out.warnings
                    .push(format!("Analyze task failed to join: {error}"));
            }
        }
    }

    out.files.sort_by(|a, b| a.path.cmp(&b.path));
    enforce_total_excerpt_limit(&mut out.files, MAX_TOTAL_CONTEXT_CHARS);
    out
}

async fn analyze_one(path: PathBuf) -> SingleAnalyzeResult {
    let meta = std::fs::metadata(&path).ok();
    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let modified_unix = meta
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64);

    let current_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string();
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|s| s.to_lowercase());

    let config = ExtractionConfig::default();
    let mut warning = None;
    let mut excerpt = String::new();
    let mut mime_type = None;
    let mut detected_languages = Vec::new();

    match timeout(EXTRACT_TIMEOUT, extract_file(&path, None, &config)).await {
        Ok(Ok(result)) => {
            mime_type = Some(result.mime_type.to_string());
            detected_languages = result.detected_languages.unwrap_or_default();
            excerpt = truncate_chars(&result.content, MAX_EXCERPT_CHARS_PER_FILE);
        }
        Ok(Err(error)) => {
            warning = Some(format!(
                "Kreuzberg analysis failed for {}: {error}",
                path.display()
            ));
        }
        Err(_) => {
            warning = Some(format!(
                "Kreuzberg analysis timed out after {:?} for {}",
                EXTRACT_TIMEOUT,
                path.display()
            ));
        }
    }

    SingleAnalyzeResult {
        context: AnalyzedFileContext {
            path,
            current_name,
            extension,
            size,
            modified_unix,
            mime_type,
            detected_languages,
            excerpt,
            warnings: Vec::new(),
        },
        warning,
    }
}

fn enforce_total_excerpt_limit(files: &mut [AnalyzedFileContext], max_total: usize) {
    let mut used = 0usize;
    for file in files {
        if file.excerpt.is_empty() {
            continue;
        }
        let chars = file.excerpt.chars().count();
        if used >= max_total {
            file.excerpt.clear();
            file.warnings
                .push("Excerpt omitted due to context size cap".to_string());
            continue;
        }
        let remaining = max_total - used;
        if chars > remaining {
            file.excerpt = truncate_chars(&file.excerpt, remaining);
            file.warnings
                .push("Excerpt truncated due to context size cap".to_string());
            used = max_total;
            continue;
        }
        used += chars;
    }
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    input.chars().take(max_chars).collect::<String>()
}
