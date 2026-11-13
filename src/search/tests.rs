use super::*;
use crate::app::commands;
use crate::app::state::AppState;
use crate::search::worker::{MAX_SNIPPET_LEN, SearchEvent};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::Instant;

struct TempTestVault {
    path: PathBuf,
}

impl TempTestVault {
    fn new(prefix: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}_{}_{}", std::process::id(), nanos));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write_file(&self, rel: &str, content: &str) -> PathBuf {
        let full = self.path.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = File::create(&full).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        full
    }
}

impl Drop for TempTestVault {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn execute_search(
    vault_root: &Path,
    query_str: &str,
    case: bool,
    word: bool,
    regex: bool,
) -> Result<Vec<FileSearchResult>, QueryError> {
    let query = SearchQuery::parse(query_str, case, word, regex)?;
    let (tx, rx) = channel();
    let index = Arc::new(Mutex::new(SearchIndex::new()));
    let cancel = Arc::new(AtomicU64::new(1));

    SearchWorker::run(vault_root.to_path_buf(), query, 1, cancel, tx, index);

    let mut results = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let SearchEvent::PartialResults { results: batch, .. } = event {
            for res in batch {
                if let Some(existing) = results
                    .iter_mut()
                    .find(|r: &&mut FileSearchResult| r.file_path == res.file_path)
                {
                    existing.matches.extend(res.matches);
                } else {
                    results.push(res);
                }
            }
        }
    }

    Ok(results)
}

#[test]
fn test_plain_search_multi_file() {
    let vault = TempTestVault::new("test_plain_search");
    vault.write_file(
        "note1.md",
        "Dr-md is a high performance editor.\nFast search.",
    );
    vault.write_file("sub/note2.md", "Markdown files are fast.\nAnother line.");
    vault.write_file("sub/other.txt", "fast in txt is ignored");

    let results = execute_search(vault.path(), "fast", false, false, false).unwrap();
    assert_eq!(results.len(), 2);

    let total_matches: usize = results.iter().map(|r| r.matches.len()).sum();
    assert_eq!(total_matches, 2);
}

#[test]
fn test_case_sensitive_search() {
    let vault = TempTestVault::new("test_case_search");
    vault.write_file("note.md", "Rust and rust are different cases.");

    let insensitive = execute_search(vault.path(), "rust", false, false, false).unwrap();
    assert_eq!(insensitive[0].matches.len(), 2);

    let sensitive = execute_search(vault.path(), "Rust", true, false, false).unwrap();
    assert_eq!(sensitive[0].matches.len(), 1);
    assert_eq!(sensitive[0].matches[0].col_number, 1);

    // Operator override
    let op_sensitive = execute_search(vault.path(), "rust case:true", false, false, false).unwrap();
    assert_eq!(op_sensitive[0].matches.len(), 1);
    assert_eq!(op_sensitive[0].matches[0].col_number, 10);
}

#[test]
fn test_whole_word_search() {
    let vault = TempTestVault::new("test_word_search");
    vault.write_file("note.md", "rust trustworthy rust_lang rust.");

    let non_whole = execute_search(vault.path(), "rust", false, false, false).unwrap();
    assert_eq!(non_whole[0].matches.len(), 4);

    let whole = execute_search(vault.path(), "rust", false, true, false).unwrap();
    assert_eq!(whole[0].matches.len(), 2);

    // Operator override
    let op_whole = execute_search(vault.path(), "rust word:true", false, false, false).unwrap();
    assert_eq!(op_whole[0].matches.len(), 2);
}

#[test]
fn test_regex_search_and_invalid_handling() {
    let vault = TempTestVault::new("test_regex_search");
    vault.write_file("note.md", "item-100, item-250, item-xyz");

    let results = execute_search(vault.path(), r"item-\d+", false, false, true).unwrap();
    assert_eq!(results[0].matches.len(), 2);

    // Autodetected regex mode (ui_is_regex is false)
    let autodetected = execute_search(vault.path(), r"item-\d+", false, false, false).unwrap();
    assert_eq!(autodetected[0].matches.len(), 2);

    // Invalid regex
    let err = execute_search(vault.path(), "(unclosed", false, false, true);
    assert!(err.is_err());
    let err_str = err.unwrap_err().to_string();
    assert!(err_str.contains("Invalid regular expression"));
}

#[test]
fn test_quoted_phrases_and_and_semantics() {
    let vault = TempTestVault::new("test_phrases_and");
    vault.write_file(
        "both.md",
        "First paragraph has rust.\nSecond has performance.",
    );
    vault.write_file("only_one.md", "Only has rust here.");
    vault.write_file(
        "phrase.md",
        "Exact phrase match here.\nNot exact other phrase.",
    );

    // AND rule: both terms must appear in the note
    let both = execute_search(vault.path(), "rust performance", false, false, false).unwrap();
    assert_eq!(both.len(), 1);
    assert_eq!(both[0].rel_path, "both.md");

    // Quoted phrase: exact contiguous substring
    let phrase = execute_search(vault.path(), r#""Exact phrase""#, false, false, false).unwrap();
    assert_eq!(phrase.len(), 1);
    assert_eq!(phrase[0].rel_path, "phrase.md");
}

#[test]
fn test_query_operators_path_file_line() {
    let vault = TempTestVault::new("test_operators");
    vault.write_file(
        "docs/api/intro.md",
        "Target line with match.\nAnother line.",
    );
    vault.write_file("docs/guide/intro.md", "Target line without match.");
    vault.write_file("notes/other.md", "Target line with match.");

    // Filter by path
    let path_res =
        execute_search(vault.path(), "Target path:docs/api", false, false, false).unwrap();
    assert_eq!(path_res.len(), 1);
    assert_eq!(path_res[0].rel_path, "docs/api/intro.md");

    // Filter by file
    let file_res =
        execute_search(vault.path(), "Target file:intro.md", false, false, false).unwrap();
    assert_eq!(file_res.len(), 2);

    // Filter by line
    let line_res = execute_search(vault.path(), "Target line:match", false, false, false).unwrap();
    assert_eq!(line_res.len(), 3);
    for f in &line_res {
        for m in &f.matches {
            assert!(m.snippet.contains("match"));
        }
    }
}

#[test]
fn test_tag_filter_frontmatter_and_inline() {
    let vault = TempTestVault::new("test_tags");
    vault.write_file("note1.md", "---\ntags: [research, ai]\n---\nBody text.");
    vault.write_file("note2.md", "Note with inline #research tag in body.");
    vault.write_file("note3.md", "Note with no relevant tag.");

    let res = execute_search(vault.path(), "tag:#research", false, false, false).unwrap();
    assert_eq!(res.len(), 2);
    let paths: Vec<_> = res.iter().map(|r| r.rel_path.as_str()).collect();
    assert!(paths.contains(&"note1.md"));
    assert!(paths.contains(&"note2.md"));
}

#[test]
fn test_unicode_character_offsets() {
    let vault = TempTestVault::new("test_unicode");
    // Multibyte emojis and characters
    let content = "🦀 Rust ❤️ 1234\nLine 2 with 🚀 rocket.";
    let path = vault.write_file("unicode.md", content);

    let res = execute_search(vault.path(), "rocket", false, false, false).unwrap();
    assert_eq!(res.len(), 1);
    let m = &res[0].matches[0];
    assert_eq!(m.line_number, 2);

    // Verify char offset in ropey / editor
    let mut state = AppState::new(Some(vault.path().to_path_buf()));
    commands::execute_open_file(&mut state, path);
    let tab = state.active_tab_mut().unwrap();

    tab.editor.cursor.char_idx = m.char_offset;
    tab.editor.selection.anchor = m.char_offset;
    tab.editor.selection.head = m.char_offset + m.match_length;

    // Check slice matches "rocket"
    let selected_text = tab
        .editor
        .buffer
        .rope
        .slice(m.char_offset..m.char_offset + m.match_length)
        .to_string();
    assert_eq!(selected_text, "rocket");
}

#[test]
fn test_large_file_and_bounds() {
    let vault = TempTestVault::new("test_large_file");
    // 500 lines containing match
    let mut large_content = String::new();
    for i in 1..=300 {
        large_content.push_str(&format!("Line {i} contains test_target keyword.\n"));
    }
    vault.write_file("large.md", &large_content);

    let res = execute_search(vault.path(), "test_target", false, false, false).unwrap();
    assert_eq!(res.len(), 1);
    // Capped at MAX_MATCHES_PER_FILE = 100
    assert_eq!(res[0].matches.len(), 100);

    for m in &res[0].matches {
        assert!(m.snippet.len() <= MAX_SNIPPET_LEN + 10);
    }
}

#[test]
fn test_ignored_directories_and_hidden_files() {
    let vault = TempTestVault::new("test_ignored_dirs");
    vault.write_file("normal.md", "secret search key");
    vault.write_file(".hidden.md", "secret search key");
    vault.write_file(".git/config.md", "secret search key");
    vault.write_file("target/build.md", "secret search key");
    vault.write_file("node_modules/pkg/readme.md", "secret search key");

    let res = execute_search(vault.path(), "secret", false, false, false).unwrap();
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].rel_path, "normal.md");
}

#[test]
fn test_symlink_escape_protection() {
    let vault = TempTestVault::new("test_symlink_escape");
    let outside = TempTestVault::new("test_outside");

    vault.write_file("internal.md", "find_me");
    let outside_file = outside.write_file("outside.md", "find_me");

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let symlink_path = vault.path().join("symlink_escaped.md");
        symlink(&outside_file, &symlink_path).unwrap();

        let res = execute_search(vault.path(), "find_me", false, false, false).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].rel_path, "internal.md");
    }
}

#[test]
fn test_invalid_utf8_unreadable_files_safety() {
    let vault = TempTestVault::new("test_invalid_utf8");
    vault.write_file("good.md", "safe target");

    // Write file with raw invalid UTF-8 bytes
    let bad_path = vault.path().join("corrupted.md");
    let mut bad_file = File::create(&bad_path).unwrap();
    bad_file
        .write_all(&[0xFF, 0xFE, 0xFD, b's', b'a', b'f', b'e'])
        .unwrap();

    let res = execute_search(vault.path(), "safe", false, false, false).unwrap();
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].rel_path, "good.md");
}

#[test]
fn test_cancellation_and_stale_query_suppression() {
    let vault = TempTestVault::new("test_cancel");
    for i in 1..=50 {
        vault.write_file(&format!("note_{i}.md"), "repeated query term");
    }

    let query = SearchQuery::parse("repeated", false, false, false).unwrap();
    let (tx, rx) = channel();
    let index = Arc::new(Mutex::new(SearchIndex::new()));
    let cancel_token = Arc::new(AtomicU64::new(1));

    // Cancel immediately before or during run
    cancel_token.store(2, Ordering::SeqCst);

    SearchWorker::run(
        vault.path().to_path_buf(),
        query,
        1, // generation 1 is already stale
        cancel_token,
        tx,
        index,
    );

    let mut events = Vec::new();
    while let Ok(e) = rx.try_recv() {
        events.push(e);
    }
    // No events should be sent for stale generation
    assert!(events.is_empty(), "Stale generation must not send events");
}

#[test]
fn test_empty_vault_and_empty_query() {
    let vault = TempTestVault::new("test_empty");

    // Empty vault
    let res = execute_search(vault.path(), "term", false, false, false).unwrap();
    assert!(res.is_empty());

    // Empty query
    let res_empty = execute_search(vault.path(), "", false, false, false).unwrap();
    assert!(res_empty.is_empty());
}

#[test]
fn test_regression_in_file_find_and_quick_open() {
    let vault = TempTestVault::new("test_regressions");
    let note_path = vault.write_file("main.md", "hello in-file find\nsecond line hello");

    let mut state = AppState::new(Some(vault.path().to_path_buf()));
    commands::execute_open_file(&mut state, note_path);

    // In-file find test
    let tab = state.active_tab_mut().unwrap();
    tab.editor_renderer.content_buffer = tab.editor.buffer.to_string();
    tab.editor_renderer.find_query = "hello".to_string();
    tab.editor_renderer.update_find_matches();
    assert_eq!(tab.editor_renderer.matches.len(), 2);

    // Quick open test
    state.explorer.search_query = "main".to_string();
    let mut quick_matches = Vec::new();
    state
        .explorer
        .collect_matching_files(vault.path(), vault.path(), "main", &mut quick_matches);
    assert_eq!(quick_matches.len(), 1);
    assert_eq!(quick_matches[0].1, "main.md");
}

#[test]
fn test_performance_10k_notes_benchmark() {
    let vault = TempTestVault::new("bench_10k_vault");
    println!("Generating 10,000 files for benchmark...");

    // Create 10,000 notes in nested directories
    for folder_idx in 0..10 {
        let folder = format!("folder_{folder_idx}");
        for file_idx in 0..1000 {
            let rel = format!("{folder}/note_{file_idx}.md");
            let content = if file_idx == 42 && folder_idx == 3 {
                "# Special Needle Note\nHere is unique_needle_term inside 10,000 files.\n#tag_benchmark"
            } else {
                "# Normal Note\nStandard filler text without the needle.\n#general"
            };
            vault.write_file(&rel, content);
        }
    }

    println!("Starting benchmark scan & search...");
    let start = Instant::now();

    let query = SearchQuery::parse("unique_needle_term", false, false, false).unwrap();
    let (tx, rx) = channel();
    let index = Arc::new(Mutex::new(SearchIndex::new()));
    let cancel = Arc::new(AtomicU64::new(1));

    SearchWorker::run(
        vault.path().to_path_buf(),
        query,
        1,
        cancel,
        tx,
        index.clone(),
    );

    let search_time = start.elapsed();
    println!("10k note search completed in: {:?}", search_time);

    let mut found_results = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let SearchEvent::PartialResults { results, .. } = event {
            found_results.extend(results);
        }
    }

    assert_eq!(found_results.len(), 1);
    assert_eq!(found_results[0].rel_path, "folder_3/note_42.md");
    assert_eq!(found_results[0].matches.len(), 1);

    // Confirm repeated search uses cached index metadata
    let start_repeat = Instant::now();
    let query2 = SearchQuery::parse("tag:#tag_benchmark", false, false, false).unwrap();
    let (tx2, rx2) = channel();
    let cancel2 = Arc::new(AtomicU64::new(2));

    SearchWorker::run(vault.path().to_path_buf(), query2, 2, cancel2, tx2, index);

    let repeat_time = start_repeat.elapsed();
    println!("Repeated search on cached index took: {:?}", repeat_time);

    let mut found_repeat = Vec::new();
    while let Ok(event) = rx2.try_recv() {
        if let SearchEvent::PartialResults { results, .. } = event {
            found_repeat.extend(results);
        }
    }

    assert_eq!(found_repeat.len(), 1);
    assert_eq!(found_repeat[0].rel_path, "folder_3/note_42.md");
}

#[test]
fn test_unified_filename_and_content_search() {
    let vault = TempTestVault::new("test_unified");
    // File 1: filename matches "inventory", body does not
    vault.write_file("inventory_tracker.md", "Blank body text with items.");

    // File 2: body matches "inventory", filename does not
    vault.write_file("warehouse.md", "Check the latest inventory report here.");

    // File 3: neither matches
    vault.write_file("unrelated.md", "Completely different text.");

    // Unified search for "inventory": must find BOTH files
    let results = execute_search(vault.path(), "inventory", false, false, false).unwrap();
    assert_eq!(results.len(), 2);
    let paths: Vec<_> = results.iter().map(|r| r.rel_path.as_str()).collect();
    assert!(paths.contains(&"inventory_tracker.md"));
    assert!(paths.contains(&"warehouse.md"));

    // File 1 match points to filename snippet
    let file1_res = results
        .iter()
        .find(|r| r.rel_path == "inventory_tracker.md")
        .unwrap();
    assert!(
        file1_res.matches[0]
            .snippet
            .contains("inventory_tracker.md")
    );

    // File 2 match points to body line
    let file2_res = results
        .iter()
        .find(|r| r.rel_path == "warehouse.md")
        .unwrap();
    assert!(
        file2_res.matches[0]
            .snippet
            .contains("latest inventory report")
    );
}
