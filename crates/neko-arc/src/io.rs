use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_LEVEL: usize = 10;

pub fn get_local_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn walk_files(root: &Path, level: usize) -> Vec<PathBuf> {
    let mut found = Vec::new();
    collect_files(root, 1, level.max(1), &mut found);
    found.sort();
    found
}

fn collect_files(dir: &Path, depth: usize, level: usize, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            if depth < level {
                collect_files(&path, depth + 1, level, found);
            }
            continue;
        }

        found.push(path);
    }
}

pub fn file_name_of(path: &Path) -> Option<&str> {
    path.file_name().and_then(|name| name.to_str())
}

pub fn has_extension(path: &Path, wanted: &str) -> bool {
    path.extension()
        .is_some_and(|ext| ext.to_str().is_some_and(|ext| ext.eq_ignore_ascii_case(wanted)))
}
