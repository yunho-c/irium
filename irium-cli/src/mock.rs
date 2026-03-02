use std::time::{Duration, SystemTime};

use crate::model::{
    Capitalization, CategoryFilter, MarketplacePreset, NameLength, Separator, SizeConstraint,
    StyleOptions, TimeConstraint,
};

pub fn default_category_filters() -> Vec<CategoryFilter> {
    vec![
        CategoryFilter {
            name: "PDF Documents".to_string(),
            extensions: vec!["pdf".to_string()],
        },
        CategoryFilter {
            name: "Images".to_string(),
            extensions: vec![
                "png".to_string(),
                "jpg".to_string(),
                "jpeg".to_string(),
                "gif".to_string(),
                "webp".to_string(),
            ],
        },
        CategoryFilter {
            name: "Archives".to_string(),
            extensions: vec![
                "zip".to_string(),
                "tar".to_string(),
                "gz".to_string(),
                "rar".to_string(),
            ],
        },
        CategoryFilter {
            name: "Audio".to_string(),
            extensions: vec![
                "mp3".to_string(),
                "wav".to_string(),
                "m4a".to_string(),
                "flac".to_string(),
            ],
        },
        CategoryFilter {
            name: "Presentations".to_string(),
            extensions: vec!["ppt".to_string(), "pptx".to_string(), "key".to_string()],
        },
        CategoryFilter {
            name: "Docs".to_string(),
            extensions: vec![
                "doc".to_string(),
                "docx".to_string(),
                "txt".to_string(),
                "md".to_string(),
            ],
        },
    ]
}

pub fn default_marketplace_presets() -> Vec<MarketplacePreset> {
    vec![
        MarketplacePreset {
            name: "Academic Papers".to_string(),
            description: "PDFs and docs from the last month".to_string(),
            extensions: vec!["pdf".to_string(), "docx".to_string(), "md".to_string()],
            time_constraint: TimeConstraint::Month,
        },
        MarketplacePreset {
            name: "Wallpapers".to_string(),
            description: "Image assets for desktop/mobile".to_string(),
            extensions: vec![
                "png".to_string(),
                "jpg".to_string(),
                "jpeg".to_string(),
                "webp".to_string(),
            ],
            time_constraint: TimeConstraint::Any,
        },
        MarketplacePreset {
            name: "Slide Decks".to_string(),
            description: "Presentation files from the last week".to_string(),
            extensions: vec!["ppt".to_string(), "pptx".to_string(), "key".to_string()],
            time_constraint: TimeConstraint::Week,
        },
    ]
}

fn split_name_words(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    for ch in name.chars() {
        if ch.is_alphanumeric() {
            current.push(ch);
        } else if !current.is_empty() {
            words.push(current.clone());
            current.clear();
        }
    }

    if !current.is_empty() {
        words.push(current);
    }

    if words.is_empty() {
        return vec!["file".to_string()];
    }

    words
}

fn apply_length(mut words: Vec<String>, length: NameLength) -> Vec<String> {
    let max_words = match length {
        NameLength::None => words.len(),
        NameLength::Long => 8,
        NameLength::Medium => 4,
        NameLength::Short => 2,
    };
    words.truncate(max_words);

    let max_chars = match length {
        NameLength::None => usize::MAX,
        NameLength::Long => 48,
        NameLength::Medium => 24,
        NameLength::Short => 12,
    };

    let mut total = 0usize;
    for word in &mut words {
        if total >= max_chars {
            *word = String::new();
            continue;
        }
        let room = max_chars - total;
        let word_chars = word.chars().count();
        if word_chars > room {
            truncate_to_char_count(word, room);
        }
        total += word.chars().count();
    }

    words.into_iter().filter(|w| !w.is_empty()).collect()
}

fn truncate_to_char_count(value: &mut String, max_chars: usize) {
    if value.chars().count() <= max_chars {
        return;
    }

    let truncate_at = value
        .char_indices()
        .nth(max_chars)
        .map(|(idx, _)| idx)
        .unwrap_or(value.len());
    value.truncate(truncate_at);
}

fn apply_capitalization(words: &[String], capitalization: Capitalization) -> Vec<String> {
    words
        .iter()
        .map(|word| match capitalization {
            Capitalization::None => word.clone(),
            Capitalization::Lower => word.to_lowercase(),
            Capitalization::Upper => word.to_uppercase(),
            Capitalization::Title => {
                let lower = word.to_lowercase();
                let mut chars = lower.chars();
                match chars.next() {
                    Some(first) => {
                        let mut out = first.to_uppercase().to_string();
                        out.push_str(chars.as_str());
                        out
                    }
                    None => lower,
                }
            }
        })
        .collect()
}

fn join_words(words: &[String], separator: Separator) -> String {
    let sep = match separator {
        Separator::Space => " ",
        Separator::Dash => "-",
        Separator::Underscore => "_",
    };
    words.join(sep)
}

pub fn format_name(base_name: &str, extension: Option<&str>, style: &StyleOptions) -> String {
    let mut working = base_name.to_string();
    if style.strip_colons {
        working = working.replace(':', " ");
    }

    let words = split_name_words(&working);
    let words = apply_length(words, style.length);
    let words = apply_capitalization(&words, style.capitalization);

    let mut out = join_words(&words, style.separator);
    if out.is_empty() {
        out = "file".to_string();
    }

    if style.keep_extension
        && let Some(ext) = extension
    {
        out.push('.');
        out.push_str(ext);
    }

    out
}

pub fn default_suggestion_labels() -> Vec<String> {
    vec![
        "Option 1".to_string(),
        "Option 2".to_string(),
        "Option 3".to_string(),
    ]
}

pub fn within_time_constraint(modified: Option<SystemTime>, constraint: TimeConstraint) -> bool {
    match constraint {
        TimeConstraint::Any | TimeConstraint::Custom => true,
        TimeConstraint::Day => within_duration(modified, Duration::from_secs(60 * 60 * 24)),
        TimeConstraint::Week => within_duration(modified, Duration::from_secs(60 * 60 * 24 * 7)),
        TimeConstraint::Month => within_duration(modified, Duration::from_secs(60 * 60 * 24 * 30)),
    }
}

fn within_duration(modified: Option<SystemTime>, threshold: Duration) -> bool {
    let Some(modified) = modified else {
        return false;
    };

    match SystemTime::now().duration_since(modified) {
        Ok(delta) => delta <= threshold,
        Err(_) => true,
    }
}

pub fn within_size_constraint(size: u64, constraint: SizeConstraint) -> bool {
    match constraint {
        SizeConstraint::Any => true,
        SizeConstraint::Small => size < 5 * 1024 * 1024,
        SizeConstraint::Medium => (5 * 1024 * 1024..=100 * 1024 * 1024).contains(&size),
        SizeConstraint::Large => size > 100 * 1024 * 1024,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_name_applies_separator_and_caps() {
        let style = StyleOptions {
            length: NameLength::Medium,
            capitalization: Capitalization::Upper,
            separator: Separator::Underscore,
            keep_extension: true,
            strip_colons: true,
        };
        let formatted = format_name("quarterly report:final", Some("pdf"), &style);
        assert_eq!(formatted, "QUARTERLY_REPORT_FINAL.pdf");
    }

    #[test]
    fn format_name_shortens_for_short_mode() {
        let style = StyleOptions {
            length: NameLength::Short,
            capitalization: Capitalization::Lower,
            separator: Separator::Dash,
            keep_extension: false,
            strip_colons: true,
        };
        let formatted = format_name("Very Long File Name With Many Words", None, &style);
        assert!(formatted.len() <= 12);
    }

    #[test]
    fn format_name_handles_multibyte_chars_without_panicking() {
        let style = StyleOptions {
            length: NameLength::Short,
            capitalization: Capitalization::Title,
            separator: Separator::Space,
            keep_extension: true,
            strip_colons: true,
        };
        let formatted = format_name("가나다라마바사-테스트🙂파일", Some("txt"), &style);
        assert!(formatted.ends_with(".txt"));
        assert!(formatted.chars().count() <= 16);
    }
}
