//! Test files come from the directories in `SIB_CORPUS` (separated like
//! `PATH`), searched recursively. None are distributed with the crate; without
//! the variable the corpus tests pass vacuously.

use std::path::{Path, PathBuf};

fn sib_files(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|entries| entries.filter_map(|e| e.ok().map(|e| e.path())).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .flat_map(|p| if p.is_dir() { sib_files(&p) } else { vec![p] })
        .filter(|p| p.extension().is_some_and(|e| e == "sib"))
        .collect()
}

pub fn corpus() -> Vec<PathBuf> {
    let dirs = std::env::var_os("SIB_CORPUS").unwrap_or_default();
    let mut files: Vec<PathBuf> = std::env::split_paths(&dirs).flat_map(|d| sib_files(&d)).collect();
    files.sort();
    files
}
