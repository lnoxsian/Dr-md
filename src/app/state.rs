pub use crate::config::ViewMode;
use crate::config::{AppConfig, FileState};
use crate::editor::{Editor, renderer::EditorRenderer};
use crate::explorer::FileTree;
use crate::markdown::MarkdownPreview;
use crate::search::SearchController;
use crate::workspace::Vault;
use std::path::PathBuf;

pub struct Tab {
    pub path: PathBuf,
    pub editor: Editor,
    pub editor_renderer: EditorRenderer,
    pub view_mode: ViewMode,
}

pub struct AppState {
    pub config: AppConfig,
    pub file_state: FileState,
    pub vault: Vault,
    pub explorer: FileTree,
    pub preview: MarkdownPreview,
    pub explorer_visible: bool,
    pub focus_mode: bool,
    pub split_ratio: f32,
    pub logo_light_mode: Option<egui::TextureHandle>,
    pub logo_dark_mode: Option<egui::TextureHandle>,
    pub last_edit_time: Option<std::time::Instant>,
    pub last_editor_version: usize,
    pub tabs: Vec<Tab>,
    pub active_tab_index: Option<usize>,
    pub session_dirty: bool,
    pub last_session_change_time: Option<std::time::Instant>,
    pub insert_table_dialog_open: bool,
    pub insert_table_cols: usize,
    pub insert_table_rows: usize,
    pub insert_table_dialog_pos: Option<egui::Pos2>,
    pub tab_scroll_accum: f32,
    pub last_active_tab_index: Option<usize>,
    pub search_active: bool,
    pub search_controller: SearchController,
}

impl AppState {
    pub fn new(root_path: Option<PathBuf>) -> Self {
        let config = AppConfig::load();
        let file_state = FileState::load();
        let resolved_path =
            root_path.or_else(|| config.last_opened_folder.as_ref().map(PathBuf::from));
        let mut state = Self {
            config,
            file_state,
            vault: Vault::new(resolved_path),
            explorer: FileTree::new(),
            preview: MarkdownPreview::new(),
            explorer_visible: true,
            focus_mode: false,
            split_ratio: 0.5,
            logo_light_mode: None,
            logo_dark_mode: None,
            last_edit_time: None,
            last_editor_version: 0,
            tabs: Vec::new(),
            active_tab_index: None,
            session_dirty: false,
            last_session_change_time: None,
            insert_table_dialog_open: false,
            insert_table_cols: 3,
            insert_table_rows: 3,
            insert_table_dialog_pos: None,
            tab_scroll_accum: 0.0,
            last_active_tab_index: None,
            search_active: false,
            search_controller: SearchController::new(),
        };

        if state.config.reopen_last_files {
            let last_tabs = state.file_state.last_open_tabs.clone();
            let last_active = state.file_state.last_active_tab;
            for tab_state in last_tabs {
                let path = PathBuf::from(tab_state.path);
                if path.exists() && path.is_file() {
                    state.open_file_in_tab_inner(path);
                    if let Some(tab) = state.tabs.last_mut() {
                        tab.view_mode = tab_state.view_mode;
                    }
                }
            }
            if let Some(active_idx) = last_active
                && active_idx < state.tabs.len()
            {
                state.switch_tab_inner(active_idx);
            }
        }

        state
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.active_tab_index.and_then(|idx| self.tabs.get(idx))
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.active_tab_index.and_then(|idx| self.tabs.get_mut(idx))
    }

    pub fn editor_id(&self) -> egui::Id {
        if let Some(ref path) = self.vault.active_file {
            egui::Id::new(("editor_text_edit", path))
        } else {
            egui::Id::new("editor_text_edit")
        }
    }

    pub fn sync_session_state(&mut self) {
        if self.config.reopen_last_files {
            self.file_state.last_open_tabs = self
                .tabs
                .iter()
                .map(|tab| crate::config::TabState {
                    path: tab.path.to_string_lossy().to_string(),
                    view_mode: tab.view_mode,
                })
                .collect();
            self.file_state.last_active_tab = self.active_tab_index;
        } else {
            self.file_state.last_open_tabs.clear();
            self.file_state.last_active_tab = None;
        }
        self.session_dirty = true;
        self.last_session_change_time = Some(std::time::Instant::now());
    }

    pub fn open_file_in_tab(&mut self, path: PathBuf) {
        self.open_file_in_tab_inner(path);
        self.sync_session_state();
    }

    fn open_file_in_tab_inner(&mut self, path: PathBuf) {
        if let Some(idx) = self.tabs.iter().position(|t| t.path == path) {
            self.switch_tab_inner(idx);
            return;
        }

        let mut new_editor = Editor::new();
        if let Err(e) = new_editor.load_file(path.clone()) {
            tracing::error!("Failed to load file: {:?}", e);
            return;
        }

        self.vault.active_file = Some(path.clone());

        let tab = Tab {
            path,
            editor: new_editor,
            editor_renderer: EditorRenderer::new(),
            view_mode: ViewMode::Split,
        };
        self.tabs.push(tab);
        let new_idx = self.tabs.len() - 1;
        self.active_tab_index = Some(new_idx);

        if let Some(active_tab) = self.active_tab() {
            self.last_editor_version = active_tab.editor.version;
        } else {
            self.last_editor_version = 0;
        }
        self.preview.active_zoom = None;
        self.last_edit_time = None;
    }

    pub fn switch_tab(&mut self, new_idx: usize) {
        self.switch_tab_inner(new_idx);
        self.sync_session_state();
    }

    fn switch_tab_inner(&mut self, new_idx: usize) {
        if new_idx >= self.tabs.len() {
            return;
        }

        self.preview.active_zoom = None;
        self.active_tab_index = Some(new_idx);
        let tab_path = self.tabs[new_idx].path.clone();
        self.vault.active_file = Some(tab_path);

        if let Some(active_tab) = self.active_tab() {
            self.last_editor_version = active_tab.editor.version;
        } else {
            self.last_editor_version = 0;
        }
        self.last_edit_time = None;
    }

    pub fn close_tab(&mut self, idx: usize) {
        if idx >= self.tabs.len() {
            return;
        }

        let tab = &mut self.tabs[idx];
        if tab.editor.is_dirty && tab.editor.active_path.is_some() {
            let _ = tab.editor.save_file();
        }

        self.tabs.remove(idx);

        let old_active_idx = self.active_tab_index;
        if self.tabs.is_empty() {
            self.active_tab_index = None;
            self.vault.active_file = None;
            self.last_editor_version = 0;
            self.last_edit_time = None;
        } else {
            if let Some(active_idx) = old_active_idx {
                if active_idx == idx {
                    let new_idx = if idx >= self.tabs.len() {
                        self.tabs.len() - 1
                    } else {
                        idx
                    };
                    self.active_tab_index = None;
                    self.switch_tab_inner(new_idx);
                } else if active_idx > idx {
                    self.active_tab_index = Some(active_idx - 1);
                }
            }
        }
        self.sync_session_state();
    }

    pub fn sync_cursor_from_egui(&mut self, ctx: &egui::Context) {
        let id = self.editor_id();
        if let Some(tab) = self.active_tab_mut()
            && let Some(text_state) = egui::widgets::text_edit::TextEditState::load(ctx, id)
            && let Some(range) = text_state.cursor.char_range()
        {
            tab.editor.cursor.char_idx = range.primary.index;
            tab.editor.selection.anchor = range.secondary.index;
            tab.editor.selection.head = range.primary.index;
        }
    }

    pub fn check_autosave(&mut self, ctx: &egui::Context) {
        if self.session_dirty
            && let Some(change_time) = self.last_session_change_time
        {
            if change_time.elapsed() >= std::time::Duration::from_secs(1) {
                let _ = self.file_state.save();
                let _ = self.config.save();
                self.session_dirty = false;
                self.last_session_change_time = None;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(500));
            }
        }

        if self.config.autosave
            && let Some(idx) = self.active_tab_index
        {
            let is_dirty_and_has_path = self
                .tabs
                .get(idx)
                .map(|tab| tab.editor.is_dirty && tab.editor.active_path.is_some())
                .unwrap_or(false);

            if is_dirty_and_has_path {
                let current_version = self.tabs[idx].editor.version;
                if self.last_editor_version != current_version {
                    self.last_editor_version = current_version;
                    self.last_edit_time = Some(std::time::Instant::now());
                }

                let elapsed = self
                    .last_edit_time
                    .get_or_insert_with(std::time::Instant::now)
                    .elapsed();

                if elapsed >= std::time::Duration::from_secs(1) {
                    if let Err(e) = self.tabs[idx].editor.save_file() {
                        tracing::error!("Failed to save file: {:?}", e);
                    }
                    self.last_edit_time = None;
                } else {
                    ctx.request_repaint_after(std::time::Duration::from_millis(500));
                }
                return;
            }
        }
        self.last_edit_time = None;
        if let Some(tab) = self.active_tab() {
            self.last_editor_version = tab.editor.version;
        } else {
            self.last_editor_version = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_autosave() {
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let file_path = std::env::temp_dir().join(format!("dr_md_test_note_{}.md", unique_id));

        let mut state = AppState::new(None);
        state.tabs.clear();
        state.active_tab_index = None;
        state.config.autosave = true;

        // Setup active tab with an active path
        state.tabs.push(Tab {
            path: file_path.clone(),
            editor: {
                let mut editor = Editor::new();
                editor.active_path = Some(file_path.clone());
                editor.set_text("Initial text");
                editor
            },
            editor_renderer: EditorRenderer::new(),
            view_mode: ViewMode::Split,
        });
        state.active_tab_index = Some(0);

        // Verify set_text makes it dirty and updates version
        assert!(state.tabs[0].editor.is_dirty);
        let version1 = state.tabs[0].editor.version;
        state.last_editor_version = version1;

        // Mock egui context
        let ctx = egui::Context::default();

        // 1. If edit is recent (less than 1s ago), check_autosave should request repaint and NOT save
        state.last_edit_time = Some(std::time::Instant::now());
        state.check_autosave(&ctx);
        assert!(state.tabs[0].editor.is_dirty); // Still dirty
        assert!(!file_path.exists()); // File not written yet

        // 2. If last edit time was more than 1s ago, check_autosave should save the file and clear is_dirty
        state.last_edit_time = Some(std::time::Instant::now() - std::time::Duration::from_secs(2));
        state.check_autosave(&ctx);

        assert!(!state.tabs[0].editor.is_dirty); // No longer dirty
        assert!(file_path.exists()); // File written!
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Initial text");

        // Clean up
        let _ = std::fs::remove_file(&file_path);
    }

    #[test]
    fn test_tab_session_state() {
        let mut state = AppState::new(None);
        state.tabs.clear();
        state.active_tab_index = None;
        state.config.reopen_last_files = true;
        state.file_state.last_open_tabs.clear();
        state.file_state.last_active_tab = None;

        let path1 = std::env::temp_dir().join("dr_md_test_session_1.md");
        let path2 = std::env::temp_dir().join("dr_md_test_session_2.md");
        std::fs::write(&path1, "content1").unwrap();
        std::fs::write(&path2, "content2").unwrap();

        // 1. Open first file
        state.open_file_in_tab(path1.clone());
        assert_eq!(state.tabs.len(), 1);
        assert_eq!(state.active_tab_index, Some(0));
        assert_eq!(state.file_state.last_open_tabs.len(), 1);
        assert_eq!(
            state.file_state.last_open_tabs[0].path,
            path1.to_string_lossy().to_string()
        );
        assert_eq!(
            state.file_state.last_open_tabs[0].view_mode,
            ViewMode::Split
        );
        assert_eq!(state.file_state.last_active_tab, Some(0));

        // 2. Open second file
        state.open_file_in_tab(path2.clone());
        assert_eq!(state.tabs.len(), 2);
        assert_eq!(state.active_tab_index, Some(1));
        assert_eq!(state.file_state.last_open_tabs.len(), 2);
        assert_eq!(
            state.file_state.last_open_tabs[1].path,
            path2.to_string_lossy().to_string()
        );
        assert_eq!(state.file_state.last_active_tab, Some(1));

        // 3. Switch tab back
        state.switch_tab(0);
        assert_eq!(state.active_tab_index, Some(0));
        assert_eq!(state.file_state.last_active_tab, Some(0));

        // 4. Close tab
        state.close_tab(0);
        assert_eq!(state.tabs.len(), 1);
        assert_eq!(state.active_tab_index, Some(0));
        assert_eq!(state.file_state.last_open_tabs.len(), 1);
        assert_eq!(
            state.file_state.last_open_tabs[0].path,
            path2.to_string_lossy().to_string()
        );
        assert_eq!(state.file_state.last_active_tab, Some(0));

        // Clean up
        let _ = std::fs::remove_file(&path1);
        let _ = std::fs::remove_file(&path2);
    }
}
