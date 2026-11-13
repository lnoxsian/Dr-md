pub mod commands;
pub mod editor_window;
pub mod explorer;
pub mod menu_bar;
pub mod state;
pub mod tab_bar;

use crate::keymap::{ShortcutAction, handle_key_events};
use state::{AppState, ViewMode};
use std::path::PathBuf;

pub struct DoctorMarkdownApp {
    state: AppState,
}

impl DoctorMarkdownApp {
    pub fn new(cc: &eframe::CreationContext<'_>, root_path: Option<PathBuf>) -> Self {
        let state = AppState::new(root_path);
        crate::config::setup_fonts(&cc.egui_ctx);
        crate::config::apply_theme(&cc.egui_ctx, &state.config);

        Self { state }
    }
}
impl eframe::App for DoctorMarkdownApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.state
            .search_controller
            .update(ctx, self.state.vault.root_path.as_deref());

        if let Some(action) = handle_key_events(ctx) {
            self.state.sync_cursor_from_egui(ctx);
            match action {
                ShortcutAction::NewNote => {
                    if let Some(ref root) = self.state.vault.root_path {
                        self.state.explorer_visible = true;
                        let target_dir = self
                            .state
                            .explorer
                            .selected_folder
                            .clone()
                            .unwrap_or_else(|| root.clone());
                        self.state.explorer.start_creation(
                            crate::explorer::tree::CreatingType::File {
                                parent_dir: target_dir,
                            },
                        );
                    }
                }
                ShortcutAction::NewFolder => {
                    if let Some(ref root) = self.state.vault.root_path {
                        self.state.explorer_visible = true;
                        let target_dir = self
                            .state
                            .explorer
                            .selected_folder
                            .clone()
                            .unwrap_or_else(|| root.clone());
                        self.state.explorer.start_creation(
                            crate::explorer::tree::CreatingType::Folder {
                                parent_dir: target_dir,
                            },
                        );
                    }
                }
                ShortcutAction::OpenFolder => {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        commands::execute_open_folder(&mut self.state, path);
                    }
                }
                ShortcutAction::Save => {
                    commands::execute_save(&mut self.state);
                }
                ShortcutAction::SaveAs => {
                    if let Some(path) = rfd::FileDialog::new().save_file() {
                        commands::execute_save_as(&mut self.state, path);
                    }
                }
                ShortcutAction::CloseNote => {
                    if let Some(idx) = self.state.active_tab_index {
                        self.state.close_tab(idx);
                    }
                }
                ShortcutAction::Undo => {
                    if let Some(tab) = self.state.active_tab_mut() {
                        tab.editor.undo();
                    }
                }
                ShortcutAction::Redo => {
                    if let Some(tab) = self.state.active_tab_mut() {
                        tab.editor.redo();
                    }
                }
                ShortcutAction::SelectAll => {
                    let editor_id = self.state.editor_id();
                    if let Some(tab) = self.state.active_tab()
                        && let Some(mut text_state) =
                            egui::widgets::text_edit::TextEditState::load(ctx, editor_id)
                    {
                        let len = tab.editor.buffer.len_chars();
                        let anchor = egui::text::CCursor::new(0);
                        let head = egui::text::CCursor::new(len);
                        text_state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::two(anchor, head)));
                        text_state.store(ctx, editor_id);
                    }
                }
                ShortcutAction::ViewSplit => {
                    if let Some(tab) = self.state.active_tab_mut() {
                        tab.view_mode = ViewMode::Split;
                        self.state.sync_session_state();
                    }
                }
                ShortcutAction::ViewPreview => {
                    if let Some(tab) = self.state.active_tab_mut() {
                        tab.view_mode = ViewMode::Preview;
                        self.state.sync_session_state();
                    }
                }
                ShortcutAction::ToggleExplorer => {
                    self.state.explorer_visible = !self.state.explorer_visible;
                }
                ShortcutAction::ToggleFocusMode => {
                    self.state.focus_mode = !self.state.focus_mode;
                    self.state.explorer_visible = !self.state.focus_mode;
                }
                ShortcutAction::Bold => {
                    if let Some(tab) = self.state.active_tab_mut() {
                        tab.editor.format_selection("bold");
                    }
                }
                ShortcutAction::Italic => {
                    if let Some(tab) = self.state.active_tab_mut() {
                        tab.editor.format_selection("italic");
                    }
                }
                ShortcutAction::Link => {
                    if let Some(tab) = self.state.active_tab_mut() {
                        tab.editor.format_selection("link");
                    }
                }
                ShortcutAction::CodeBlock => {
                    if let Some(tab) = self.state.active_tab_mut() {
                        tab.editor.format_selection("code");
                    }
                }
                ShortcutAction::Checkbox => {
                    if let Some(tab) = self.state.active_tab_mut() {
                        tab.editor.format_selection("checkbox");
                    }
                }
                ShortcutAction::Comment => {
                    if let Some(tab) = self.state.active_tab_mut() {
                        tab.editor.format_selection("comment");
                    }
                }
                ShortcutAction::Table => {
                    let pos = self
                        .state
                        .active_tab()
                        .and_then(|t| t.editor.cursor_screen_pos);
                    self.state.insert_table_dialog_open = true;
                    self.state.insert_table_dialog_pos =
                        pos.or_else(|| ctx.input(|i| i.pointer.latest_pos()));
                }
                ShortcutAction::NextTab => {
                    if !self.state.tabs.is_empty()
                        && let Some(idx) = self.state.active_tab_index
                    {
                        let next_idx = (idx + 1) % self.state.tabs.len();
                        self.state.switch_tab(next_idx);
                        ctx.memory_mut(|mem| {
                            if let Some(id) = mem.focused() {
                                mem.surrender_focus(id);
                            }
                        });
                    }
                }
                ShortcutAction::PrevTab => {
                    if !self.state.tabs.is_empty()
                        && let Some(idx) = self.state.active_tab_index
                    {
                        let prev_idx = if idx == 0 {
                            self.state.tabs.len() - 1
                        } else {
                            idx - 1
                        };
                        self.state.switch_tab(prev_idx);
                        ctx.memory_mut(|mem| {
                            if let Some(id) = mem.focused() {
                                mem.surrender_focus(id);
                            }
                        });
                    }
                }
                ShortcutAction::SearchInFile => {
                    if let Some(tab) = self.state.active_tab_mut() {
                        tab.editor_renderer.find_visible = true;
                        tab.editor_renderer.focus_search_input = true;
                        tab.editor_renderer.update_find_matches();
                    }
                }
                ShortcutAction::QuickOpen | ShortcutAction::SearchInVault => {
                    self.state.explorer_visible = true;
                    self.state.search_active = !self.state.search_active;
                    if self.state.search_active {
                        self.state.search_controller.focus_input = true;
                    }
                }
                ShortcutAction::GoToLine => {
                    if let Some(tab) = self.state.active_tab_mut() {
                        tab.editor_renderer.find_visible = true;
                        tab.editor_renderer.focus_search_input = true;
                        tab.editor_renderer.update_find_matches();
                    }
                }
                ShortcutAction::Back => {
                    if !self.state.tabs.is_empty()
                        && let Some(idx) = self.state.active_tab_index
                    {
                        let prev_idx = if idx == 0 {
                            self.state.tabs.len() - 1
                        } else {
                            idx - 1
                        };
                        self.state.switch_tab(prev_idx);
                        ctx.memory_mut(|mem| {
                            if let Some(id) = mem.focused() {
                                mem.surrender_focus(id);
                            }
                        });
                    }
                }
                ShortcutAction::Forward => {
                    if !self.state.tabs.is_empty()
                        && let Some(idx) = self.state.active_tab_index
                    {
                        let next_idx = (idx + 1) % self.state.tabs.len();
                        self.state.switch_tab(next_idx);
                        ctx.memory_mut(|mem| {
                            if let Some(id) = mem.focused() {
                                mem.surrender_focus(id);
                            }
                        });
                    }
                }
            }
        }

        let mut request_dialog = false;
        let mut cursor_pos = None;
        if let Some(tab) = self.state.active_tab_mut()
            && tab.editor.request_table_dialog
        {
            tab.editor.request_table_dialog = false;
            request_dialog = true;
            cursor_pos = tab.editor.cursor_screen_pos;
        }
        if request_dialog {
            self.state.insert_table_dialog_open = true;
            self.state.insert_table_dialog_pos =
                cursor_pos.or_else(|| ctx.input(|i| i.pointer.latest_pos()));
        }

        menu_bar::render_menu_bar(ctx, &mut self.state);
        explorer::render_explorer(ctx, &mut self.state);
        editor_window::render_editor_window(ctx, &mut self.state);

        if self.state.insert_table_dialog_open {
            let mut close_dialog = false;
            let mut insert = false;

            let mut window = egui::Window::new("Insert Table")
                .title_bar(false)
                .collapsible(false)
                .resizable(false);

            if let Some(pos) = self.state.insert_table_dialog_pos {
                window = window.current_pos(pos + egui::vec2(10.0, 10.0));
            } else {
                window = window.anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0));
            }

            let window_response = window.show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Table");
                    ui.add(
                        egui::DragValue::new(&mut self.state.insert_table_rows)
                            .clamp_range(1..=100),
                    );
                    ui.label("x");
                    ui.add(
                        egui::DragValue::new(&mut self.state.insert_table_cols).clamp_range(1..=20),
                    );
                });
            });

            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                insert = true;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                close_dialog = true;
            }

            if let Some(response) = window_response {
                let rect = response.response.rect;
                if ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary))
                    && let Some(click_pos) = ctx.input(|i| i.pointer.interact_pos())
                    && !rect.contains(click_pos)
                {
                    close_dialog = true;
                }
            }

            if close_dialog {
                self.state.insert_table_dialog_open = false;
            }

            if insert {
                self.state.insert_table_dialog_open = false;
                self.state.sync_cursor_from_egui(ctx);

                let cols = self.state.insert_table_cols;
                let rows = self.state.insert_table_rows;
                let editor_id = self.state.editor_id();

                if let Some(tab) = self.state.active_tab_mut() {
                    let selected_text = if let Some(range) = tab.editor.selection.range() {
                        tab.editor.buffer.rope.slice(range).to_string()
                    } else {
                        String::new()
                    };
                    tab.editor.format_table(cols, rows, &selected_text);
                    tab.editor_renderer.content_buffer = tab.editor.buffer.to_string();

                    if let Some(mut text_state) =
                        egui::widgets::text_edit::TextEditState::load(ctx, editor_id)
                    {
                        let cursor = egui::text::CCursor::new(tab.editor.cursor.char_idx);
                        text_state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::two(cursor, cursor)));
                        text_state.store(ctx, editor_id);
                    }
                }
            }
        }

        if let Some(open_url) = ctx.output_mut(|o| o.open_url.take()) {
            let url_str = open_url.url.as_str();

            let (path_part, anchor_part) = if let Some(idx) = url_str.find('#') {
                (&url_str[..idx], Some(&url_str[idx + 1..]))
            } else {
                (url_str, None)
            };

            if path_part.is_empty() {
                // Current file anchor link
                if let Some(anchor) = anchor_part
                    && let Some(tab) = self.state.active_tab_mut()
                {
                    let doc_text = tab.editor.buffer.to_string();
                    let line_idx = if anchor.starts_with('^') {
                        crate::markdown::wikilink::find_block_line(&doc_text, anchor)
                    } else {
                        crate::markdown::wikilink::find_heading_line(&doc_text, anchor)
                    };

                    if let Some(line_idx) = line_idx {
                        let char_idx = tab.editor.buffer.rope.line_to_char(line_idx);
                        tab.editor.cursor.char_idx = char_idx;
                        tab.editor.selection.anchor = char_idx;
                        tab.editor.selection.head = char_idx;

                        let total_lines = tab.editor.buffer.rope.len_lines();
                        let ratio = if total_lines > 1 {
                            line_idx as f32 / (total_lines - 1) as f32
                        } else {
                            0.0
                        };
                        let max_scroll = (self.state.preview.last_content_height
                            - self.state.preview.last_viewport_height)
                            .max(0.0);
                        let target_y = ratio * self.state.preview.last_content_height;
                        let target_offset = (target_y
                            - self.state.preview.last_viewport_height / 2.0)
                            .clamp(0.0, max_scroll);
                        self.state.preview.scroll_target_y = Some(target_offset);
                    }
                }
            } else if !path_part.starts_with("http://")
                && !path_part.starts_with("https://")
                && !path_part.starts_with("mailto:")
            {
                let resolved = crate::markdown::wikilink::resolve_wikilink_target(
                    path_part,
                    self.state.vault.active_file.as_deref(),
                    self.state.vault.root_path.as_deref(),
                );

                match resolved {
                    crate::markdown::wikilink::ResolvedLinkTarget::CurrentDocument => {}
                    crate::markdown::wikilink::ResolvedLinkTarget::Directory { path, .. } => {
                        commands::execute_open_folder(&mut self.state, path);
                    }
                    crate::markdown::wikilink::ResolvedLinkTarget::MarkdownFile {
                        path,
                        exists,
                    } => {
                        if exists {
                            commands::execute_open_file(&mut self.state, path.clone());
                        } else {
                            // Note does not exist: create note in vault and open!
                            if let Some(parent) = path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            if std::fs::write(&path, "").is_ok() {
                                commands::execute_open_file(&mut self.state, path.clone());
                            } else {
                                tracing::warn!("Failed to create unresolved note: {:?}", path);
                            }
                        }

                        // Jump to anchor in newly opened note if present
                        if let Some(anchor) = anchor_part
                            && let Some(tab) = self.state.active_tab_mut()
                        {
                            let doc_text = tab.editor.buffer.to_string();
                            let line_idx = if anchor.starts_with('^') {
                                crate::markdown::wikilink::find_block_line(&doc_text, anchor)
                            } else {
                                crate::markdown::wikilink::find_heading_line(&doc_text, anchor)
                            };

                            if let Some(line_idx) = line_idx {
                                let char_idx = tab.editor.buffer.rope.line_to_char(line_idx);
                                tab.editor.cursor.char_idx = char_idx;
                                tab.editor.selection.anchor = char_idx;
                                tab.editor.selection.head = char_idx;

                                let total_lines = tab.editor.buffer.rope.len_lines();
                                let ratio = if total_lines > 1 {
                                    line_idx as f32 / (total_lines - 1) as f32
                                } else {
                                    0.0
                                };
                                let max_scroll = (self.state.preview.last_content_height
                                    - self.state.preview.last_viewport_height)
                                    .max(0.0);
                                let target_y = ratio * self.state.preview.last_content_height;
                                let target_offset = (target_y
                                    - self.state.preview.last_viewport_height / 2.0)
                                    .clamp(0.0, max_scroll);
                                self.state.preview.scroll_target_y = Some(target_offset);
                            }
                        }
                    }
                    crate::markdown::wikilink::ResolvedLinkTarget::MediaOrFile {
                        path,
                        exists,
                        ..
                    } => {
                        if exists {
                            let file_url = format!("file://{}", path.to_string_lossy());
                            ctx.output_mut(|o| {
                                o.open_url = Some(egui::output::OpenUrl {
                                    url: file_url,
                                    new_tab: false,
                                })
                            });
                        } else {
                            tracing::warn!("Linked file does not exist: {:?}", path);
                        }
                    }
                }
            } else {
                ctx.output_mut(|o| o.open_url = Some(open_url));
            }
        }

        self.state.check_autosave(ctx);
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        if self.state.session_dirty {
            let _ = self.state.file_state.save();
            let _ = self.state.config.save();
            self.state.session_dirty = false;
        }
    }
}

#[allow(dead_code)]
pub fn resolve_link_path(
    url_str: &str,
    active_file: Option<&std::path::Path>,
    root_path: Option<&std::path::Path>,
) -> PathBuf {
    let decoded_url_str =
        urlencoding::decode(url_str).unwrap_or(std::borrow::Cow::Borrowed(url_str));

    let mut target_path = if decoded_url_str.starts_with("file://") {
        if let Ok(parsed_url) = url::Url::parse(&decoded_url_str) {
            parsed_url.to_file_path().ok()
        } else {
            None
        }
        .unwrap_or_else(|| {
            let path_part = decoded_url_str.strip_prefix("file://").unwrap();
            let path_part = if let Some(stripped_localhost) = path_part.strip_prefix("localhost") {
                stripped_localhost
            } else {
                path_part
            };
            PathBuf::from(path_part)
        })
    } else {
        PathBuf::from(decoded_url_str.as_ref())
    };

    // If target_path is relative, resolve it against the active file's parent directory,
    // fallback to the vault root, or the current working directory.
    if target_path.is_relative() {
        let base_dir = active_file
            .and_then(|active| active.parent().map(|p| p.to_path_buf()))
            .or_else(|| root_path.map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        target_path = base_dir.join(target_path);
    }

    clean_path(&target_path)
}

#[allow(dead_code)]
pub fn clean_path(path: &std::path::Path) -> std::path::PathBuf {
    use std::path::Component;
    let mut clean = std::path::PathBuf::new();

    for comp in path.components() {
        match comp {
            Component::ParentDir => {
                match clean.components().next_back() {
                    Some(Component::Normal(_)) => {
                        clean.pop();
                    }
                    Some(Component::ParentDir) | None => {
                        clean.push(Component::ParentDir);
                    }
                    _ => {} // Do nothing for RootDir or Prefix
                }
            }
            Component::CurDir => {}
            _ => {
                clean.push(comp);
            }
        }
    }
    clean
}

#[allow(dead_code)]
pub fn slugify(text: &str) -> String {
    let mut slug = String::new();
    let mut last_was_hyphen = false;
    for c in text.chars() {
        if c.is_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
            last_was_hyphen = false;
        } else if (c.is_whitespace() || c == '-' || c == '_') && !last_was_hyphen {
            slug.push('-');
            last_was_hyphen = true;
        }
    }
    let mut trimmed = slug.as_str();
    while trimmed.starts_with('-') || trimmed.starts_with('_') {
        trimmed = &trimmed[1..];
    }
    while trimmed.ends_with('-') || trimmed.ends_with('_') {
        trimmed = &trimmed[..trimmed.len() - 1];
    }
    trimmed.to_string()
}

#[allow(dead_code)]
pub fn find_heading_line(text: &str, anchor: &str) -> Option<usize> {
    crate::markdown::wikilink::find_heading_line(text, anchor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Section Name"), "section-name");
        assert_eq!(
            slugify("## My Cool Heading! (Extra)"),
            "my-cool-heading-extra"
        );
        assert_eq!(slugify("**Bold** Heading"), "bold-heading");
    }

    #[test]
    fn test_find_heading_line() {
        let doc = "# Title\n\n## Section One\nSome text.\n\n### Sub-section\nMore text.";
        assert_eq!(find_heading_line(doc, "title"), Some(0));
        assert_eq!(find_heading_line(doc, "section-one"), Some(2));
        assert_eq!(find_heading_line(doc, "sub-section"), Some(5));
        assert_eq!(find_heading_line(doc, "non-existent"), None);
    }

    #[test]
    fn test_resolve_link_path_relative() {
        let active_file = Path::new("/home/user/vault/sub/note.md");
        let root_path = Path::new("/home/user/vault");

        // Relative path
        let res = resolve_link_path("./README_2.md", Some(active_file), Some(root_path));
        assert_eq!(res, Path::new("/home/user/vault/sub/README_2.md"));

        // Relative path with parent traversal
        let res = resolve_link_path("../README_2.md", Some(active_file), Some(root_path));
        assert_eq!(res, Path::new("/home/user/vault/README_2.md"));
    }

    #[test]
    fn test_resolve_link_path_absolute() {
        let active_file = Path::new("/home/user/vault/sub/note.md");
        let root_path = Path::new("/home/user/vault");

        // Absolute path
        let res = resolve_link_path(
            "/media/user/README_2.md",
            Some(active_file),
            Some(root_path),
        );
        assert_eq!(res, Path::new("/media/user/README_2.md"));
    }

    #[test]
    fn test_resolve_link_path_file_scheme() {
        let active_file = Path::new("/home/user/vault/sub/note.md");
        let root_path = Path::new("/home/user/vault");

        // file:// absolute path
        let res = resolve_link_path(
            "file:///media/user/README_2.md",
            Some(active_file),
            Some(root_path),
        );
        assert_eq!(res, Path::new("/media/user/README_2.md"));

        // file:// with localhost
        let res = resolve_link_path(
            "file://localhost/media/user/README_2.md",
            Some(active_file),
            Some(root_path),
        );
        assert_eq!(res, Path::new("/media/user/README_2.md"));
    }
}
