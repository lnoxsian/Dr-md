use super::Editor;
use egui::text::LayoutJob;
use egui::{Color32, FontId, TextFormat};

fn get_editor_id(editor: &Editor) -> egui::Id {
    if let Some(ref path) = editor.active_path {
        egui::Id::new(("editor_text_edit", path))
    } else {
        egui::Id::new("editor_text_edit")
    }
}

pub struct EditorRenderer {
    pub content_buffer: String,
    pub last_version: usize,
    pub previous_text: String,
    pub previous_cursor: Option<egui::text::CCursorRange>,
    pub find_visible: bool,
    pub find_query: String,
    pub use_regex: bool,
    pub case_sensitive: bool,
    pub matches: Vec<std::ops::Range<usize>>,
    pub byte_matches: Vec<std::ops::Range<usize>>,
    pub active_match_index: Option<usize>,
    pub focus_search_input: bool,
    pub scroll_to_cursor_requested: bool,
    pub replace_query: String,
    pub replace_visible: bool,
}

impl EditorRenderer {
    pub fn new() -> Self {
        Self {
            content_buffer: String::new(),
            last_version: usize::MAX,
            previous_text: String::new(),
            previous_cursor: None,
            find_visible: false,
            find_query: String::new(),
            use_regex: false,
            case_sensitive: false,
            matches: Vec::new(),
            byte_matches: Vec::new(),
            active_match_index: None,
            focus_search_input: false,
            scroll_to_cursor_requested: false,
            replace_query: String::new(),
            replace_visible: false,
        }
    }

    pub fn sync_from_editor(&mut self, editor: &Editor, ctx: &egui::Context) {
        if editor.version != self.last_version {
            self.content_buffer = editor.buffer.to_string();
            self.last_version = editor.version;
            self.previous_text = self.content_buffer.clone();
            self.previous_cursor = None;

            if let Some(mut text_state) =
                egui::widgets::text_edit::TextEditState::load(ctx, get_editor_id(editor))
            {
                let anchor = egui::text::CCursor::new(editor.selection.anchor);
                let head = egui::text::CCursor::new(editor.selection.head);
                text_state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(anchor, head)));
                text_state.store(ctx, get_editor_id(editor));
            }
        }
    }

    pub fn sync_to_editor(&mut self, editor: &mut Editor, ctx: &egui::Context) {
        if let Some(text_state) =
            egui::widgets::text_edit::TextEditState::load(ctx, get_editor_id(editor))
            && let Some(range) = text_state.cursor.char_range() {
                editor.cursor.char_idx = range.primary.index;
                editor.selection.anchor = range.secondary.index;
                editor.selection.head = range.primary.index;
            }
        editor.sync_text(&self.content_buffer);
        self.last_version = editor.version;
    }

    pub fn update_find_matches(&mut self) {
        self.matches.clear();
        self.byte_matches.clear();
        if self.find_query.is_empty() {
            self.active_match_index = None;
            return;
        }

        // Build byte-to-char index mapping
        let mut byte_to_char = vec![0; self.content_buffer.len() + 1];
        let mut char_idx = 0;
        let mut byte_idx = 0;
        for c in self.content_buffer.chars() {
            let len = c.len_utf8();
            for _ in 0..len {
                if byte_idx < byte_to_char.len() {
                    byte_to_char[byte_idx] = char_idx;
                }
                byte_idx += 1;
            }
            char_idx += 1;
        }
        if byte_idx < byte_to_char.len() {
            byte_to_char[byte_idx] = char_idx;
        }

        if self.use_regex {
            let mut builder = regex::RegexBuilder::new(&self.find_query);
            builder.case_insensitive(!self.case_sensitive);
            if let Ok(re) = builder.build() {
                for m in re.find_iter(&self.content_buffer) {
                    if m.start() < m.end() {
                        let c_start = byte_to_char.get(m.start()).cloned().unwrap_or(char_idx);
                        let c_end = byte_to_char.get(m.end()).cloned().unwrap_or(char_idx);
                        self.matches.push(c_start..c_end);
                        self.byte_matches.push(m.start()..m.end());
                    }
                }
            }
        } else {
            // Normal string search (case sensitive or insensitive)
            let query = if self.case_sensitive {
                self.find_query.clone()
            } else {
                self.find_query.to_lowercase()
            };
            let text = if self.case_sensitive {
                self.content_buffer.clone()
            } else {
                self.content_buffer.to_lowercase()
            };

            let mut start_byte = 0;
            while let Some(idx) = text[start_byte..].find(&query) {
                let actual_start_byte = start_byte + idx;
                let actual_end_byte = actual_start_byte + query.len();
                let c_start = byte_to_char
                    .get(actual_start_byte)
                    .cloned()
                    .unwrap_or(char_idx);
                let c_end = byte_to_char
                    .get(actual_end_byte)
                    .cloned()
                    .unwrap_or(char_idx);
                self.matches.push(c_start..c_end);
                self.byte_matches.push(actual_start_byte..actual_end_byte);
                start_byte = actual_end_byte;
            }
        }

        // Adjust active_match_index
        if self.matches.is_empty() {
            self.active_match_index = None;
        } else {
            if let Some(idx) = self.active_match_index {
                if idx >= self.matches.len() {
                    self.active_match_index = Some(0);
                }
            } else {
                self.active_match_index = Some(0);
            }
        }
    }

    pub fn find_matches_in_text(&self, text: &str) -> Vec<std::ops::Range<usize>> {
        if self.find_query.is_empty() {
            return Vec::new();
        }
        let mut matches = Vec::new();
        if self.use_regex {
            let mut builder = regex::RegexBuilder::new(&self.find_query);
            builder.case_insensitive(!self.case_sensitive);
            if let Ok(re) = builder.build() {
                for mat in re.find_iter(text) {
                    let start_char = text[..mat.start()].chars().count();
                    let end_char = start_char + text[mat.start()..mat.end()].chars().count();
                    matches.push(start_char..end_char);
                }
            }
        } else {
            let query = if self.case_sensitive {
                self.find_query.clone()
            } else {
                self.find_query.to_lowercase()
            };
            let search_text = if self.case_sensitive {
                text.to_string()
            } else {
                text.to_lowercase()
            };

            let mut start_pos = 0;
            while let Some(idx) = search_text[start_pos..].find(&query) {
                let match_start_byte = start_pos + idx;
                let match_end_byte = match_start_byte + query.len();

                let start_char = text[..match_start_byte].chars().count();
                let end_char = start_char + text[match_start_byte..match_end_byte].chars().count();
                matches.push(start_char..end_char);

                start_pos = match_end_byte;
                if query.is_empty() {
                    break;
                }
            }
        }
        matches
    }

    pub fn navigate_match(&mut self, next: bool, editor: &mut Editor, ctx: &egui::Context) {
        if self.matches.is_empty() {
            return;
        }

        let new_idx = if let Some(curr) = self.active_match_index {
            if next {
                (curr + 1) % self.matches.len()
            } else {
                if curr == 0 {
                    self.matches.len() - 1
                } else {
                    curr - 1
                }
            }
        } else {
            0
        };

        self.active_match_index = Some(new_idx);
        let range = &self.matches[new_idx];

        // Update editor state cursor range
        editor.selection.anchor = range.start;
        editor.selection.head = range.end;
        editor.cursor.char_idx = range.end;
        editor.version += 1;
        self.scroll_to_cursor_requested = true;

        // Store back in egui's text edit state so that egui scrolls to it!
        if let Some(mut text_state) =
            egui::widgets::text_edit::TextEditState::load(ctx, get_editor_id(editor))
        {
            let anchor = egui::text::CCursor::new(range.start);
            let head = egui::text::CCursor::new(range.end);
            text_state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(anchor, head)));
            text_state.store(ctx, get_editor_id(editor));
        }
    }

    pub fn replace_current(&mut self, editor: &mut Editor, ctx: &egui::Context) {
        if self.matches.is_empty() {
            return;
        }
        let idx = match self.active_match_index {
            Some(i) => i,
            None => return,
        };
        if idx >= self.matches.len() {
            return;
        }
        let range = self.matches[idx].clone();

        let replacement = if self.use_regex {
            let mut builder = regex::RegexBuilder::new(&self.find_query);
            builder.case_insensitive(!self.case_sensitive);
            if let Ok(re) = builder.build() {
                let matched_str = self
                    .content_buffer
                    .chars()
                    .skip(range.start)
                    .take(range.end - range.start)
                    .collect::<String>();
                let mut expanded = String::new();
                if let Some(captures) = re.captures(&matched_str) {
                    captures.expand(&self.replace_query, &mut expanded);
                } else {
                    expanded = self.replace_query.clone();
                }
                expanded
            } else {
                self.replace_query.clone()
            }
        } else {
            self.replace_query.clone()
        };

        let mut chars: Vec<char> = self.content_buffer.chars().collect();
        if range.start <= chars.len() && range.end <= chars.len() {
            let replace_chars: Vec<char> = replacement.chars().collect();
            chars.splice(range.start..range.end, replace_chars);
            self.content_buffer = chars.into_iter().collect();

            editor.sync_text(&self.content_buffer);
            editor.version += 1;

            self.update_find_matches();

            let new_cursor_idx = range.start + replacement.chars().count();
            editor.selection.anchor = new_cursor_idx;
            editor.selection.head = new_cursor_idx;
            editor.cursor.char_idx = new_cursor_idx;

            if let Some(mut text_state) =
                egui::widgets::text_edit::TextEditState::load(ctx, get_editor_id(editor))
            {
                let cursor = egui::text::CCursor::new(new_cursor_idx);
                text_state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(cursor, cursor)));
                text_state.store(ctx, get_editor_id(editor));
            }

            if !self.matches.is_empty() {
                let next_idx = idx.min(self.matches.len() - 1);
                self.active_match_index = Some(next_idx);
                let next_range = self.matches[next_idx].clone();
                editor.selection.anchor = next_range.start;
                editor.selection.head = next_range.end;
                editor.cursor.char_idx = next_range.end;

                if let Some(mut text_state) =
                    egui::widgets::text_edit::TextEditState::load(ctx, get_editor_id(editor))
                {
                    let anchor = egui::text::CCursor::new(next_range.start);
                    let head = egui::text::CCursor::new(next_range.end);
                    text_state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::two(anchor, head)));
                    text_state.store(ctx, get_editor_id(editor));
                }
                self.scroll_to_cursor_requested = true;
            }
        }
    }

    pub fn replace_all(&mut self, editor: &mut Editor) {
        if self.matches.is_empty() {
            return;
        }

        if self.use_regex {
            let mut builder = regex::RegexBuilder::new(&self.find_query);
            builder.case_insensitive(!self.case_sensitive);
            if let Ok(re) = builder.build() {
                let replaced = re.replace_all(&self.content_buffer, &self.replace_query);
                self.content_buffer = replaced.into_owned();
            }
        } else {
            let mut chars: Vec<char> = self.content_buffer.chars().collect();
            let replace_chars: Vec<char> = self.replace_query.chars().collect();
            for range in self.matches.iter().rev() {
                if range.start <= chars.len() && range.end <= chars.len() {
                    chars.splice(range.start..range.end, replace_chars.clone());
                }
            }
            self.content_buffer = chars.into_iter().collect();
        }

        editor.sync_text(&self.content_buffer);
        editor.version += 1;

        self.update_find_matches();
    }

    fn process_autoclosing(
        &mut self,
        ctx: &egui::Context,
        id: egui::Id,
        mut state: egui::widgets::text_edit::TextEditState,
        ui_changed: bool,
    ) -> bool {
        let mut text_changed = false;

        if let Some(range) = state.cursor.char_range() {
            let current_idx = range.primary.index;
            let secondary_idx = range.secondary.index;

            // Only perform autoclosing/modifications if the text was actually changed by typing/deleting
            if ui_changed {
                // Check if there was a selection in the previous frame
                if let Some(prev_range) = self.previous_cursor {
                    let prev_start = prev_range.primary.index.min(prev_range.secondary.index);
                    let prev_end = prev_range.primary.index.max(prev_range.secondary.index);

                    if prev_start != prev_end {
                        // There was a selection!
                        // Let's see if the user replaced it by typing a single character.
                        // Egui would replace the selection with the character, making the new cursor a single cursor
                        // at start + 1.
                        if current_idx == secondary_idx && current_idx == prev_start + 1 {
                            let new_chars: Vec<char> = self.content_buffer.chars().collect();
                            if prev_start < new_chars.len() {
                                let typed_char = new_chars[prev_start];
                                let selected_text: String = self
                                    .previous_text
                                    .chars()
                                    .skip(prev_start)
                                    .take(prev_end - prev_start)
                                    .collect();

                                let mut replacement = None;
                                let mut cursor_offset = 0;

                                match typed_char {
                                    '(' => {
                                        replacement = Some(format!("({})", selected_text));
                                        cursor_offset = selected_text.len() + 2;
                                    }
                                    '[' => {
                                        replacement = Some(format!("[{}]()", selected_text));
                                        cursor_offset = selected_text.len() + 3;
                                    }
                                    '{' => {
                                        replacement = Some(format!("{{{}}}", selected_text));
                                        cursor_offset = selected_text.len() + 2;
                                    }
                                    '"' => {
                                        replacement = Some(format!("\"{}\"", selected_text));
                                        cursor_offset = selected_text.len() + 2;
                                    }
                                    '\'' => {
                                        replacement = Some(format!("'{}\'", selected_text));
                                        cursor_offset = selected_text.len() + 2;
                                    }
                                    '`' => {
                                        replacement = Some(format!("`{}`", selected_text));
                                        cursor_offset = selected_text.len() + 2;
                                    }
                                    '*' => {
                                        replacement = Some(format!("*{}*", selected_text));
                                        cursor_offset = selected_text.len() + 2;
                                    }
                                    '_' => {
                                        replacement = Some(format!("_{}_", selected_text));
                                        cursor_offset = selected_text.len() + 2;
                                    }
                                    '~' => {
                                        replacement = Some(format!("~{}~", selected_text));
                                        cursor_offset = selected_text.len() + 2;
                                    }
                                    _ => {}
                                }

                                if let Some(rep) = replacement {
                                    let mut final_text = String::new();
                                    final_text.push_str(
                                        &self
                                            .content_buffer
                                            .chars()
                                            .take(prev_start)
                                            .collect::<String>(),
                                    );
                                    final_text.push_str(&rep);
                                    final_text.push_str(
                                        &self
                                            .content_buffer
                                            .chars()
                                            .skip(prev_start + 1)
                                            .collect::<String>(),
                                    );

                                    self.content_buffer = final_text;
                                    text_changed = true;

                                    let new_cursor_idx = prev_start + cursor_offset;
                                    let ccursor = egui::text::CCursor::new(new_cursor_idx);
                                    state.cursor.set_char_range(Some(
                                        egui::text::CCursorRange::two(ccursor, ccursor),
                                    ));
                                }
                            }
                        }
                    } else {
                        // No selection in previous frame.
                        let prev_idx = prev_range.primary.index;
                        if current_idx == secondary_idx {
                            if current_idx == prev_idx + 1 {
                                let new_chars: Vec<char> = self.content_buffer.chars().collect();
                                if prev_idx < new_chars.len() {
                                    let typed_char = new_chars[prev_idx];

                                    // 0. HANDLE ENTER BETWEEN BRACKETS AND QUOTES
                                    let mut enter_between_pairs = false;
                                    if typed_char == '\n'
                                        && prev_idx > 0 && prev_idx + 1 < new_chars.len() {
                                            let left = new_chars[prev_idx - 1];
                                            let right = new_chars[prev_idx + 1];
                                            let is_pair = [
                                                ('{', '}'),
                                                ('(', ')'),
                                                ('[', ']'),
                                                ('"', '"'),
                                                ('\'', '\''),
                                                ('`', '`'),
                                            ]
                                            .iter()
                                            .any(|&(l, r)| l == left && r == right);

                                            if is_pair {
                                                let mut final_text = String::new();
                                                final_text.push_str(
                                                    &self
                                                        .content_buffer
                                                        .chars()
                                                        .take(current_idx)
                                                        .collect::<String>(),
                                                );
                                                final_text.push('\n');
                                                final_text.push_str(
                                                    &self
                                                        .content_buffer
                                                        .chars()
                                                        .skip(current_idx)
                                                        .collect::<String>(),
                                                );

                                                self.content_buffer = final_text;
                                                text_changed = true;
                                                enter_between_pairs = true;

                                                let ccursor = egui::text::CCursor::new(current_idx);
                                                state.cursor.set_char_range(Some(
                                                    egui::text::CCursorRange::two(ccursor, ccursor),
                                                ));
                                            }
                                        }

                                    // 1. STEP OVER CLOSING BRACKETS
                                    let mut stepped_over = false;
                                    if !enter_between_pairs
                                        && [')', ']', '}', '"', '\'', '`'].contains(&typed_char)
                                        && current_idx < new_chars.len()
                                            && new_chars[current_idx] == typed_char
                                        {
                                            let mut final_text = String::new();
                                            final_text.push_str(
                                                &self
                                                    .content_buffer
                                                    .chars()
                                                    .take(current_idx)
                                                    .collect::<String>(),
                                            );
                                            final_text.push_str(
                                                &self
                                                    .content_buffer
                                                    .chars()
                                                    .skip(current_idx + 1)
                                                    .collect::<String>(),
                                            );

                                            self.content_buffer = final_text;
                                            text_changed = true;
                                            stepped_over = true;

                                            let ccursor = egui::text::CCursor::new(current_idx);
                                            state.cursor.set_char_range(Some(
                                                egui::text::CCursorRange::two(ccursor, ccursor),
                                            ));
                                        }

                                     if !enter_between_pairs && !stepped_over {
                                         // 2. AUTOCLOSE OPENING BRACKETS & QUOTES
                                         let next_char = new_chars.get(current_idx).copied();
                                         let prev_char = if prev_idx > 0 {
                                             new_chars.get(prev_idx - 1).copied()
                                         } else {
                                             None
                                         };

                                         let is_before_word =
                                             next_char.map_or(false, |c| c.is_alphanumeric() || c == '_');
                                         let is_after_word =
                                             prev_char.map_or(false, |c| c.is_alphanumeric() || c == '_');

                                         let mut autoclose_char = None;
                                         match typed_char {
                                             '(' => if !is_before_word { autoclose_char = Some(')') },
                                             '[' => if !is_before_word { autoclose_char = Some(']') },
                                             '{' => if !is_before_word { autoclose_char = Some('}') },
                                             '"' => {
                                                 if !is_before_word && !is_after_word {
                                                     autoclose_char = Some('"');
                                                 }
                                             }
                                             '\'' => {
                                                 // Don't auto-close single quote if after a word (e.g. apostrophe in don't)
                                                 if !is_before_word && !is_after_word {
                                                     autoclose_char = Some('\'');
                                                 }
                                             }
                                             '`' => {
                                                 if prev_idx >= 2
                                                     && new_chars[prev_idx - 1] == '`'
                                                     && new_chars[prev_idx - 2] == '`'
                                                 {
                                                     let remaining_on_line: String = new_chars[current_idx..]
                                                         .iter()
                                                         .take_while(|&&c| c != '\n')
                                                         .collect();

                                                     let is_line_rest_empty = remaining_on_line.trim().is_empty();
                                                     let already_has_closing_fences = remaining_on_line.trim_start().starts_with("```");

                                                     if is_line_rest_empty && !already_has_closing_fences {
                                                         let mut final_text = String::new();
                                                         final_text.push_str(
                                                             &self
                                                                 .content_buffer
                                                                 .chars()
                                                                 .take(current_idx)
                                                                 .collect::<String>(),
                                                         );
                                                         final_text.push_str("```");
                                                         final_text.push_str(
                                                             &self
                                                                 .content_buffer
                                                                 .chars()
                                                                 .skip(current_idx)
                                                                 .collect::<String>(),
                                                         );

                                                         self.content_buffer = final_text;
                                                         text_changed = true;

                                                         let ccursor =
                                                             egui::text::CCursor::new(current_idx);
                                                         state.cursor.set_char_range(Some(
                                                             egui::text::CCursorRange::two(
                                                                 ccursor, ccursor,
                                                             ),
                                                         ));
                                                     }
                                                 } else if !is_before_word && !is_after_word {
                                                     autoclose_char = Some('`');
                                                 }
                                             }
                                             '*' => {
                                                if prev_idx >= 1 && new_chars[prev_idx - 1] == '*' {
                                                    let mut final_text = String::new();
                                                    final_text.push_str(
                                                        &self
                                                            .content_buffer
                                                            .chars()
                                                            .take(current_idx)
                                                            .collect::<String>(),
                                                    );
                                                    final_text.push_str("**");
                                                    final_text.push_str(
                                                        &self
                                                            .content_buffer
                                                            .chars()
                                                            .skip(current_idx)
                                                            .collect::<String>(),
                                                    );

                                                    self.content_buffer = final_text;
                                                    text_changed = true;

                                                    let ccursor =
                                                        egui::text::CCursor::new(current_idx);
                                                    state.cursor.set_char_range(Some(
                                                        egui::text::CCursorRange::two(
                                                            ccursor, ccursor,
                                                        ),
                                                    ));
                                                }
                                            }
                                            '_' => {
                                                if prev_idx >= 1 && new_chars[prev_idx - 1] == '_' {
                                                    let mut final_text = String::new();
                                                    final_text.push_str(
                                                        &self
                                                            .content_buffer
                                                            .chars()
                                                            .take(current_idx)
                                                            .collect::<String>(),
                                                    );
                                                    final_text.push_str("__");
                                                    final_text.push_str(
                                                        &self
                                                            .content_buffer
                                                            .chars()
                                                            .skip(current_idx)
                                                            .collect::<String>(),
                                                    );

                                                    self.content_buffer = final_text;
                                                    text_changed = true;

                                                    let ccursor =
                                                        egui::text::CCursor::new(current_idx);
                                                    state.cursor.set_char_range(Some(
                                                        egui::text::CCursorRange::two(
                                                            ccursor, ccursor,
                                                        ),
                                                    ));
                                                }
                                            }
                                            '~'
                                                if prev_idx >= 1 && new_chars[prev_idx - 1] == '~' => {
                                                    let mut final_text = String::new();
                                                    final_text.push_str(
                                                        &self
                                                            .content_buffer
                                                            .chars()
                                                            .take(current_idx)
                                                            .collect::<String>(),
                                                    );
                                                    final_text.push_str("~~");
                                                    final_text.push_str(
                                                        &self
                                                            .content_buffer
                                                            .chars()
                                                            .skip(current_idx)
                                                            .collect::<String>(),
                                                    );

                                                    self.content_buffer = final_text;
                                                    text_changed = true;

                                                    let ccursor =
                                                        egui::text::CCursor::new(current_idx);
                                                    state.cursor.set_char_range(Some(
                                                        egui::text::CCursorRange::two(
                                                            ccursor, ccursor,
                                                        ),
                                                    ));
                                                }
                                            _ => {}
                                        }

                                        if let Some(ac) = autoclose_char {
                                            let mut final_text = String::new();
                                            final_text.push_str(
                                                &self
                                                    .content_buffer
                                                    .chars()
                                                    .take(current_idx)
                                                    .collect::<String>(),
                                            );
                                            final_text.push(ac);
                                            final_text.push_str(
                                                &self
                                                    .content_buffer
                                                    .chars()
                                                    .skip(current_idx)
                                                    .collect::<String>(),
                                            );

                                            self.content_buffer = final_text;
                                            text_changed = true;

                                            let ccursor = egui::text::CCursor::new(current_idx);
                                            state.cursor.set_char_range(Some(
                                                egui::text::CCursorRange::two(ccursor, ccursor),
                                            ));
                                        }
                                    }
                                }
                            } else if current_idx == prev_idx.saturating_sub(1) && prev_idx > 0 {
                                let old_chars: Vec<char> = self.previous_text.chars().collect();
                                let new_chars: Vec<char> = self.content_buffer.chars().collect();
                                if current_idx < old_chars.len() {
                                    let deleted_char = old_chars[current_idx];
                                    let mut matching_close = None;
                                    match deleted_char {
                                        '(' => matching_close = Some(')'),
                                        '[' => matching_close = Some(']'),
                                        '{' => matching_close = Some('}'),
                                        '"' => matching_close = Some('"'),
                                        '\'' => matching_close = Some('\''),
                                        '`' => matching_close = Some('`'),
                                        _ => {}
                                    }
                                    if let Some(close_char) = matching_close
                                        && current_idx < new_chars.len()
                                            && new_chars[current_idx] == close_char
                                        {
                                            let mut final_text = String::new();
                                            final_text.push_str(
                                                &self
                                                    .content_buffer
                                                    .chars()
                                                    .take(current_idx)
                                                    .collect::<String>(),
                                            );
                                            final_text.push_str(
                                                &self
                                                    .content_buffer
                                                    .chars()
                                                    .skip(current_idx + 1)
                                                    .collect::<String>(),
                                            );

                                            self.content_buffer = final_text;
                                            text_changed = true;

                                            let ccursor = egui::text::CCursor::new(current_idx);
                                            state.cursor.set_char_range(Some(
                                                egui::text::CCursorRange::two(ccursor, ccursor),
                                            ));
                                        }
                                }
                            }
                        }
                    }
                }
            }

            self.previous_cursor = state.cursor.char_range();
        } else {
            self.previous_cursor = None;
        }

        self.previous_text = self.content_buffer.clone();

        if text_changed {
            state.store(ctx, id);
        }

        text_changed
    }

    pub fn show_find_panel(
        &mut self,
        ui: &mut egui::Ui,
        editor: &mut Editor,
        enable_replace: bool,
    ) {
        if self.find_visible && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.find_visible = false;
        }

        self.sync_from_editor(editor, ui.ctx());

        if self.find_visible {
            egui::Frame::none()
                .fill(ui.visuals().panel_fill)
                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                .inner_margin(egui::Margin::symmetric(12.0, 6.0))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);

                    egui::Grid::new("find_replace_grid")
                        .num_columns(2)
                        .spacing(egui::vec2(6.0, 6.0))
                        .show(ui, |ui| {
                            // Row 1 Column 1: Find Input Box
                            let search_input_id = ui.make_persistent_id("find_search_input");
                            let has_focus = ui.memory(|mem| mem.has_focus(search_input_id));

                            if has_focus {
                                if ui.input_mut(|i| {
                                    i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                                }) {
                                    self.navigate_match(true, editor, ui.ctx());
                                } else if ui.input_mut(|i| {
                                    i.consume_key(egui::Modifiers::SHIFT, egui::Key::Enter)
                                }) {
                                    self.navigate_match(false, editor, ui.ctx());
                                }

                                if ui.input_mut(|i| {
                                    i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                                }) {
                                    self.find_visible = false;
                                }
                            }

                            let text_edit_res = ui.add_sized(
                                egui::vec2(120.0, 20.0),
                                egui::TextEdit::singleline(&mut self.find_query)
                                    .id(search_input_id)
                                    .hint_text("Find in file..."),
                            );

                            if self.focus_search_input {
                                text_edit_res.request_focus();
                                self.focus_search_input = false;
                            }

                            if text_edit_res.changed() {
                                self.update_find_matches();
                            }

                            // Row 1 Column 2: Find Actions & Options
                            ui.horizontal(|ui| {
                                let case_btn = ui.selectable_label(self.case_sensitive, "Aa");
                                if case_btn.clicked() {
                                    self.case_sensitive = !self.case_sensitive;
                                    self.update_find_matches();
                                }
                                case_btn.on_hover_text("Match Case");

                                let regex_btn = ui.selectable_label(self.use_regex, ".*");
                                if regex_btn.clicked() {
                                    self.use_regex = !self.use_regex;
                                    self.update_find_matches();
                                }
                                regex_btn.on_hover_text("Use Regular Expression");

                                if !self.find_query.is_empty() {
                                    let count = self.matches.len();
                                    if count == 0 {
                                        ui.label(
                                            egui::RichText::new("No results").color(Color32::GRAY),
                                        );
                                    } else {
                                        let current =
                                            self.active_match_index.map(|i| i + 1).unwrap_or(0);
                                        ui.label(format!("{} of {}", current, count));
                                    }
                                }

                                let prev_btn = ui.button("⏶");
                                if prev_btn.clicked() {
                                    self.navigate_match(false, editor, ui.ctx());
                                }
                                prev_btn.on_hover_text("Previous Match (Shift+Enter)");

                                let next_btn = ui.button("⏷");
                                if next_btn.clicked() {
                                    self.navigate_match(true, editor, ui.ctx());
                                }
                                next_btn.on_hover_text("Next Match (Enter)");

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("×").clicked() {
                                            self.find_visible = false;
                                        }

                                        if enable_replace {
                                            let toggle_label =
                                                if self.replace_visible { "-" } else { "+" };
                                            let toggle_btn = ui.button(toggle_label);
                                            if toggle_btn.clicked() {
                                                self.replace_visible = !self.replace_visible;
                                            }
                                            toggle_btn.on_hover_text("Toggle Replace Panel");
                                        }
                                    },
                                );
                            });
                            ui.end_row();

                            if enable_replace && self.replace_visible {
                                // Row 2 Column 1: Replace Input Box
                                let replace_input_id = ui.make_persistent_id("find_replace_input");
                                ui.add_sized(
                                    egui::vec2(120.0, 20.0),
                                    egui::TextEdit::singleline(&mut self.replace_query)
                                        .id(replace_input_id)
                                        .hint_text("Replace with..."),
                                );

                                // Row 2 Column 2: Replace Actions
                                ui.horizontal(|ui| {
                                    if ui.button("Replace").clicked() {
                                        self.replace_current(editor, ui.ctx());
                                    }
                                    if ui.button("Replace All").clicked() {
                                        self.replace_all(editor);
                                    }
                                });
                                ui.end_row();
                            }
                        });
                });
            ui.add_space(6.0);
        }
    }

    fn handle_tab_key(
        &mut self,
        ui: &mut egui::Ui,
        editor_id: egui::Id,
        tab_pressed: bool,
        shift_tab_pressed: bool,
        tab_width: usize,
    ) -> bool {
        if !tab_pressed && !shift_tab_pressed {
            return false;
        }

        ui.memory_mut(|mem| mem.request_focus(editor_id));

        if let Some(mut text_state) = egui::widgets::text_edit::TextEditState::load(ui.ctx(), editor_id)
            && let Some(range) = text_state.cursor.char_range() {
                let spaces = " ".repeat(tab_width);
                
                if range.primary.index == range.secondary.index {
                    // No selection - single cursor
                    let cursor_idx = range.primary.index;
                    let chars: Vec<char> = self.content_buffer.chars().collect();
                    
                    if tab_pressed {
                        let mut new_chars = chars;
                        let insert_idx = cursor_idx.min(new_chars.len());
                        for c in spaces.chars().rev() {
                            new_chars.insert(insert_idx, c);
                        }
                        self.content_buffer = new_chars.into_iter().collect();
                        let new_cursor = cursor_idx + tab_width;
                        let ccursor = egui::text::CCursor::new(new_cursor);
                        text_state.cursor.set_char_range(Some(egui::text::CCursorRange::two(ccursor, ccursor)));
                    } else {
                        // Shift + Tab (unindent current line)
                        let mut line_start = 0;
                        for i in (0..cursor_idx.min(chars.len())).rev() {
                            if chars[i] == '\n' {
                                line_start = i + 1;
                                break;
                            }
                        }
                        let mut spaces_to_remove = 0;
                        for i in 0..tab_width {
                            if line_start + i < chars.len() && chars[line_start + i] == ' ' {
                                spaces_to_remove += 1;
                            } else {
                                break;
                            }
                        }
                        if spaces_to_remove > 0 {
                            let mut new_chars = chars;
                            new_chars.drain(line_start..line_start + spaces_to_remove);
                            self.content_buffer = new_chars.into_iter().collect();
                            let new_cursor = if cursor_idx >= line_start + spaces_to_remove {
                                cursor_idx - spaces_to_remove
                            } else {
                                line_start
                            };
                            let ccursor = egui::text::CCursor::new(new_cursor);
                            text_state.cursor.set_char_range(Some(egui::text::CCursorRange::two(ccursor, ccursor)));
                        }
                    }
                } else {
                    // Selection - block indent/unindent
                    let start_idx = range.primary.index.min(range.secondary.index);
                    let end_idx = range.primary.index.max(range.secondary.index);
                    let chars: Vec<char> = self.content_buffer.chars().collect();
                    
                    let mut first_line_start = 0;
                    for i in (0..start_idx.min(chars.len())).rev() {
                        if chars[i] == '\n' {
                            first_line_start = i + 1;
                            break;
                        }
                    }
                    
                    let mut selected_line_starts = vec![first_line_start];
                    for (i, c) in chars.iter().enumerate().take(end_idx.min(chars.len())).skip(first_line_start) {
                        if *c == '\n' && i + 1 < end_idx {
                            selected_line_starts.push(i + 1);
                        }
                    }
                    
                    if tab_pressed {
                        let mut new_chars = chars;
                        let mut added_chars = 0;
                        for &line_start in selected_line_starts.iter().rev() {
                            for c in spaces.chars().rev() {
                                new_chars.insert(line_start, c);
                            }
                            added_chars += tab_width;
                        }
                        self.content_buffer = new_chars.into_iter().collect();
                        let new_start = start_idx + tab_width;
                        let new_end = end_idx + added_chars;
                        let (anchor, head) = if range.primary.index >= range.secondary.index {
                            (new_start, new_end)
                        } else {
                            (new_end, new_start)
                        };
                        let c_anchor = egui::text::CCursor::new(anchor);
                        let c_head = egui::text::CCursor::new(head);
                        text_state.cursor.set_char_range(Some(egui::text::CCursorRange::two(c_anchor, c_head)));
                    } else {
                        // Shift + Tab block unindent
                        let mut new_chars = chars;
                        let mut removed_chars = 0;
                        let mut first_line_removed = 0;
                        for &line_start in selected_line_starts.iter().rev() {
                            let mut spaces_to_remove = 0;
                            for i in 0..tab_width {
                                if line_start + i < new_chars.len() && new_chars[line_start + i] == ' ' {
                                    spaces_to_remove += 1;
                                } else {
                                    break;
                                }
                            }
                            if spaces_to_remove > 0 {
                                new_chars.drain(line_start..line_start + spaces_to_remove);
                                removed_chars += spaces_to_remove;
                                if line_start == first_line_start {
                                    first_line_removed = spaces_to_remove;
                                }
                            }
                        }
                        self.content_buffer = new_chars.into_iter().collect();
                        let new_start = (start_idx as isize - first_line_removed as isize).max(first_line_start as isize) as usize;
                        let new_end = (end_idx as isize - removed_chars as isize).max(new_start as isize) as usize;
                        let (anchor, head) = if range.primary.index >= range.secondary.index {
                            (new_start, new_end)
                        } else {
                            (new_end, new_start)
                        };
                        let c_anchor = egui::text::CCursor::new(anchor);
                        let c_head = egui::text::CCursor::new(head);
                        text_state.cursor.set_char_range(Some(egui::text::CCursorRange::two(c_anchor, c_head)));
                    }
                }
                text_state.store(ui.ctx(), editor_id);
                return true;
            }
        false
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        editor: &mut Editor,
        font_size: f32,
        line_numbers: bool,
        cursor_style: crate::config::CursorStyle,
        tab_width: usize,
    ) {
        let editor_id = get_editor_id(editor);
        let had_focus = ui.memory(|mem| mem.has_focus(editor_id));
        
        let mut tab_pressed = false;
        let mut shift_tab_pressed = false;
        if had_focus {
            ui.input_mut(|i| {
                if i.key_pressed(egui::Key::Tab) && !i.modifiers.command {
                    if i.modifiers.shift {
                        shift_tab_pressed = true;
                        i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab);
                    } else {
                        tab_pressed = true;
                        i.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
                    }
                }
            });
        }
        let mut previous_selection = None;
        if let Some(text_state) =
            egui::widgets::text_edit::TextEditState::load(ui.ctx(), get_editor_id(editor))
            && let Some(range) = text_state.cursor.char_range()
                && range.primary.index != range.secondary.index {
                    previous_selection = Some(range);
                }
        if previous_selection.is_none() {
            if let Some(r) = editor.selection.range() {
                let start = egui::text::CCursor::new(r.start);
                let end = egui::text::CCursor::new(r.end);
                previous_selection = Some(egui::text::CCursorRange::two(start, end));
            }
        }
        let is_right_click = ui.input(|i| i.pointer.secondary_pressed() || i.pointer.secondary_down());

        self.show_find_panel(ui, editor, true);

        let find_visible = self.find_visible;
        let byte_matches = self.byte_matches.clone();
        let active_match_idx = self.active_match_index;

        if line_numbers {
            let mut line_positions = Vec::new();

            let output = egui::ScrollArea::vertical()
                .id_source("editor_scroll")
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    self.handle_drag_autoscroll(ui, editor);

                    egui::Frame::none()
                        .inner_margin(egui::Margin::symmetric(24.0, 8.0))
                        .show(ui, |ui| {
                            let available_width = ui.available_width();
                            let gutter_width = 30.0;
                            let spacing = ui.spacing().item_spacing.x;
                            let text_wrap_width =
                                (available_width - gutter_width - spacing - 8.0).max(100.0);

                            let (gutter_rect, edit_res, text_changed) = {
                                let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                                    let default_color = ui.style().visuals.text_color();
                                    let link_color = ui.style().visuals.hyperlink_color;
                                    let mut job = create_layout_job(
                                        text,
                                        font_size,
                                        default_color,
                                        link_color,
                                    );
                                    job.wrap.max_width = wrap_width.min(text_wrap_width);
                                    if find_visible && !byte_matches.is_empty() {
                                        highlight_matches(
                                            &mut job,
                                            &byte_matches,
                                            active_match_idx,
                                        );
                                    }
                                    let galley = ui.fonts(|f| f.layout_job(job));

                                    let mut positions = Vec::new();
                                    let mut line_idx = 1;
                                    let mut next_row_is_new_line = true;
                                    for row in &galley.rows {
                                        if next_row_is_new_line {
                                            positions.push((line_idx, row.rect.min.y));
                                            line_idx += 1;
                                        }
                                        next_row_is_new_line = row.ends_with_newline;
                                    }
                                    line_positions = positions;

                                    galley
                                };

                                ui.horizontal_top(|ui| {
                                    let gutter_width = 30.0;
                                    let (gutter_rect, _) = ui.allocate_at_least(
                                        egui::vec2(gutter_width, ui.available_height()),
                                        egui::Sense::hover(),
                                    );

                                    let editor_id = get_editor_id(editor);
                                    let is_menu_open = ui.memory(|m| m.is_popup_open(editor_id));
                                    if (is_right_click || is_menu_open)
                                        && let Some(prev_range) = previous_selection
                                        && let Some(mut text_state) =
                                            egui::widgets::text_edit::TextEditState::load(
                                                ui.ctx(),
                                                editor_id,
                                            )
                                        {
                                            text_state.cursor.set_char_range(Some(prev_range));
                                            text_state.store(ui.ctx(), editor_id);
                                            editor.cursor.char_idx = prev_range.primary.index;
                                            editor.selection.anchor = prev_range.secondary.index;
                                            editor.selection.head = prev_range.primary.index;
                                        }

                                    let text_edit =
                                        egui::TextEdit::multiline(&mut self.content_buffer)
                                            .id(editor_id)
                                            .font(FontId::monospace(font_size))
                                            .frame(false)
                                            .layouter(&mut layouter)
                                            .code_editor()
                                            .desired_width(f32::INFINITY);
                                    let edit_output = text_edit.show(ui);
                                    let edit_res = edit_output.response.clone();
                                    if self.scroll_to_cursor_requested
                                        && let Some(active_idx) = self.active_match_index
                                            && let Some(range) = self.matches.get(active_idx) {
                                                let local_rect =
                                                    edit_output.galley.pos_from_ccursor(
                                                        egui::text::CCursor::new(range.start),
                                                    );
                                                let screen_rect = local_rect
                                                    .translate(edit_res.rect.min.to_vec2());
                                                ui.scroll_to_rect(
                                                    screen_rect,
                                                    Some(egui::Align::Center),
                                                );
                                                self.scroll_to_cursor_requested = false;
                                            }

                                    self.update_cursor_screen_pos(editor, &edit_output);

                                    let content_buf = &mut self.content_buffer;
                                    edit_res.clone().context_menu(|ui| {
                                        Self::render_context_menu(ui, editor, content_buf);
                                    });

                                    self.draw_custom_cursor(
                                        ui,
                                        editor,
                                        &edit_output,
                                        cursor_style,
                                        font_size,
                                    );

                                    if (is_right_click || is_menu_open)
                                        && let Some(prev_range) = previous_selection
                                        && let Some(mut text_state) =
                                            egui::widgets::text_edit::TextEditState::load(
                                                ui.ctx(),
                                                editor_id,
                                            )
                                        {
                                            text_state.cursor.set_char_range(Some(prev_range));
                                            text_state.store(ui.ctx(), editor_id);
                                            editor.cursor.char_idx = prev_range.primary.index;
                                            editor.selection.anchor =
                                                prev_range.secondary.index;
                                            editor.selection.head = prev_range.primary.index;
                                        }

                                    let mut text_changed = self.process_autoclosing(
                                        ui.ctx(),
                                        edit_res.id,
                                        edit_output.state,
                                        edit_res.changed(),
                                    );
                                    let tab_changed = self.handle_tab_key(ui, editor_id, tab_pressed, shift_tab_pressed, tab_width);
                                    if tab_changed {
                                        text_changed = true;
                                    }

                                    (gutter_rect, edit_res, text_changed)
                                })
                                .inner
                            };

                            // Paint line numbers onto the allocated gutter_rect
                            let painter = ui.painter();
                            let text_color = Color32::from_rgb(100, 100, 100);
                            let font = FontId::monospace(font_size);

                            for (line_num, y_offset) in &line_positions {
                                let text_pos = egui::pos2(
                                    gutter_rect.max.x - 5.0,
                                    edit_res.rect.min.y + y_offset,
                                );
                                painter.text(
                                    text_pos,
                                    egui::Align2::RIGHT_TOP,
                                    line_num.to_string(),
                                    font.clone(),
                                    text_color,
                                );
                            }

                            // Add bottom padding inside scroll viewport
                            ui.add_space(100.0);

                            (edit_res, text_changed)
                        })
                        .inner
                });

            if output.inner.0.changed() || output.inner.1 {
                self.sync_to_editor(editor, ui.ctx());
                if self.find_visible {
                    self.update_find_matches();
                }
            }
        } else {
            let output = egui::ScrollArea::vertical()
                .id_source("editor_scroll")
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    self.handle_drag_autoscroll(ui, editor);

                    egui::Frame::none()
                        .inner_margin(egui::Margin::symmetric(24.0, 8.0))
                        .show(ui, |ui| {
                            let available_width = ui.available_width();
                            let text_wrap_width = (available_width - 8.0).max(100.0);
                            let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                                let default_color = ui.style().visuals.text_color();
                                let link_color = ui.style().visuals.hyperlink_color;
                                let mut job =
                                    create_layout_job(text, font_size, default_color, link_color);
                                job.wrap.max_width = wrap_width.min(text_wrap_width);
                                if find_visible && !byte_matches.is_empty() {
                                    highlight_matches(&mut job, &byte_matches, active_match_idx);
                                }
                                ui.fonts(|f| f.layout_job(job))
                            };

                            let editor_id = get_editor_id(editor);
                            let is_menu_open = ui.memory(|m| m.is_popup_open(editor_id));

                            let text_edit = egui::TextEdit::multiline(&mut self.content_buffer)
                                .id(editor_id)
                                .font(FontId::monospace(font_size))
                                .frame(false)
                                .layouter(&mut layouter)
                                .code_editor()
                                .desired_width(f32::INFINITY);
                            let edit_output = text_edit.show(ui);
                            let edit_res = edit_output.response.clone();
                            if self.scroll_to_cursor_requested
                                && let Some(active_idx) = self.active_match_index
                                    && let Some(range) = self.matches.get(active_idx) {
                                        let local_rect = edit_output.galley.pos_from_ccursor(
                                            egui::text::CCursor::new(range.start),
                                        );
                                        let screen_rect =
                                            local_rect.translate(edit_res.rect.min.to_vec2());
                                        ui.scroll_to_rect(screen_rect, Some(egui::Align::Center));
                                        self.scroll_to_cursor_requested = false;
                                    }

                            self.update_cursor_screen_pos(editor, &edit_output);

                            let content_buf = &mut self.content_buffer;
                            edit_res.clone().context_menu(|ui| {
                                Self::render_context_menu(ui, editor, content_buf);
                            });

                            self.draw_custom_cursor(
                                ui,
                                editor,
                                &edit_output,
                                cursor_style,
                                font_size,
                            );

                            if (is_right_click || is_menu_open)
                                && let Some(prev_range) = previous_selection
                                    && let Some(mut text_state) =
                                        egui::widgets::text_edit::TextEditState::load(
                                            ui.ctx(),
                                            editor_id,
                                        )
                                    {
                                        text_state.cursor.set_char_range(Some(prev_range));
                                        text_state.store(ui.ctx(), editor_id);
                                        editor.cursor.char_idx = prev_range.primary.index;
                                        editor.selection.anchor = prev_range.secondary.index;
                                        editor.selection.head = prev_range.primary.index;
                                    }

                            let mut text_changed = self.process_autoclosing(
                                ui.ctx(),
                                edit_res.id,
                                edit_output.state,
                                edit_res.changed(),
                            );
                            let tab_changed = self.handle_tab_key(ui, editor_id, tab_pressed, shift_tab_pressed, tab_width);
                            if tab_changed {
                                text_changed = true;
                            }

                            // Add bottom padding inside scroll viewport
                            ui.add_space(100.0);

                            (edit_res, text_changed)
                        })
                        .inner
                });

            if output.inner.0.changed() || output.inner.1 {
                self.sync_to_editor(editor, ui.ctx());
                if self.find_visible {
                    self.update_find_matches();
                }
            }
        }
    }

    fn sync_cursor(ctx: &egui::Context, editor: &mut Editor) {
        if let Some(text_state) =
            egui::widgets::text_edit::TextEditState::load(ctx, get_editor_id(editor))
            && let Some(range) = text_state.cursor.char_range() {
                editor.cursor.char_idx = range.primary.index;
                editor.selection.anchor = range.secondary.index;
                editor.selection.head = range.primary.index;
            }
    }

    fn render_context_menu(ui: &mut egui::Ui, editor: &mut Editor, content_buffer: &mut String) {
        if ui.button("Cut").clicked() {
            if let Some(mut text_state) =
                egui::widgets::text_edit::TextEditState::load(ui.ctx(), get_editor_id(editor)) {
                    let range = text_state.cursor.char_range().or_else(|| {
                        editor.selection.range().map(|r| {
                            let start = egui::text::CCursor::new(r.start);
                            let end = egui::text::CCursor::new(r.end);
                            egui::text::CCursorRange::two(start, end)
                        })
                    });
                    if let Some(range) = range {
                        let start = range.primary.index.min(range.secondary.index);
                        let end = range.primary.index.max(range.secondary.index);
                        let sorted = start..end;
                        if !sorted.is_empty() {
                            let text_to_copy = editor.buffer.rope.slice(sorted.clone()).to_string();
                            ui.ctx().copy_text(text_to_copy.clone());
                            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                let _ = clipboard.set_text(text_to_copy);
                            }

                            editor.buffer.remove(sorted.start, sorted.end);
                            editor.cursor.char_idx = sorted.start;
                            editor.selection.clear(sorted.start);
                            editor.is_dirty = true;

                            *content_buffer = editor.buffer.to_string();

                            let cursor = egui::text::CCursor::new(sorted.start);
                            text_state
                                .cursor
                                .set_char_range(Some(egui::text::CCursorRange::two(cursor, cursor)));
                            text_state.store(ui.ctx(), get_editor_id(editor));
                        }
                    }
                }
            ui.close_menu();
        }
        if ui.button("Copy").clicked() {
            let text_state = egui::widgets::text_edit::TextEditState::load(ui.ctx(), get_editor_id(editor));
            let range = text_state.as_ref().and_then(|ts| ts.cursor.char_range()).or_else(|| {
                editor.selection.range().map(|r| {
                    let start = egui::text::CCursor::new(r.start);
                    let end = egui::text::CCursor::new(r.end);
                    egui::text::CCursorRange::two(start, end)
                })
            });
            if let Some(range) = range {
                let start = range.primary.index.min(range.secondary.index);
                let end = range.primary.index.max(range.secondary.index);
                let sorted = start..end;
                if !sorted.is_empty() {
                    let text_to_copy = editor.buffer.rope.slice(sorted).to_string();
                    ui.ctx().copy_text(text_to_copy.clone());
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        let _ = clipboard.set_text(text_to_copy);
                    }
                }
            }
            if let Some(mut ts) = egui::widgets::text_edit::TextEditState::load(ui.ctx(), get_editor_id(editor)) {
                let cursor = egui::text::CCursor::new(editor.cursor.char_idx);
                ts.cursor.set_char_range(Some(egui::text::CCursorRange::two(cursor, cursor)));
                ts.store(ui.ctx(), get_editor_id(editor));
            }
            editor.selection.clear(editor.cursor.char_idx);
            ui.close_menu();
        }
        if ui.button("Paste").clicked() {
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                let paste_text = clipboard.get_text().unwrap_or_default();
                if !paste_text.is_empty()
                    && let Some(mut text_state) = egui::widgets::text_edit::TextEditState::load(
                        ui.ctx(),
                        get_editor_id(editor),
                    ) {
                        let range = text_state.cursor.char_range().unwrap_or_else(|| {
                            let len = editor.buffer.len_chars();
                            let cursor = egui::text::CCursor::new(len);
                            egui::text::CCursorRange::two(cursor, cursor)
                        });
                        let start = range.primary.index.min(range.secondary.index);
                        let end = range.primary.index.max(range.secondary.index);
                        let sorted = start..end;

                        if !sorted.is_empty() {
                            editor.buffer.remove(sorted.start, sorted.end);
                        }

                        editor.buffer.insert(sorted.start, &paste_text);
                        editor.cursor.char_idx = sorted.start + paste_text.chars().count();
                        editor.selection.clear(editor.cursor.char_idx);
                        editor.is_dirty = true;

                        *content_buffer = editor.buffer.to_string();

                        let cursor = egui::text::CCursor::new(editor.cursor.char_idx);
                        text_state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::two(cursor, cursor)));
                        text_state.store(ui.ctx(), get_editor_id(editor));
                    }
            }
            ui.close_menu();
        }
        ui.separator();
        // Single Line/Word Operations
        if ui.button("Bold").clicked() {
            Self::sync_cursor(ui.ctx(), editor);
            editor.format_selection("bold");
            ui.close_menu();
        }
        if ui.button("Italic").clicked() {
            Self::sync_cursor(ui.ctx(), editor);
            editor.format_selection("italic");
            ui.close_menu();
        }
        if ui.button("Link").clicked() {
            Self::sync_cursor(ui.ctx(), editor);
            editor.format_selection("link");
            ui.close_menu();
        }
        if ui.button("Comment").clicked() {
            Self::sync_cursor(ui.ctx(), editor);
            editor.format_selection("comment");
            ui.close_menu();
        }
        ui.separator();
        // Paragraph Operations
        if ui.button("Code Block").clicked() {
            Self::sync_cursor(ui.ctx(), editor);
            editor.format_selection("code");
            ui.close_menu();
        }
        if ui.button("Checkbox").clicked() {
            Self::sync_cursor(ui.ctx(), editor);
            editor.format_selection("checkbox");
            ui.close_menu();
        }
        if ui.button("Table").clicked() {
            Self::sync_cursor(ui.ctx(), editor);
            editor.request_table_dialog = true;
            ui.close_menu();
        }
        if ui.button("Numbered List").clicked() {
            Self::sync_cursor(ui.ctx(), editor);
            editor.format_selection("numbered_list");
            ui.close_menu();
        }
        if ui.button("Bulleted List").clicked() {
            Self::sync_cursor(ui.ctx(), editor);
            editor.format_selection("bulleted_list");
            ui.close_menu();
        }
        if ui.button("Blockquote").clicked() {
            Self::sync_cursor(ui.ctx(), editor);
            editor.format_selection("indent");
            ui.close_menu();
        }
    }

    fn handle_drag_autoscroll(&self, ui: &mut egui::Ui, editor: &Editor) {
        let is_focused = ui.memory(|mem| mem.has_focus(get_editor_id(editor)));
        let pointer = ui.input(|i| i.pointer.clone());
        if is_focused && pointer.primary_down()
            && let Some(pos) = pointer.latest_pos() {
                let clip_rect = ui.clip_rect();
                if pos.y < clip_rect.min.y {
                    let delta = ((clip_rect.min.y - pos.y) * 0.15).min(10.0);
                    let target_rect = egui::Rect::from_min_max(
                        egui::pos2(clip_rect.min.x, clip_rect.min.y - delta - 2.0),
                        egui::pos2(clip_rect.max.x, clip_rect.min.y),
                    );
                    ui.scroll_to_rect(target_rect, Some(egui::Align::TOP));
                    ui.ctx().request_repaint();
                } else if pos.y > clip_rect.max.y {
                    let delta = ((pos.y - clip_rect.max.y) * 0.15).min(10.0);
                    let target_rect = egui::Rect::from_min_max(
                        egui::pos2(clip_rect.min.x, clip_rect.max.y),
                        egui::pos2(clip_rect.max.x, clip_rect.max.y + delta + 2.0),
                    );
                    ui.scroll_to_rect(target_rect, Some(egui::Align::BOTTOM));
                    ui.ctx().request_repaint();
                }
            }
    }

    fn update_cursor_screen_pos(
        &self,
        editor: &mut Editor,
        edit_output: &egui::text_edit::TextEditOutput,
    ) {
        let mut cursor_screen_pos = None;
        if let Some(range) = edit_output.state.cursor.char_range() {
            let ccursor = range.primary;
            let local_rect = edit_output.galley.pos_from_ccursor(ccursor);
            let line_height = local_rect.height();
            cursor_screen_pos = Some(
                edit_output.response.rect.min
                    + local_rect.min.to_vec2()
                    + egui::vec2(0.0, line_height),
            );
        }
        editor.cursor_screen_pos = cursor_screen_pos;
    }

    fn draw_custom_cursor(
        &self,
        ui: &mut egui::Ui,
        editor: &Editor,
        edit_output: &egui::text_edit::TextEditOutput,
        cursor_style: crate::config::CursorStyle,
        font_size: f32,
    ) {
        let is_focused = ui.memory(|mem| mem.has_focus(edit_output.response.id));
        if is_focused && cursor_style != crate::config::CursorStyle::IBeam
            && let Some(range) = edit_output.state.cursor.char_range()
                && range.primary.index == range.secondary.index {
                    let ccursor = range.primary;
                    let pos_start = edit_output.galley.pos_from_ccursor(ccursor);
                    let pos_end = edit_output
                        .galley
                        .pos_from_ccursor(egui::text::CCursor::new(ccursor.index + 1));
                    let char_width = if pos_end.min.y == pos_start.min.y {
                        (pos_end.min.x - pos_start.min.x).max(6.0)
                    } else {
                        font_size * 0.6
                    };
                    let accent_color = ui.visuals().hyperlink_color;
                    let rect = match cursor_style {
                        crate::config::CursorStyle::Block => egui::Rect::from_min_max(
                            pos_start.min,
                            egui::pos2(pos_start.min.x + char_width, pos_start.max.y),
                        ),
                        crate::config::CursorStyle::Underline => egui::Rect::from_min_max(
                            egui::pos2(pos_start.min.x, pos_start.max.y - 2.0),
                            egui::pos2(pos_start.min.x + char_width, pos_start.max.y),
                        ),
                        _ => pos_start,
                    };
                    let color = match cursor_style {
                        crate::config::CursorStyle::Block => accent_color,
                        _ => accent_color,
                    };
                    let screen_rect = rect.translate(edit_output.galley_pos.to_vec2());
                    ui.painter().rect_filled(screen_rect, 0.0, color);

                    if cursor_style == crate::config::CursorStyle::Block
                        && let Some(c) = editor.buffer.rope.get_char(ccursor.index)
                            && c != '\n' && c != '\r' && c != '\t' {
                                let text_color = ui.visuals().extreme_bg_color;
                                let font_id = egui::FontId::monospace(font_size);
                                ui.painter().text(
                                    screen_rect.min,
                                    egui::Align2::LEFT_TOP,
                                    c.to_string(),
                                    font_id,
                                    text_color,
                                );
                            }
                }
    }
}

fn create_layout_job(
    text: &str,
    font_size: f32,
    text_color: Color32,
    link_color: Color32,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    let normal_font = FontId::monospace(font_size);
    let heading_font = FontId::monospace(font_size);

    let default_color = text_color;
    let header_color = link_color;

    // Check if the theme is light by looking at text color lightness
    let is_light_theme = text_color.r() < 128;
    let code_color = if is_light_theme {
        Color32::from_rgb(0, 120, 0)
    } else {
        Color32::from_rgb(140, 220, 140)
    };
    let blockquote_color = if is_light_theme {
        Color32::from_rgb(80, 80, 80)
    } else {
        Color32::from_rgb(140, 140, 140)
    };

    let link_format = TextFormat {
        font_id: normal_font.clone(),
        color: link_color,
        underline: egui::Stroke::new(1.0_f32, link_color),
        ..Default::default()
    };

    let mut in_code_block = false;
    let mut lines = text.split('\n').peekable();

    while let Some(line) = lines.next() {
        let mut line_format = TextFormat {
            font_id: normal_font.clone(),
            color: default_color,
            ..Default::default()
        };

        let trimmed = line.trim();
        let is_fence = trimmed.starts_with("```");
        if is_fence {
            in_code_block = !in_code_block;
            line_format.color = code_color;
            job.append(line, 0.0, line_format);
        } else if in_code_block {
            line_format.color = code_color;
            job.append(line, 0.0, line_format);
        } else {
            if trimmed.starts_with('#') {
                line_format.font_id = heading_font.clone();
                line_format.color = header_color;
            } else if trimmed.starts_with('>') {
                line_format.color = blockquote_color;
            }

            if !line.contains('[') {
                job.append(line, 0.0, line_format);
            } else {
                let bytes = line.as_bytes();
                let len = bytes.len();
                let mut idx = 0;

                while idx < len {
                    let mut matched = false;

                    // Wiki link: [[path]] or [[path|label]]
                    if idx + 3 < len && &bytes[idx..idx + 2] == b"[[" {
                        if let Some(rel_end) = line[idx + 2..].find("]]") {
                            let end_pos = idx + 2 + rel_end + 2;
                            job.append(&line[idx..end_pos], 0.0, link_format.clone());
                            idx = end_pos;
                            matched = true;
                        }
                    }

                    // Standard markdown link: [label](url)
                    if !matched && bytes[idx] == b'[' {
                        if let Some(rel_close_bracket) = line[idx + 1..].find(']') {
                            let close_bracket = idx + 1 + rel_close_bracket;
                            if close_bracket + 1 < len && bytes[close_bracket + 1] == b'(' {
                                if let Some(rel_close_paren) = line[close_bracket + 2..].find(')') {
                                    let close_paren = close_bracket + 2 + rel_close_paren;
                                    let end_pos = close_paren + 1;
                                    job.append(&line[idx..end_pos], 0.0, link_format.clone());
                                    idx = end_pos;
                                    matched = true;
                                }
                            }
                        }
                    }

                    if matched {
                        continue;
                    }

                    let start_run = idx;
                    idx += 1;
                    while idx < len && bytes[idx] != b'[' {
                        idx += 1;
                    }
                    job.append(&line[start_run..idx], 0.0, line_format.clone());
                }
            }
        }

        if lines.peek().is_some() {
            job.append(
                "\n",
                0.0,
                TextFormat {
                    font_id: normal_font.clone(),
                    color: default_color,
                    ..Default::default()
                },
            );
        }
    }
    job
}



fn highlight_matches(
    job: &mut LayoutJob,
    matches: &[std::ops::Range<usize>],
    active_match_idx: Option<usize>,
) {
    if matches.is_empty() {
        return;
    }

    let mut new_sections = Vec::new();

    for section in &job.sections {
        let mut current_start = section.byte_range.start;
        let section_end = section.byte_range.end;

        // Find all matches that intersect with this section
        for (idx, m) in matches.iter().enumerate() {
            if m.end <= current_start {
                continue;
            }
            if m.start >= section_end {
                break;
            }

            // 1. Part before the match
            if m.start > current_start {
                new_sections.push(egui::text::LayoutSection {
                    leading_space: if current_start == section.byte_range.start {
                        section.leading_space
                    } else {
                        0.0
                    },
                    byte_range: current_start..m.start,
                    format: section.format.clone(),
                });
                current_start = m.start;
            }

            // 2. Overlapping part (inside match)
            let overlap_start = current_start;
            let overlap_end = m.end.min(section_end);
            if overlap_start < overlap_end {
                let mut format = section.format.clone();
                let is_active = Some(idx) == active_match_idx;
                format.background = if is_active {
                    Color32::from_rgba_unmultiplied(255, 99, 71, 140) // Tomato/Orange for active match
                } else {
                    Color32::from_rgba_unmultiplied(255, 215, 0, 90) // Gold/Yellow for other matches
                };
                new_sections.push(egui::text::LayoutSection {
                    leading_space: if current_start == section.byte_range.start {
                        section.leading_space
                    } else {
                        0.0
                    },
                    byte_range: overlap_start..overlap_end,
                    format,
                });
                current_start = overlap_end;
            }
        }

        // 3. Remaining part after all intersecting matches
        if current_start < section_end {
            new_sections.push(egui::text::LayoutSection {
                leading_space: if current_start == section.byte_range.start {
                    section.leading_space
                } else {
                    0.0
                },
                byte_range: current_start..section_end,
                format: section.format.clone(),
            });
        }
    }

    job.sections = new_sections;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_autoclosing_pairs() {
        let pairs = [
            ("{}", "{\n\n}"),
            ("()", "(\n\n)"),
            ("[]", "[\n\n]"),
            ("\"\"", "\"\n\n\""),
            ("''", "'\n\n'"),
            ("``", "`\n\n`"),
        ];

        for (input_pair, expected_output) in pairs {
            let mut renderer = EditorRenderer::new();
            // Set up the state before enter: content_buffer has the pair and cursor is between them (index 1)
            renderer.content_buffer = input_pair.to_string();
            renderer.previous_text = input_pair.to_string();

            let ccursor = egui::text::CCursor::new(1);
            renderer.previous_cursor = Some(egui::text::CCursorRange::two(ccursor, ccursor));

            // Mock state for typing a newline:
            // content_buffer becomes "{left}\n{right}" and cursor is at index 2.
            let left_char = input_pair.chars().next().unwrap();
            let right_char = input_pair.chars().nth(1).unwrap();
            renderer.content_buffer = format!("{}\n{}", left_char, right_char);

            let ctx = egui::Context::default();
            let id = egui::Id::new("test_editor");
            let mut state = egui::widgets::text_edit::TextEditState::default();
            let new_ccursor = egui::text::CCursor::new(2);
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    new_ccursor,
                    new_ccursor,
                )));

            let changed = renderer.process_autoclosing(&ctx, id, state, true);

            assert!(changed, "Failed for pair: {}", input_pair);
            assert_eq!(
                renderer.content_buffer, expected_output,
                "Incorrect content buffer for pair: {}",
                input_pair
            );

            // Load stored state from context to verify cursor position
            let updated_state = egui::widgets::text_edit::TextEditState::load(&ctx, id).unwrap();
            let updated_range = updated_state.cursor.char_range().unwrap();
            assert_eq!(
                updated_range.primary.index, 2,
                "Incorrect primary cursor index for pair: {}",
                input_pair
            );
            assert_eq!(
                updated_range.secondary.index, 2,
                "Incorrect secondary cursor index for pair: {}",
                input_pair
            );
        }
    }

    #[test]
    fn test_autoclosing_before_word_suppression() {
        let mut renderer = EditorRenderer::new();
        // User typed '`' when cursor was right before 'b' in "bash"
        renderer.previous_text = "bash".to_string();
        renderer.content_buffer = "`bash".to_string();
        let prev_cc = egui::text::CCursor::new(0);
        renderer.previous_cursor = Some(egui::text::CCursorRange::two(prev_cc, prev_cc));

        let ctx = egui::Context::default();
        let id = egui::Id::new("test_editor");
        let mut state = egui::widgets::text_edit::TextEditState::default();
        let curr_cc = egui::text::CCursor::new(1);
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(curr_cc, curr_cc)));

        let changed = renderer.process_autoclosing(&ctx, id, state, true);
        assert!(!changed, "Should not insert closing backtick when typing before a word");
        assert_eq!(renderer.content_buffer, "`bash");
    }

    #[test]
    fn test_autoclosing_apostrophe_suppression() {
        let mut renderer = EditorRenderer::new();
        // User typed '\'' after 'n' in "don"
        renderer.previous_text = "don".to_string();
        renderer.content_buffer = "don'".to_string();
        let prev_cc = egui::text::CCursor::new(3);
        renderer.previous_cursor = Some(egui::text::CCursorRange::two(prev_cc, prev_cc));

        let ctx = egui::Context::default();
        let id = egui::Id::new("test_editor");
        let mut state = egui::widgets::text_edit::TextEditState::default();
        let curr_cc = egui::text::CCursor::new(4);
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(curr_cc, curr_cc)));

        let changed = renderer.process_autoclosing(&ctx, id, state, true);
        assert!(!changed, "Should not auto-close single quote when typing after a word (apostrophe)");
        assert_eq!(renderer.content_buffer, "don'");
    }

    #[test]
    fn test_autoclosing_paste_suppression() {
        let mut renderer = EditorRenderer::new();
        // User pasted "[example]" into an empty editor
        renderer.previous_text = "".to_string();
        renderer.content_buffer = "[example]".to_string();
        let prev_cc = egui::text::CCursor::new(0);
        renderer.previous_cursor = Some(egui::text::CCursorRange::two(prev_cc, prev_cc));

        let ctx = egui::Context::default();
        let id = egui::Id::new("test_editor");
        let mut state = egui::widgets::text_edit::TextEditState::default();
        let curr_cc = egui::text::CCursor::new(9);
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(curr_cc, curr_cc)));

        let changed = renderer.process_autoclosing(&ctx, id, state, true);
        assert!(!changed, "Pasting text should not trigger autoclosing");
        assert_eq!(renderer.content_buffer, "[example]");
    }

    #[test]
    fn test_find_in_file_string_and_regex() {
        let mut renderer = EditorRenderer::new();
        renderer.content_buffer =
            "Hello world! This is a test file for search. Hello again!".to_string();

        // 1. Literal string search, case insensitive (default)
        renderer.find_query = "hello".to_string();
        renderer.case_sensitive = false;
        renderer.use_regex = false;
        renderer.update_find_matches();
        assert_eq!(renderer.matches.len(), 2);
        assert_eq!(renderer.matches[0], 0..5); // "Hello"
        assert_eq!(renderer.matches[1], 45..50); // "Hello"
        assert_eq!(renderer.active_match_index, Some(0));

        // 2. Literal string search, case sensitive
        renderer.find_query = "Hello".to_string();
        renderer.case_sensitive = true;
        renderer.update_find_matches();
        assert_eq!(renderer.matches.len(), 2);
        assert_eq!(renderer.matches[0], 0..5); // "Hello"
        assert_eq!(renderer.matches[1], 45..50); // "Hello"
        assert_eq!(renderer.active_match_index, Some(0));

        // 3. Regex search, case insensitive
        renderer.find_query = "th.s".to_string();
        renderer.case_sensitive = false;
        renderer.use_regex = true;
        renderer.update_find_matches();
        assert_eq!(renderer.matches.len(), 1);
        assert_eq!(renderer.matches[0], 13..17); // "This"
        assert_eq!(renderer.active_match_index, Some(0));

        // 4. Regex search, case sensitive
        renderer.case_sensitive = true;
        renderer.update_find_matches();
        assert_eq!(renderer.matches.len(), 0);
        assert_eq!(renderer.active_match_index, None);

        // 5. Navigate matches
        renderer.find_query = "world".to_string();
        renderer.case_sensitive = true;
        renderer.use_regex = false;
        renderer.update_find_matches();
        assert_eq!(renderer.matches.len(), 1);

        renderer.find_query = "Hello".to_string();
        renderer.case_sensitive = false;
        renderer.update_find_matches();
        assert_eq!(renderer.matches.len(), 2);
        assert_eq!(renderer.active_match_index, Some(0));

        let mut editor = Editor::new();
        editor.buffer = crate::editor::EditBuffer::from_str(&renderer.content_buffer);

        let ctx = egui::Context::default();
        renderer.navigate_match(true, &mut editor, &ctx);
        assert_eq!(renderer.active_match_index, Some(1));
        assert_eq!(editor.selection.anchor, 45);
        assert_eq!(editor.selection.head, 50);

        renderer.navigate_match(true, &mut editor, &ctx);
        assert_eq!(renderer.active_match_index, Some(0));
        assert_eq!(editor.selection.anchor, 0);
        assert_eq!(editor.selection.head, 5);

        renderer.navigate_match(false, &mut editor, &ctx);
        assert_eq!(renderer.active_match_index, Some(1));
        assert_eq!(editor.selection.anchor, 45);
        assert_eq!(editor.selection.head, 50);

        // 6. Replace current match
        renderer.replace_query = "Hi".to_string();
        // Currently active match is matches[1] which is "Hello" at 45..50
        renderer.replace_current(&mut editor, &ctx);
        // String should become: "Hello world! This is a test file for search. Hi again!"
        assert_eq!(
            renderer.content_buffer,
            "Hello world! This is a test file for search. Hi again!"
        );
        // Since we replaced the match, we should have 1 match remaining (the "Hello" at the beginning)
        assert_eq!(renderer.matches.len(), 1);
        assert_eq!(renderer.matches[0], 0..5); // "Hello" at index 0

        // 7. Replace all matches
        renderer.find_query = "a".to_string();
        renderer.replace_query = "x".to_string();
        renderer.case_sensitive = true;
        renderer.use_regex = false;
        renderer.update_find_matches(); // matches "a" in "a test", "search", "again"
        renderer.replace_all(&mut editor);
        // check that all 'a' characters are replaced with 'x'
        // "Hello world! This is a test file for search. Hi again!" -> "Hello world! This is x test file for sexrch. Hi xgxin!"
        assert_eq!(
            renderer.content_buffer,
            "Hello world! This is x test file for sexrch. Hi xgxin!"
        );
    }
}
