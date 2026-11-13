use crate::search::scanner::VaultScanner;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

/// Compact metadata for an indexed file. Full file contents are NEVER stored in memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedFileMetadata {
    pub rel_path: String,
    pub mtime: SystemTime,
    pub file_size: u64,
    pub tags: Vec<String>,
    pub headings: Vec<String>,
}

#[derive(Debug, Default)]
pub struct SearchIndex {
    pub files: HashMap<PathBuf, IndexedFileMetadata>,
}

impl SearchIndex {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn get(&self, path: &Path) -> Option<&IndexedFileMetadata> {
        self.files.get(path)
    }

    /// Incrementally refreshes the index for `vault_root`.
    ///
    /// Skips unchanged files (based on mtime and file_size).
    /// Extracts tags and headings incrementally.
    /// Prunes deleted files.
    /// Supports cancellation via `cancel_token`.
    pub fn refresh<F>(
        &mut self,
        vault_root: &Path,
        cancel_token: Option<&Arc<AtomicU64>>,
        expected_generation: u64,
        mut on_progress: F,
    ) -> bool
    where
        F: FnMut(usize, usize),
    {
        let discovered = VaultScanner::scan_vault(vault_root);
        let total = discovered.len();

        let mut seen_paths = HashMap::new();

        for (idx, disc) in discovered.into_iter().enumerate() {
            if let Some(token) = cancel_token
                && token.load(Ordering::Relaxed) != expected_generation
            {
                return false; // Cancelled
            }

            on_progress(idx + 1, total);

            // Check if already indexed and unchanged
            if let Some(existing) = self.files.get(&disc.canonical_path)
                && existing.mtime == disc.mtime
                && existing.file_size == disc.file_size
            {
                seen_paths.insert(disc.canonical_path, existing.clone());
                continue;
            }

            // Otherwise, extract metadata incrementally
            let (tags, headings) = extract_file_metadata(&disc.canonical_path);
            let meta = IndexedFileMetadata {
                rel_path: disc.rel_path,
                mtime: disc.mtime,
                file_size: disc.file_size,
                tags,
                headings,
            };

            seen_paths.insert(disc.canonical_path, meta);
        }

        // Replace files map with updated/pruned entries
        self.files = seen_paths;
        true
    }
}

/// Reads a Markdown file incrementally to extract tags and headings.
///
/// Bounded buffer reading; handles invalid UTF-8 losslessly and never loads the entire file into memory.
fn extract_file_metadata(path: &Path) -> (Vec<String>, Vec<String>) {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return (Vec::new(), Vec::new()),
    };

    let reader = BufReader::new(file);
    let mut tags = Vec::new();
    let mut headings = Vec::new();

    let mut in_frontmatter = false;
    let mut line_index = 0;
    let mut in_tags_block = false;

    for line_res in reader.lines() {
        let line = match line_res {
            Ok(l) => l,
            Err(_) => continue, // Lossy/skip unreadable lines safely
        };

        let trimmed = line.trim();

        // Frontmatter detection
        if line_index == 0 && trimmed == "---" {
            in_frontmatter = true;
            line_index += 1;
            continue;
        }

        if in_frontmatter {
            if trimmed == "---" {
                in_frontmatter = false;
                in_tags_block = false;
                line_index += 1;
                continue;
            }

            // Parse tags in frontmatter: `tags: [tag1, tag2]` or `tags: tag1, tag2` or list items `- tag1`
            if trimmed.starts_with("tags:") || trimmed.starts_with("tag:") {
                let rest = trimmed
                    .trim_start_matches("tags:")
                    .trim_start_matches("tag:")
                    .trim();
                if rest.is_empty() {
                    in_tags_block = true;
                } else {
                    let cleaned = rest.trim_matches(|c| c == '[' || c == ']');
                    for part in cleaned.split(',') {
                        let t = part
                            .trim()
                            .trim_matches('"')
                            .trim_matches('\'')
                            .trim_start_matches('#');
                        if !t.is_empty() && !tags.contains(&t.to_string()) {
                            tags.push(t.to_string());
                        }
                    }
                }
            } else if in_tags_block && trimmed.starts_with('-') {
                let t = trimmed
                    .trim_start_matches('-')
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .trim_start_matches('#');
                if !t.is_empty() && !tags.contains(&t.to_string()) {
                    tags.push(t.to_string());
                }
            } else if !trimmed.starts_with('-') {
                in_tags_block = false;
            }

            line_index += 1;
            continue;
        }

        line_index += 1;

        // Heading detection: lines starting with `# `, `## `, etc.
        if trimmed.starts_with('#') {
            let hash_count = trimmed.chars().take_while(|&c| c == '#').count();
            if hash_count <= 6 && trimmed[hash_count..].starts_with(' ') {
                let heading_text = trimmed[hash_count..].trim();
                if !heading_text.is_empty() && !headings.contains(&heading_text.to_string()) {
                    headings.push(heading_text.to_string());
                }
            }
        }

        // Inline tag extraction: `#tag`
        extract_inline_tags(&line, &mut tags);
    }

    (tags, headings)
}

fn extract_inline_tags(line: &str, tags: &mut Vec<String>) {
    let mut chars = line.char_indices().peekable();

    while let Some((idx, c)) = chars.next() {
        if c == '#' {
            // Check preceding char: must be start of line or whitespace/punctuation
            if idx > 0 {
                let prev_char = line[..idx].chars().last().unwrap();
                if prev_char.is_alphanumeric() || prev_char == '#' {
                    continue;
                }
            }

            // Read tag body: alphanumeric, '_', '-', '/'
            let mut tag_buf = String::new();
            while let Some(&(_, next_c)) = chars.peek() {
                if next_c.is_alphanumeric() || next_c == '_' || next_c == '-' || next_c == '/' {
                    tag_buf.push(next_c);
                    chars.next();
                } else {
                    break;
                }
            }

            // Tag must not be purely numeric or empty
            if !tag_buf.is_empty()
                && !tag_buf.chars().all(|ch| ch.is_numeric())
                && !tags.contains(&tag_buf)
            {
                tags.push(tag_buf);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
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
    fn test_extract_metadata_frontmatter_and_inline() {
        let temp_dir = TempTestDir::new("dr_md_meta_test");
        let file_path = temp_dir.path().join("test.md");

        let content = r#"---
title: My Note
tags: [rust, project/deep]
---

# Main Heading

Here is some text with an inline #daily-note and #todo tag.
## Section 2 heading
"#;
        let mut f = File::create(&file_path).unwrap();
        f.write_all(content.as_bytes()).unwrap();

        let (tags, headings) = extract_file_metadata(&file_path);

        assert!(tags.contains(&"rust".to_string()));
        assert!(tags.contains(&"project/deep".to_string()));
        assert!(tags.contains(&"daily-note".to_string()));
        assert!(tags.contains(&"todo".to_string()));

        assert_eq!(headings, vec!["Main Heading", "Section 2 heading"]);
    }

    #[test]
    fn test_incremental_index_refresh() {
        let temp_dir = TempTestDir::new("dr_md_index_test");
        let root = temp_dir.path();

        let note1 = root.join("note1.md");
        fs::write(&note1, "# Heading 1\n#tag1").unwrap();

        let mut index = SearchIndex::new();
        let refreshed = index.refresh(root, None, 0, |_, _| {});
        assert!(refreshed);
        assert_eq!(index.len(), 1);

        let meta = index.get(&fs::canonicalize(&note1).unwrap()).unwrap();
        assert_eq!(meta.headings, vec!["Heading 1"]);
        assert_eq!(meta.tags, vec!["tag1"]);

        // Modify note1
        std::thread::sleep(std::time::Duration::from_millis(50));
        fs::write(&note1, "# Heading 2\n#tag2").unwrap();

        index.refresh(root, None, 0, |_, _| {});
        let meta2 = index.get(&fs::canonicalize(&note1).unwrap()).unwrap();
        assert_eq!(meta2.headings, vec!["Heading 2"]);
        assert_eq!(meta2.tags, vec!["tag2"]);

        // Delete note1
        fs::remove_file(&note1).unwrap();
        index.refresh(root, None, 0, |_, _| {});
        assert_eq!(index.len(), 0);
    }
}
