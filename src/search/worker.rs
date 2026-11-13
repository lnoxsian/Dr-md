use crate::search::index::SearchIndex;
use crate::search::query::SearchQuery;
use crate::search::result::{FileSearchResult, MatchLocation};
use crate::search::scanner::VaultScanner;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub const MAX_TOTAL_MATCHES: usize = 5000;
pub const MAX_MATCHES_PER_FILE: usize = 100;
pub const MAX_SNIPPET_LEN: usize = 120;

#[derive(Debug, Clone)]
pub enum SearchEvent {
    IndexingProgress {
        current: usize,
        total: usize,
    },
    PartialResults {
        generation: u64,
        results: Vec<FileSearchResult>,
    },
    Complete {
        generation: u64,
        total_matches: usize,
        total_files: usize,
        elapsed_ms: u128,
    },
    Error {
        generation: u64,
        message: String,
    },
}

pub struct SearchWorker;

impl SearchWorker {
    pub fn run(
        vault_root: PathBuf,
        query: SearchQuery,
        generation: u64,
        cancel_token: Arc<AtomicU64>,
        tx: Sender<SearchEvent>,
        index_lock: Arc<Mutex<SearchIndex>>,
    ) {
        let start_time = Instant::now();

        // Check cancellation
        if cancel_token.load(Ordering::Relaxed) != generation {
            return;
        }

        // 1. Ensure index is refreshed if empty
        let is_index_empty = {
            let index = match index_lock.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            index.is_empty()
        };

        if is_index_empty {
            let mut index = match index_lock.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };

            let tx_prog = tx.clone();
            let refreshed = index.refresh(
                &vault_root,
                Some(&cancel_token),
                generation,
                move |current, total| {
                    let _ = tx_prog.send(SearchEvent::IndexingProgress { current, total });
                },
            );

            if !refreshed {
                return; // Cancelled
            }
        }

        if cancel_token.load(Ordering::Relaxed) != generation {
            return;
        }

        let index_snapshot = match index_lock.lock() {
            Ok(guard) => guard.files.clone(),
            Err(poisoned) => poisoned.into_inner().files.clone(),
        };

        // 2. Discover files to search (use index snapshot if available for instant in-memory lookup)
        let discovered: Vec<crate::search::scanner::DiscoveredFile> = if !index_snapshot.is_empty()
        {
            index_snapshot
                .iter()
                .map(|(path, meta)| crate::search::scanner::DiscoveredFile {
                    canonical_path: path.clone(),
                    rel_path: meta.rel_path.clone(),
                    mtime: meta.mtime,
                    file_size: meta.file_size,
                })
                .collect()
        } else {
            VaultScanner::scan_vault(&vault_root)
        };

        let mut total_matches = 0;
        let mut total_files = 0;
        let mut partial_batch = Vec::new();

        for disc in discovered {
            if cancel_token.load(Ordering::Relaxed) != generation {
                return; // Superseded
            }

            if total_matches >= MAX_TOTAL_MATCHES {
                break; // Reached global match cap
            }

            // Apply cheap filters first
            let rel_lower = disc.rel_path.to_lowercase();
            if let Some(ref pf) = query.path_filter
                && !rel_lower.contains(&pf.to_lowercase())
            {
                continue;
            }

            let file_name = disc
                .canonical_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if let Some(ref ff) = query.file_filter
                && !file_name.to_lowercase().contains(&ff.to_lowercase())
            {
                continue;
            }

            // Tag filter
            if !query.tag_filters.is_empty() {
                if let Some(meta) = index_snapshot.get(&disc.canonical_path) {
                    let has_all_tags = query
                        .tag_filters
                        .iter()
                        .all(|req_tag| meta.tags.iter().any(|t| t.eq_ignore_ascii_case(req_tag)));
                    if !has_all_tags {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            // Search file content
            let file_result = search_single_file(
                &disc.canonical_path,
                &disc.rel_path,
                &query,
                MAX_MATCHES_PER_FILE,
            );

            if let Some(res) = file_result
                && !res.matches.is_empty()
            {
                total_matches += res.matches.len();
                total_files += 1;
                partial_batch.push(res);

                // Stream partial results immediately so matches appear in real-time
                let _ = tx.send(SearchEvent::PartialResults {
                    generation,
                    results: std::mem::take(&mut partial_batch),
                });
            }
        }

        // Flush remaining partial results
        if !partial_batch.is_empty() {
            let _ = tx.send(SearchEvent::PartialResults {
                generation,
                results: partial_batch,
            });
        }

        if cancel_token.load(Ordering::Relaxed) == generation {
            let elapsed_ms = start_time.elapsed().as_millis();
            let _ = tx.send(SearchEvent::Complete {
                generation,
                total_matches,
                total_files,
                elapsed_ms,
            });
        }
    }
}

/// Searches a single file according to `SearchQuery`.
fn search_single_file(
    file_path: &Path,
    rel_path: &str,
    query: &SearchQuery,
    max_matches: usize,
) -> Option<FileSearchResult> {
    // If no text terms, regex, or line filters: matching by metadata alone
    if query.terms.is_empty() && query.line_filter.is_none() && query.compiled_regex.is_none() {
        return Some(FileSearchResult {
            file_path: file_path.to_path_buf(),
            rel_path: rel_path.to_string(),
            matches: vec![MatchLocation {
                line_number: 1,
                col_number: 1,
                char_offset: 0,
                match_length: 0,
                snippet: rel_path.to_string(),
                snippet_match_range: 0..rel_path.len(),
            }],
            is_collapsed: false,
        });
    }

    let file = match File::open(file_path) {
        Ok(f) => f,
        Err(_) => return None,
    };

    let mut reader = BufReader::new(file);
    let mut line_buf = String::new();
    let mut line_number = 1;
    let mut char_offset = 0;

    let mut file_matches = Vec::new();
    let mut terms_found_in_file = HashSet::new();

    // Check if filename / rel_path matches terms or regex
    let mut filename_matches = Vec::new();
    if let Some(ref re) = query.compiled_regex {
        if let Some(m) = re.find(rel_path) {
            let col_start = rel_path[..m.start()].chars().count();
            let match_len = m.as_str().chars().count();
            filename_matches.push((col_start, col_start + match_len));
        }
    } else {
        for (term_idx, term) in query.terms.iter().enumerate() {
            let matches_in_name =
                find_term_matches(rel_path, term, query.case_sensitive, query.whole_word);
            if !matches_in_name.is_empty() {
                terms_found_in_file.insert(term_idx);
                for range in matches_in_name {
                    filename_matches.push((range.start, range.end));
                }
            }
        }
    }

    loop {
        line_buf.clear();
        match reader.read_line(&mut line_buf) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(_) => {
                // If invalid UTF-8 or I/O error, skip line safely
                line_number += 1;
                continue;
            }
        }

        let line_chars_count = line_buf.chars().count();
        let trimmed_line = line_buf.trim_end_matches(['\r', '\n']);

        // Check line filter if present
        if let Some(ref lf) = query.line_filter {
            let contains_lf = if query.case_sensitive {
                trimmed_line.contains(lf)
            } else {
                trimmed_line.to_lowercase().contains(&lf.to_lowercase())
            };
            if !contains_lf {
                char_offset += line_chars_count;
                line_number += 1;
                continue;
            }
        }

        // Search in this line
        if let Some(ref re) = query.compiled_regex {
            for m in re.find_iter(trimmed_line) {
                // Regex byte offsets -> char offsets
                let col_start = trimmed_line[..m.start()].chars().count();
                let match_len = m.as_str().chars().count();

                let (snippet, snippet_range) =
                    create_snippet(trimmed_line, col_start, col_start + match_len);

                file_matches.push(MatchLocation {
                    line_number,
                    col_number: col_start + 1,
                    char_offset: char_offset + col_start,
                    match_length: match_len,
                    snippet,
                    snippet_match_range: snippet_range,
                });

                if file_matches.len() >= max_matches {
                    break;
                }
            }
        } else {
            // Plain text matching
            for (term_idx, term) in query.terms.iter().enumerate() {
                let matches_in_line =
                    find_term_matches(trimmed_line, term, query.case_sensitive, query.whole_word);

                if !matches_in_line.is_empty() {
                    terms_found_in_file.insert(term_idx);

                    for range in matches_in_line {
                        if file_matches.len() < max_matches {
                            let match_len = range.end - range.start;
                            let (snippet, snippet_range) =
                                create_snippet(trimmed_line, range.start, range.end);

                            file_matches.push(MatchLocation {
                                line_number,
                                col_number: range.start + 1,
                                char_offset: char_offset + range.start,
                                match_length: match_len,
                                snippet,
                                snippet_match_range: snippet_range,
                            });
                        }
                    }
                }
            }
        }

        char_offset += line_chars_count;
        line_number += 1;
    }

    // Document-level AND check: all terms must be found in the note
    if query.compiled_regex.is_none()
        && !query.terms.is_empty()
        && terms_found_in_file.len() < query.terms.len()
    {
        return None; // Missing one or more required terms
    }

    // If no content lines matched, but filename matched, create a file match item
    if file_matches.is_empty() && !filename_matches.is_empty() {
        let snippet = rel_path.to_string();
        let snippet_range = if let Some(&(start, end)) = filename_matches.first() {
            start..end
        } else {
            0..snippet.chars().count()
        };

        file_matches.push(MatchLocation {
            line_number: 1,
            col_number: 1,
            char_offset: 0,
            match_length: 0,
            snippet,
            snippet_match_range: snippet_range,
        });
    }

    if file_matches.is_empty() {
        None
    } else {
        Some(FileSearchResult {
            file_path: file_path.to_path_buf(),
            rel_path: rel_path.to_string(),
            matches: file_matches,
            is_collapsed: false,
        })
    }
}

/// Finds all character ranges matching `term` in `line`.
fn find_term_matches(
    line: &str,
    term: &str,
    case_sensitive: bool,
    whole_word: bool,
) -> Vec<Range<usize>> {
    let mut results = Vec::new();
    let term_chars: Vec<char> = term.chars().collect();
    let term_char_len = term_chars.len();
    if term_char_len == 0 {
        return results;
    }

    let line_chars: Vec<char> = line.chars().collect();
    let line_char_len = line_chars.len();
    if line_char_len < term_char_len {
        return results;
    }

    for i in 0..=(line_char_len - term_char_len) {
        let slice = &line_chars[i..i + term_char_len];
        let matches = if case_sensitive {
            slice == term_chars.as_slice()
        } else {
            slice
                .iter()
                .zip(term_chars.iter())
                .all(|(a, b)| a.to_lowercase().eq(b.to_lowercase()))
        };

        if matches {
            if whole_word {
                let prev_ok = if i == 0 {
                    true
                } else {
                    let prev_c = line_chars[i - 1];
                    !prev_c.is_alphanumeric() && prev_c != '_'
                };

                let next_ok = if i + term_char_len >= line_char_len {
                    true
                } else {
                    let next_c = line_chars[i + term_char_len];
                    !next_c.is_alphanumeric() && next_c != '_'
                };

                if prev_ok && next_ok {
                    results.push(i..i + term_char_len);
                }
            } else {
                results.push(i..i + term_char_len);
            }
        }
    }

    results
}

/// Creates a concise context snippet around `match_start..match_end` (in char indices).
fn create_snippet(line: &str, match_start: usize, match_end: usize) -> (String, Range<usize>) {
    let line_chars: Vec<char> = line.chars().collect();
    let total_len = line_chars.len();

    if total_len <= MAX_SNIPPET_LEN {
        return (line.to_string(), match_start..match_end);
    }

    // Centered window around match
    let match_len = match_end - match_start;
    let context_each_side = (MAX_SNIPPET_LEN.saturating_sub(match_len)) / 2;

    let start_char = match_start.saturating_sub(context_each_side);
    let end_char = (match_end + context_each_side).min(total_len);

    let prefix = if start_char > 0 { "..." } else { "" };
    let suffix = if end_char < total_len { "..." } else { "" };

    let snippet_body: String = line_chars[start_char..end_char].iter().collect();
    let snippet = format!("{prefix}{snippet_body}{suffix}");

    let prefix_char_count = prefix.chars().count();
    let snippet_match_start = prefix_char_count + (match_start - start_char);
    let snippet_match_end = snippet_match_start + match_len;

    (snippet, snippet_match_start..snippet_match_end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_term_matches_plain_and_case() {
        let line = "Hello world, hello Rust";
        let matches = find_term_matches(line, "hello", false, false);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0], 0..5);
        assert_eq!(matches[1], 13..18);

        let case_matches = find_term_matches(line, "Hello", true, false);
        assert_eq!(case_matches.len(), 1);
        assert_eq!(case_matches[0], 0..5);
    }

    #[test]
    fn test_find_term_matches_whole_word() {
        let line = "rust is trustworthy, rust_lang or rust";
        let matches = find_term_matches(line, "rust", false, true);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0], 0..4);
        assert_eq!(matches[1], 34..38);
    }

    #[test]
    fn test_create_snippet() {
        let line = "Short line with match";
        let (snippet, range) = create_snippet(line, 16, 21);
        assert_eq!(snippet, line);
        assert_eq!(range, 16..21);

        let long_line = "word ".repeat(50);
        let (snip, rng) = create_snippet(&long_line, 50, 54);
        assert!(snip.len() <= MAX_SNIPPET_LEN + 10);
        let matched_text: String = snip
            .chars()
            .skip(rng.start)
            .take(rng.end - rng.start)
            .collect();
        assert_eq!(matched_text, "word");
    }
}
