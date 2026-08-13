use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use colored::Colorize;
use tracing::{debug, error, info, trace, warn};

use crate::hash::hash_hex;
use crate::io::{file_name_of, get_local_dir, walk_files};

pub struct DecodeRequest<'a> {
    pub names: &'a [&'a str],
    pub directory: Option<(PathBuf, usize)>,
    pub dictionary: Option<&'a str>,
    pub print: bool,
    pub rename: Option<usize>,
}

struct Mapping {
    order: Vec<String>,
    names: HashMap<String, String>,
}

impl Mapping {
    fn new() -> Self {
        Self {
            order: Vec::new(),
            names: HashMap::new(),
        }
    }

    fn insert(&mut self, digest: String, name: String) {
        if self.names.contains_key(&digest) {
            trace!(hash = %digest, name = %name, "Ignoring duplicate hash");
            return;
        }

        self.order.push(digest.clone());
        self.names.insert(digest, name);
    }

    fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
}

pub fn execute(request: &DecodeRequest, show_ui: bool) {
    let mut mapping = Mapping::new();

    for name in request.names {
        mapping.insert(hash_hex(name), (*name).to_string());
    }

    if let Some((directory, level)) = &request.directory {
        if !directory.is_dir() {
            if show_ui {
                println!("\n  {} Directory not found: {}\n", "✗".red(), directory.display());
            }
            error!(path = %directory.display(), "Directory not found");
            std::process::exit(1);
        }

        debug!(path = %directory.display(), level, "Walking directory for asset names");

        for path in walk_files(directory, *level) {
            if let Some(name) = file_name_of(&path) {
                mapping.insert(hash_hex(name), name.to_string());
            }
        }
    }

    if let Some(dictionary_path) = request.dictionary {
        match load_dictionary(Path::new(dictionary_path)) {
            Ok(pairs) => {
                for (digest, name) in pairs {
                    mapping.insert(digest, name);
                }
            }
            Err(err) => {
                if show_ui {
                    println!("\n  {} Could not read dictionary: {}\n", "✗".red(), err);
                }
                error!(path = %dictionary_path, error = %err, "Could not read dictionary");
                std::process::exit(1);
            }
        }
    }

    if mapping.is_empty() {
        if show_ui {
            println!("\n  {} No names resolved from the given input\n", "!".yellow());
        }
        warn!("No names resolved from the given input");
        std::process::exit(1);
    }

    if request.print {
        print_mapping(&mapping, show_ui);
    }

    if let Some(level) = request.rename {
        rename_matches(&mapping, level, show_ui);
    }
}

fn print_mapping(mapping: &Mapping, show_ui: bool) {
    if show_ui {
        println!();
    }

    for digest in &mapping.order {
        let Some(name) = mapping.names.get(digest) else {
            continue;
        };

        if show_ui {
            println!("  {}  {}", digest.cyan(), name);
        }
        info!(hash = %digest, name = %name, "Resolved asset hash");
    }

    if show_ui {
        println!("\nSUCCESS: Hashed {} names!\n", mapping.order.len().to_string().cyan());
    }
}

fn rename_matches(mapping: &Mapping, level: usize, show_ui: bool) {
    let root = get_local_dir();
    debug!(path = %root.display(), level, "Walking working directory for hashed files");

    let mut renamed_count = 0;
    let mut skipped_count = 0;

    for path in walk_files(&root, level) {
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };

        let Some(proper_name) = mapping.names.get(&stem.to_ascii_lowercase()) else {
            continue;
        };

        let Some(parent) = path.parent() else {
            continue;
        };

        let target = parent.join(restore_name(proper_name, &path));

        if target == path {
            continue;
        }

        if target.exists() {
            if show_ui {
                println!(
                    "  {} Skipped {} ({} already exists)",
                    "!".yellow(),
                    stem.cyan(),
                    proper_name.cyan()
                );
            }
            warn!(hash = %stem, name = %proper_name, "Rename target already exists");
            skipped_count += 1;
            continue;
        }

        if let Err(err) = fs::rename(&path, &target) {
            if show_ui {
                println!("  {} Failed to rename {}: {}", "✗".red(), stem.cyan(), err);
            }
            error!(hash = %stem, error = %err, "Failed to rename file");
            skipped_count += 1;
            continue;
        }

        renamed_count += 1;
        trace!(from = %path.display(), to = %target.display(), "Renamed hashed file");
    }

    if renamed_count == 0 {
        if show_ui {
            println!("\nFAILURE: Renamed no files!\n");
        }
        error!(skipped = skipped_count, "Renamed no files");
        std::process::exit(1);
    }

    if show_ui {
        println!("\nSUCCESS: Renamed {} files!\n", renamed_count.to_string().cyan());
    }
    info!(renamed = renamed_count, skipped = skipped_count, "Rename pass complete");
}

fn restore_name(proper_name: &str, hashed_path: &Path) -> String {
    let Some(extension) = hashed_path.extension().and_then(|ext| ext.to_str()) else {
        return proper_name.to_string();
    };

    let suffix = format!(".{extension}");
    if proper_name.to_ascii_lowercase().ends_with(&suffix.to_ascii_lowercase()) {
        return proper_name.to_string();
    }

    format!("{proper_name}{suffix}")
}

fn load_dictionary(path: &Path) -> Result<Vec<(String, String)>, String> {
    let content = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let mut pairs = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let Some((digest, name)) = line.split_once(',') else {
            trace!(line = %line, "Ignoring malformed dictionary line");
            continue;
        };

        let digest = digest.trim().to_ascii_lowercase();
        let name = name.trim();

        if digest.is_empty() || name.is_empty() {
            continue;
        }

        pairs.push((digest, name.to_string()));
    }

    if pairs.is_empty() {
        return Err(String::from("Dictionary holds no usable entries"));
    }

    Ok(pairs)
}
