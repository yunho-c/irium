use std::path::PathBuf;

pub mod analyze;
pub mod models;
pub mod suggest;
pub mod worker;

pub const MAX_FILES_PER_RUN: usize = 80;
pub const MAX_EXCERPT_CHARS_PER_FILE: usize = 2_000;
pub const MAX_TOTAL_CONTEXT_CHARS: usize = 120_000;

#[derive(Debug, Clone)]
pub struct ModelListItem {
    pub id: String,
    pub name: String,
    pub context_length: Option<u64>,
    pub pricing_prompt: Option<String>,
    pub pricing_completion: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AnalyzedFileContext {
    pub path: PathBuf,
    pub current_name: String,
    pub extension: Option<String>,
    pub size: u64,
    pub modified_unix: Option<i64>,
    pub mime_type: Option<String>,
    pub detected_languages: Vec<String>,
    pub excerpt: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AnalyzeOutput {
    pub files: Vec<AnalyzedFileContext>,
    pub skipped_count: usize,
    pub warnings: Vec<String>,
}
