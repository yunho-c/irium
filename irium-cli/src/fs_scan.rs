use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Debug, Clone)]
pub struct FsEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub modified: Option<SystemTime>,
    pub size: u64,
    pub extension: Option<String>,
}

fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

pub fn scan_children(path: &Path, show_hidden: bool) -> Result<Vec<FsEntry>, String> {
    let mut entries = Vec::new();
    let dir = fs::read_dir(path).map_err(|e| format!("{}: {e}", path.display()))?;

    for item in dir {
        let entry = match item {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        let file_name = entry.file_name().to_string_lossy().to_string();
        if !show_hidden && is_hidden(&file_name) {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(md) => md,
            Err(_) => continue,
        };

        let path = entry.path();
        let is_dir = metadata.is_dir();
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase());

        entries.push(FsEntry {
            path,
            name: file_name,
            is_dir,
            modified: metadata.modified().ok(),
            size: metadata.len(),
            extension,
        });
    }

    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(entries)
}

pub fn collect_files(root: &Path, show_hidden: bool, limit: usize) -> Vec<FsEntry> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(path) = stack.pop() {
        if out.len() >= limit {
            break;
        }

        let children = match scan_children(&path, show_hidden) {
            Ok(children) => children,
            Err(_) => continue,
        };

        for child in children.into_iter().rev() {
            if child.is_dir {
                stack.push(child.path.clone());
                continue;
            }
            out.push(child);
            if out.len() >= limit {
                break;
            }
        }
    }

    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}
