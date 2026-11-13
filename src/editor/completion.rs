use crate::editor::EditBuffer;
use crate::markdown::wikilink::detect_media_type;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Note,
    Heading,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub detail: Option<String>,
    pub insert_text: String,
    pub kind: CompletionKind,
}

#[derive(Debug, Clone)]
pub struct WikilinkCompletion {
    pub is_active: bool,
    /// Document char index of the first `[` in `[[`
    pub trigger_char_pos: usize,
    /// Query string between `[[` and cursor
    pub query: String,
    /// Filtered completion suggestions
    pub items: Vec<CompletionItem>,
    /// Selected index in `items`
    pub selected_index: usize,
    /// Suppressed trigger position (set when user presses Escape)
    pub dismissed_trigger_pos: Option<usize>,
}

impl Default for WikilinkCompletion {
    fn default() -> Self {
        Self::new()
    }
}

impl WikilinkCompletion {
    pub fn new() -> Self {
        Self {
            is_active: false,
            trigger_char_pos: 0,
            query: String::new(),
            items: Vec::new(),
            selected_index: 0,
            dismissed_trigger_pos: None,
        }
    }

    pub fn dismiss(&mut self) {
        self.is_active = false;
        self.dismissed_trigger_pos = Some(self.trigger_char_pos);
        self.items.clear();
        self.selected_index = 0;
    }

    /// Checks if the cursor is currently inside an unclosed `[[...` trigger on the current line.
    pub fn check_trigger(buffer: &EditBuffer, cursor_char_idx: usize) -> Option<(usize, String)> {
        let rope = &buffer.rope;
        if rope.len_chars() == 0 || cursor_char_idx > rope.len_chars() {
            return None;
        }

        let line_idx = rope.char_to_line(cursor_char_idx);
        let line_start_char = rope.line_to_char(line_idx);
        let line_str = rope.line(line_idx).to_string();

        let col = cursor_char_idx.saturating_sub(line_start_char);
        let col = col.min(line_str.chars().count());

        // Extract slice before cursor on the current line
        let chars: Vec<char> = line_str.chars().collect();
        let before_cursor: String = chars[..col].iter().collect();

        // Find last "[[" before cursor
        if let Some(open_rel) = before_cursor.rfind("[[") {
            let after_open = &before_cursor[open_rel + 2..];
            // If there is a closing "]]" or pipe "|" between "[[" and cursor, don't trigger completion
            if after_open.contains("]]") || after_open.contains('|') {
                return None;
            }

            let trigger_pos = line_start_char + open_rel;
            let query = after_open.to_string();
            return Some((trigger_pos, query));
        }

        None
    }

    /// Updates completion state based on the current buffer and cursor.
    pub fn update(
        &mut self,
        buffer: &EditBuffer,
        cursor_char_idx: usize,
        vault_root: Option<&Path>,
        active_file: Option<&Path>,
    ) {
        if let Some((trigger_pos, query)) = Self::check_trigger(buffer, cursor_char_idx) {
            // If this trigger position was dismissed by Escape, stay dismissed unless query or pos changed
            if self.dismissed_trigger_pos == Some(trigger_pos) && self.query == query {
                self.is_active = false;
                return;
            }

            self.trigger_char_pos = trigger_pos;
            self.query = query.clone();
            self.dismissed_trigger_pos = None;
            self.is_active = true;

            // Generate items
            self.items = generate_completion_items(&query, vault_root, active_file, buffer);
            if self.selected_index >= self.items.len() {
                self.selected_index = 0;
            }
        } else {
            self.is_active = false;
            self.items.clear();
            self.selected_index = 0;
            self.dismissed_trigger_pos = None;
        }
    }

    pub fn select_next(&mut self) {
        if !self.items.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.items.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.items.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.items.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    /// Inserts the selected completion item into the editor buffer.
    /// Returns the new cursor character index.
    pub fn insert_selected(
        &mut self,
        buffer: &mut EditBuffer,
        cursor_char_idx: usize,
    ) -> Option<usize> {
        if !self.is_active || self.items.is_empty() {
            return None;
        }

        let item = &self.items[self.selected_index];
        let insert_text = &item.insert_text;

        let _query_len = self.query.chars().count();
        let replace_start = self.trigger_char_pos + 2;
        let replace_end = cursor_char_idx;

        if replace_start > replace_end || replace_end > buffer.rope.len_chars() {
            return None;
        }

        // Check if there are already closing brackets "]]" immediately after the cursor
        let after_slice: String = buffer.rope.chars().skip(cursor_char_idx).take(2).collect();
        let already_closed = after_slice.starts_with("]]");

        let text_to_insert = if already_closed {
            insert_text.clone()
        } else {
            format!("{insert_text}]]")
        };

        // Remove the typed query
        if replace_end > replace_start {
            buffer.remove(replace_start, replace_end);
        }

        // Insert completed text
        buffer.insert(replace_start, &text_to_insert);

        let new_cursor = if already_closed {
            // Position after the existing "]]"
            replace_start + insert_text.chars().count() + 2
        } else {
            // Position after the inserted "]]"
            replace_start + text_to_insert.chars().count()
        };

        self.is_active = false;
        self.items.clear();
        self.selected_index = 0;
        self.dismissed_trigger_pos = None;

        Some(new_cursor)
    }
}

/// Generates completion candidates based on the query.
pub fn generate_completion_items(
    query: &str,
    vault_root: Option<&Path>,
    active_file: Option<&Path>,
    buffer: &EditBuffer,
) -> Vec<CompletionItem> {
    let mut items = Vec::new();

    if let Some(hash_pos) = query.find('#') {
        // Heading completion
        let target_note = query[..hash_pos].trim();
        let heading_query = query[hash_pos + 1..].trim();

        let doc_content = if target_note.is_empty() {
            Some(buffer.to_string())
        } else if let Some(vr) = vault_root {
            let note_path = vr.join(format!("{}.md", target_note));
            if note_path.exists() {
                std::fs::read_to_string(&note_path).ok()
            } else {
                std::fs::read_to_string(vr.join(target_note)).ok()
            }
        } else {
            None
        };

        if let Some(content) = doc_content {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('#') {
                    let level = trimmed.chars().take_while(|&c| c == '#').count();
                    if level <= 6 {
                        let heading_text = trimmed[level..].trim();
                        if heading_query.is_empty()
                            || heading_text
                                .to_lowercase()
                                .contains(&heading_query.to_lowercase())
                        {
                            let insert = if target_note.is_empty() {
                                format!("#{}", heading_text)
                            } else {
                                format!("{}#{}", target_note, heading_text)
                            };

                            items.push(CompletionItem {
                                label: format!("{} {}", "#".repeat(level), heading_text),
                                detail: Some(if target_note.is_empty() {
                                    "Heading in current document".to_string()
                                } else {
                                    format!("Heading in {}", target_note)
                                }),
                                insert_text: insert,
                                kind: CompletionKind::Heading,
                            });
                        }
                    }
                }
            }
        }
    } else if let Some(block_pos) = query.find('^') {
        // Block completion
        let target_note = query[..block_pos].trim();
        let block_query = query[block_pos + 1..].trim();

        let doc_content = if target_note.is_empty() {
            Some(buffer.to_string())
        } else if let Some(vr) = vault_root {
            let note_path = vr.join(format!("{}.md", target_note));
            if note_path.exists() {
                std::fs::read_to_string(&note_path).ok()
            } else {
                std::fs::read_to_string(vr.join(target_note)).ok()
            }
        } else {
            None
        };

        if let Some(content) = doc_content {
            for line in content.lines() {
                let trimmed = line.trim_end();
                if let Some(pos) = trimmed.rfind('^') {
                    let block_id = &trimmed[pos + 1..];
                    if (pos == 0 || trimmed.as_bytes()[pos - 1].is_ascii_whitespace())
                        && (block_query.is_empty()
                            || block_id
                                .to_lowercase()
                                .contains(&block_query.to_lowercase()))
                    {
                        let snippet = trimmed[..pos].trim();
                        let insert = if target_note.is_empty() {
                            format!("^{}", block_id)
                        } else {
                            format!("{}^{}", target_note, block_id)
                        };

                        items.push(CompletionItem {
                            label: format!("^{}", block_id),
                            detail: Some(snippet.to_string()),
                            insert_text: insert,
                            kind: CompletionKind::Block,
                        });
                    }
                }
            }
        }
    } else {
        // Note completion: scan files in vault
        let query_lower = query.to_lowercase();

        if let Some(root) = vault_root {
            let mut file_matches = Vec::new();
            collect_vault_files(root, root, &mut file_matches);

            for (abs_path, name, rel_dir) in file_matches {
                let stem = abs_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&name);

                let is_current = active_file == Some(&abs_path);
                if is_current {
                    continue;
                }

                let matches_name = stem.to_lowercase().contains(&query_lower);
                let matches_dir = rel_dir.to_lowercase().contains(&query_lower);

                if query_lower.is_empty() || matches_name || matches_dir {
                    let insert = stem.to_string();
                    let detail = if rel_dir.is_empty() {
                        None
                    } else {
                        Some(rel_dir)
                    };

                    items.push(CompletionItem {
                        label: stem.to_string(),
                        detail,
                        insert_text: insert,
                        kind: CompletionKind::Note,
                    });
                }
            }
        }

        // Add heading in current note entry if query is empty or starts with #
        if query.is_empty() {
            items.push(CompletionItem {
                label: "# Headings in current note...".to_string(),
                detail: Some("Search headings in active note".to_string()),
                insert_text: "#".to_string(),
                kind: CompletionKind::Heading,
            });
        }
    }

    // Limit to top 20 suggestions
    items.truncate(20);
    items
}

/// Renders the floating wikilink completion popup near the editor cursor.
pub fn render_completion_popup(
    ui: &mut egui::Ui,
    editor: &mut crate::editor::Editor,
    editor_renderer: &mut crate::editor::renderer::EditorRenderer,
) {
    if !editor.completion.is_active || editor.completion.items.is_empty() {
        return;
    }

    let cursor_pos = match editor.cursor_screen_pos {
        Some(pos) => pos + egui::vec2(0.0, 4.0),
        None => return,
    };

    let mut selected_to_insert = None;
    let mut close_popup = false;

    let area_id = egui::Id::new("wikilink_completion_popup");
    let response = egui::Area::new(area_id)
        .order(egui::Order::Foreground)
        .fixed_pos(cursor_pos)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style())
                .inner_margin(egui::Margin::symmetric(6.0, 6.0))
                .rounding(egui::Rounding::same(6.0))
                .show(ui, |ui| {
                    ui.set_max_width(280.0);
                    ui.set_min_width(220.0);

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Link to:")
                                .small()
                                .color(ui.visuals().weak_text_color()),
                        );
                        ui.label(
                            egui::RichText::new(&editor.completion.query)
                                .small()
                                .monospace(),
                        );
                    });
                    ui.separator();

                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            for (idx, item) in editor.completion.items.iter().enumerate() {
                                let is_selected = idx == editor.completion.selected_index;
                                let bg_color = if is_selected {
                                    ui.visuals().selection.bg_fill
                                } else {
                                    egui::Color32::TRANSPARENT
                                };

                                let icon = match item.kind {
                                    CompletionKind::Note => "📄",
                                    CompletionKind::Heading => "🔖",
                                    CompletionKind::Block => "🧱",
                                };

                                let frame = egui::Frame::none()
                                    .fill(bg_color)
                                    .rounding(egui::Rounding::same(3.0))
                                    .inner_margin(egui::Margin::symmetric(4.0, 3.0));

                                let resp = frame.show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(icon);
                                        let text_color = if is_selected {
                                            ui.visuals().selection.stroke.color
                                        } else {
                                            ui.visuals().text_color()
                                        };
                                        ui.label(
                                            egui::RichText::new(&item.label).color(text_color),
                                        );
                                        if let Some(ref detail) = item.detail {
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    ui.label(
                                                        egui::RichText::new(detail).small().weak(),
                                                    );
                                                },
                                            );
                                        }
                                    });
                                });

                                let item_resp = resp.response.interact(egui::Sense::click());
                                if item_resp.clicked() {
                                    selected_to_insert = Some(idx);
                                }
                                if item_resp.hovered() && !is_selected {
                                    editor.completion.selected_index = idx;
                                }
                            }
                        });
                });
        });

    if let Some(idx) = selected_to_insert {
        editor.completion.selected_index = idx;
        if let Some(new_cursor) = editor
            .completion
            .insert_selected(&mut editor.buffer, editor.cursor.char_idx)
        {
            editor_renderer.content_buffer = editor.buffer.to_string();
            editor_renderer.previous_text = editor_renderer.content_buffer.clone();
            editor.cursor.char_idx = new_cursor;
            editor.selection.clear(new_cursor);
            editor.is_dirty = true;
            editor.version += 1;
            editor_renderer.last_version = editor.version;

            let editor_id = crate::editor::renderer::get_editor_id(editor);
            if let Some(mut text_state) =
                egui::widgets::text_edit::TextEditState::load(ui.ctx(), editor_id)
            {
                let cursor = egui::text::CCursor::new(new_cursor);
                text_state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(cursor, cursor)));
                text_state.store(ui.ctx(), editor_id);
            }
        }
    }

    // Dismiss on click outside
    if ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary))
        && let Some(click_pos) = ui.input(|i| i.pointer.interact_pos())
        && !response.response.rect.contains(click_pos)
    {
        close_popup = true;
    }

    if close_popup {
        editor.completion.dismiss();
    }
}

fn collect_vault_files(dir: &Path, root: &Path, matches: &mut Vec<(PathBuf, String, String)>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                collect_vault_files(&path, root, matches);
            } else if path.is_file() {
                let is_md = path
                    .extension()
                    .map(|e| e == "md" || e == "markdown")
                    .unwrap_or(false);
                let is_media = detect_media_type(&path).is_some();
                if is_md || is_media {
                    let rel_path = path.strip_prefix(root).unwrap_or(&path);
                    let parent_dir = rel_path
                        .parent()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    matches.push((path, name, parent_dir));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_trigger_detection() {
        let buf = EditBuffer::from_str("Hello [[world");
        let trigger = WikilinkCompletion::check_trigger(&buf, 13);
        assert_eq!(trigger, Some((6, "world".to_string())));

        let buf2 = EditBuffer::from_str("Hello [[world]] more");
        let trigger2 = WikilinkCompletion::check_trigger(&buf2, 17);
        assert_eq!(trigger2, None);

        let buf3 = EditBuffer::from_str("Hello [[note|alias");
        let trigger3 = WikilinkCompletion::check_trigger(&buf3, 18);
        assert_eq!(trigger3, None);
    }

    #[test]
    fn test_completion_insertion_unclosed() {
        let mut buf = EditBuffer::from_str("See [[pro");
        let mut completion = WikilinkCompletion::new();
        completion.is_active = true;
        completion.trigger_char_pos = 4;
        completion.query = "pro".to_string();
        completion.items = vec![CompletionItem {
            label: "Project Plan".to_string(),
            detail: None,
            insert_text: "Project Plan".to_string(),
            kind: CompletionKind::Note,
        }];

        let new_cursor = completion.insert_selected(&mut buf, 9).unwrap();
        assert_eq!(buf.to_string(), "See [[Project Plan]]");
        assert_eq!(new_cursor, 20);
    }

    #[test]
    fn test_completion_insertion_already_closed() {
        let mut buf = EditBuffer::from_str("See [[pro]] text");
        let mut completion = WikilinkCompletion::new();
        completion.is_active = true;
        completion.trigger_char_pos = 4;
        completion.query = "pro".to_string();
        completion.items = vec![CompletionItem {
            label: "Project Plan".to_string(),
            detail: None,
            insert_text: "Project Plan".to_string(),
            kind: CompletionKind::Note,
        }];

        let new_cursor = completion.insert_selected(&mut buf, 9).unwrap();
        assert_eq!(buf.to_string(), "See [[Project Plan]] text");
        assert_eq!(new_cursor, 20);
    }

    #[test]
    fn test_generate_heading_completion() {
        let buf = EditBuffer::from_str("# Intro\nText\n## Setup Guide\nMore");
        let items = generate_completion_items("#Set", None, None, &buf);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "## Setup Guide");
        assert_eq!(items[0].insert_text, "#Setup Guide");
    }
}
