use std::ops::Range;
use std::path::PathBuf;

/// Exact location of a match within a file, with context snippet for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchLocation {
    /// 1-based line number
    pub line_number: usize,
    /// 1-based character column offset in line
    pub col_number: usize,
    /// Absolute character offset in the file (Unicode scalar values)
    pub char_offset: usize,
    /// Length of the match in characters
    pub match_length: usize,
    /// Context snippet text around the match
    pub snippet: String,
    /// Character range inside `snippet` that should be highlighted
    pub snippet_match_range: Range<usize>,
}

/// Search results grouped by file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSearchResult {
    /// Absolute path to the file
    pub file_path: PathBuf,
    /// Relative path for display
    pub rel_path: String,
    /// List of matches found within this file
    pub matches: Vec<MatchLocation>,
    /// UI state: whether this file group is collapsed in the results tree
    pub is_collapsed: bool,
}

impl FileSearchResult {
    pub fn new(file_path: PathBuf, rel_path: String) -> Self {
        Self {
            file_path,
            rel_path,
            matches: Vec::new(),
            is_collapsed: false,
        }
    }
}
