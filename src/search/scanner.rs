use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// 10 MB maximum file size limit to prevent memory and processing spikes.
pub const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Directories that should always be ignored during search scanning.
const IGNORED_DIRECTORIES: &[&str] = &[
    "target",
    "node_modules",
    "dist",
    "build",
    ".git",
    ".obsidian",
    ".vscode",
    ".idea",
    ".trash",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredFile {
    pub canonical_path: PathBuf,
    pub rel_path: String,
    pub mtime: SystemTime,
    pub file_size: u64,
}

pub struct VaultScanner;

impl VaultScanner {
    /// Discovers all eligible Markdown files within `vault_root`.
    ///
    /// Confines strictly to `vault_root`. Does NOT follow symlinks that point outside `vault_root`.
    /// Skips hidden files/directories, build artifacts, files exceeding `MAX_FILE_SIZE`,
    /// and detects symlink cycles.
    pub fn scan_vault(vault_root: &Path) -> Vec<DiscoveredFile> {
        let canonical_root = match fs::canonicalize(vault_root) {
            Ok(root) => root,
            Err(_) => return Vec::new(),
        };

        let mut discovered = Vec::new();
        let mut visited_dirs = HashSet::new();
        let mut dir_stack = vec![canonical_root.clone()];

        visited_dirs.insert(canonical_root.clone());

        while let Some(current_dir) = dir_stack.pop() {
            let entries = match fs::read_dir(&current_dir) {
                Ok(entries) => entries,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().into_owned();

                // Skip hidden files and directories (starting with '.')
                if file_name.starts_with('.') {
                    continue;
                }

                let entry_path = entry.path();
                let file_type = match entry.file_type() {
                    Ok(ft) => ft,
                    Err(_) => continue,
                };

                // Check symlink safety
                let is_symlink = file_type.is_symlink();
                let canonical_entry = if is_symlink {
                    match fs::canonicalize(&entry_path) {
                        Ok(target) => {
                            // Ensure symlink stays strictly inside the vault
                            if !target.starts_with(&canonical_root) {
                                tracing::warn!(
                                    "Ignoring symlink escaping vault root: {:?}",
                                    entry_path
                                );
                                continue;
                            }
                            target
                        }
                        Err(_) => continue,
                    }
                } else {
                    entry_path.clone()
                };

                let is_dir = if is_symlink {
                    canonical_entry.is_dir()
                } else {
                    file_type.is_dir()
                };

                if is_dir {
                    // Check ignored directories
                    if IGNORED_DIRECTORIES
                        .iter()
                        .any(|&ignored| file_name.eq_ignore_ascii_case(ignored))
                    {
                        continue;
                    }

                    // Prevent directory cycles
                    if visited_dirs.insert(canonical_entry.clone()) {
                        dir_stack.push(canonical_entry);
                    }
                } else {
                    // Check file extension
                    let is_markdown = entry_path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| {
                            ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown")
                        })
                        .unwrap_or(false);

                    if !is_markdown {
                        continue;
                    }

                    let metadata = match fs::metadata(&canonical_entry) {
                        Ok(m) => m,
                        Err(_) => continue,
                    };

                    let file_size = metadata.len();
                    if file_size > MAX_FILE_SIZE {
                        tracing::warn!(
                            "Skipping file {:?} exceeding max size limit ({} bytes)",
                            canonical_entry,
                            file_size
                        );
                        continue;
                    }

                    let mtime = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);

                    // Compute relative path from vault root
                    let rel_path = match canonical_entry.strip_prefix(&canonical_root) {
                        Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
                        Err(_) => file_name,
                    };

                    discovered.push(DiscoveredFile {
                        canonical_path: canonical_entry,
                        rel_path,
                        mtime,
                        file_size,
                    });
                }
            }
        }

        // Sort by relative path for consistent results
        discovered.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
        discovered
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    struct TempTestDir(PathBuf);
    impl TempTestDir {
        fn new(prefix: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("{prefix}_{}_{}", std::process::id(), nanos));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempTestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn test_scan_vault_basic_and_filters() {
        let temp_dir = TempTestDir::new("dr_md_scan_test");
        let root = temp_dir.path();

        // Normal files
        File::create(root.join("note1.md")).unwrap();
        fs::create_dir(root.join("sub")).unwrap();
        File::create(root.join("sub").join("note2.markdown")).unwrap();

        // Ignored files
        File::create(root.join("image.png")).unwrap();
        File::create(root.join(".hidden.md")).unwrap();

        // Ignored directories
        fs::create_dir(root.join("target")).unwrap();
        File::create(root.join("target").join("build.md")).unwrap();
        fs::create_dir(root.join(".git")).unwrap();
        File::create(root.join(".git").join("config.md")).unwrap();

        let files = VaultScanner::scan_vault(root);
        let rel_paths: Vec<_> = files.iter().map(|f| f.rel_path.as_str()).collect();

        assert_eq!(rel_paths, vec!["note1.md", "sub/note2.markdown"]);
    }

    #[test]
    fn test_scan_vault_escaped_symlink() {
        let temp_dir = TempTestDir::new("dr_md_symlink_test");
        let root = temp_dir.path().join("vault");
        let outside = temp_dir.path().join("outside");

        fs::create_dir(&root).unwrap();
        fs::create_dir(&outside).unwrap();

        let secret_file = outside.join("secret.md");
        File::create(&secret_file).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let symlink_in_vault = root.join("escaped_link.md");
            symlink(&secret_file, &symlink_in_vault).unwrap();

            let files = VaultScanner::scan_vault(&root);
            assert!(files.is_empty(), "Escaped symlink must not be included");
        }
    }

    #[test]
    fn test_scan_vault_file_size_limit() {
        let temp_dir = TempTestDir::new("dr_md_size_limit_test");
        let root = temp_dir.path();

        let small = root.join("small.md");
        let mut f = File::create(&small).unwrap();
        f.write_all(b"small").unwrap();

        let large = root.join("large.md");
        let f2 = File::create(&large).unwrap();
        f2.set_len(MAX_FILE_SIZE + 1).unwrap();

        let files = VaultScanner::scan_vault(root);
        let rel_paths: Vec<_> = files.iter().map(|f| f.rel_path.as_str()).collect();

        assert_eq!(rel_paths, vec!["small.md"]);
    }
}
