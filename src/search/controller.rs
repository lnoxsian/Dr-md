use crate::search::index::SearchIndex;
use crate::search::query::SearchQuery;
use crate::search::result::{FileSearchResult, MatchLocation};
use crate::search::worker::{SearchEvent, SearchWorker};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct SearchController {
    pub query_input: String,
    pub active_query: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub is_regex: bool,

    pub last_input_time: Option<Instant>,
    pub debounce_duration: Duration,

    pub generation: Arc<AtomicU64>,
    pub cancel_token: Arc<AtomicU64>,

    pub tx: Sender<SearchEvent>,
    pub rx: Receiver<SearchEvent>,

    pub index: Arc<Mutex<SearchIndex>>,
    pub results: Vec<FileSearchResult>,
    pub total_matches: usize,
    pub total_files: usize,
    pub elapsed_ms: u128,

    pub is_searching: bool,
    pub is_indexing: bool,
    pub indexing_progress: (usize, usize),
    pub error_message: Option<String>,

    pub selected_match: Option<(usize, usize)>, // (file_index, match_index)
    pub focus_input: bool,
}

impl SearchController {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Self {
            query_input: String::new(),
            active_query: String::new(),
            case_sensitive: false,
            whole_word: false,
            is_regex: false,

            last_input_time: None,
            debounce_duration: Duration::from_millis(0),

            generation: Arc::new(AtomicU64::new(0)),
            cancel_token: Arc::new(AtomicU64::new(0)),

            tx,
            rx,

            index: Arc::new(Mutex::new(SearchIndex::new())),
            results: Vec::new(),
            total_matches: 0,
            total_files: 0,
            elapsed_ms: 0,

            is_searching: false,
            is_indexing: false,
            indexing_progress: (0, 0),
            error_message: None,

            selected_match: None,
            focus_input: false,
        }
    }

    /// Call this on every UI frame update.
    pub fn update(&mut self, ctx: &egui::Context, vault_root: Option<&Path>) {
        // Handle debounce trigger
        if let Some(input_time) = self.last_input_time
            && input_time.elapsed() >= self.debounce_duration
        {
            self.last_input_time = None;
            if let Some(root) = vault_root {
                self.trigger_search(root);
            }
        }

        let current_gen = self.generation.load(Ordering::Relaxed);

        // Drain channel events
        while let Ok(event) = self.rx.try_recv() {
            match event {
                SearchEvent::IndexingProgress { current, total } => {
                    self.is_indexing = true;
                    self.indexing_progress = (current, total);
                    ctx.request_repaint();
                }
                SearchEvent::PartialResults {
                    generation,
                    results,
                } => {
                    if generation == current_gen {
                        for res in results {
                            if let Some(existing) = self
                                .results
                                .iter_mut()
                                .find(|r| r.file_path == res.file_path)
                            {
                                existing.matches.extend(res.matches);
                            } else {
                                self.results.push(res);
                            }
                        }
                        self.total_matches = self.results.iter().map(|r| r.matches.len()).sum();
                        self.total_files = self.results.len();
                        ctx.request_repaint();
                    }
                }
                SearchEvent::Complete {
                    generation,
                    total_matches,
                    total_files,
                    elapsed_ms,
                } => {
                    if generation == current_gen {
                        self.is_searching = false;
                        self.is_indexing = false;
                        self.total_matches = total_matches;
                        self.total_files = total_files;
                        self.elapsed_ms = elapsed_ms;
                        ctx.request_repaint();
                    }
                }
                SearchEvent::Error {
                    generation,
                    message,
                } => {
                    if generation == current_gen {
                        self.is_searching = false;
                        self.is_indexing = false;
                        self.error_message = Some(message);
                        ctx.request_repaint();
                    }
                }
            }
        }

        // Request continuous repaint while searching/indexing to stream live updates
        if self.is_searching || self.is_indexing {
            ctx.request_repaint();
        }
    }

    pub fn on_input_changed(&mut self) {
        self.last_input_time = Some(Instant::now());
    }

    pub fn trigger_search(&mut self, vault_root: &Path) {
        // Cancel active searches
        let next_gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.cancel_token.store(next_gen, Ordering::SeqCst);

        self.results.clear();
        self.total_matches = 0;
        self.total_files = 0;
        self.selected_match = None;

        let trimmed = self.query_input.trim();
        if trimmed.is_empty() {
            self.active_query.clear();
            self.error_message = None;
            self.is_searching = false;
            return;
        }

        self.active_query = trimmed.to_string();

        match SearchQuery::parse(trimmed, self.case_sensitive, self.whole_word, self.is_regex) {
            Ok(query) => {
                self.error_message = None;
                self.is_searching = true;

                let vault_path = vault_root.to_path_buf();
                let cancel_token = self.cancel_token.clone();
                let tx = self.tx.clone();
                let index_lock = self.index.clone();

                std::thread::spawn(move || {
                    SearchWorker::run(vault_path, query, next_gen, cancel_token, tx, index_lock);
                });
            }
            Err(err) => {
                self.error_message = Some(err.to_string());
                self.is_searching = false;
            }
        }
    }

    pub fn select_next_match(&mut self) {
        if self.results.is_empty() {
            return;
        }

        match self.selected_match {
            None => {
                if !self.results[0].matches.is_empty() {
                    self.selected_match = Some((0, 0));
                }
            }
            Some((file_idx, match_idx)) => {
                if match_idx + 1 < self.results[file_idx].matches.len() {
                    self.selected_match = Some((file_idx, match_idx + 1));
                } else if file_idx + 1 < self.results.len() {
                    self.selected_match = Some((file_idx + 1, 0));
                }
            }
        }
    }

    pub fn select_prev_match(&mut self) {
        if self.results.is_empty() {
            return;
        }

        match self.selected_match {
            None => {}
            Some((file_idx, match_idx)) => {
                if match_idx > 0 {
                    self.selected_match = Some((file_idx, match_idx - 1));
                } else if file_idx > 0 {
                    let prev_file_idx = file_idx - 1;
                    let last_match = self.results[prev_file_idx].matches.len().saturating_sub(1);
                    self.selected_match = Some((prev_file_idx, last_match));
                }
            }
        }
    }

    pub fn get_selected_match(&self) -> Option<(PathBuf, MatchLocation)> {
        let (file_idx, match_idx) = self.selected_match?;
        let file = self.results.get(file_idx)?;
        let loc = file.matches.get(match_idx)?;
        Some((file.file_path.clone(), loc.clone()))
    }
}

impl Default for SearchController {
    fn default() -> Self {
        Self::new()
    }
}
