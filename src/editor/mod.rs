pub mod buffer;
pub mod cursor;
pub mod renderer;
pub mod selection;
pub mod undo;

pub use buffer::EditBuffer;
pub use cursor::Cursor;
pub use selection::Selection;
pub use undo::UndoStack;

use std::path::PathBuf;

pub struct Editor {
    pub buffer: EditBuffer,
    pub cursor: Cursor,
    pub selection: Selection,
    pub undo_stack: UndoStack,
    pub active_path: Option<PathBuf>,
    pub is_dirty: bool,
    pub version: usize,
    pub request_table_dialog: bool,
    pub cursor_screen_pos: Option<egui::Pos2>,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            buffer: EditBuffer::new(),
            cursor: Cursor::default(),
            selection: Selection::default(),
            undo_stack: UndoStack::new(),
            active_path: None,
            is_dirty: false,
            version: 0,
            request_table_dialog: false,
            cursor_screen_pos: None,
        }
    }

    pub fn load_file(&mut self, path: PathBuf) -> anyhow::Result<()> {
        self.buffer.load_file(&path)?;
        self.active_path = Some(path);
        self.cursor = Cursor::default();
        self.selection.clear(0);
        self.undo_stack.clear();
        self.is_dirty = false;
        self.version += 1;
        Ok(())
    }

    pub fn save_file(&mut self) -> anyhow::Result<()> {
        if let Some(ref path) = self.active_path {
            self.buffer.save_file(path)?;
            self.is_dirty = false;
        }
        Ok(())
    }

    pub fn save_as(&mut self, path: PathBuf) -> anyhow::Result<()> {
        self.buffer.save_file(&path)?;
        self.active_path = Some(path);
        self.is_dirty = false;
        Ok(())
    }

    pub fn set_text(&mut self, text: &str) {
        self.buffer = EditBuffer::from_str(text);
        self.cursor = Cursor::default();
        self.selection.clear(0);
        self.undo_stack.clear();
        self.is_dirty = true;
        self.version += 1;
    }

    pub fn sync_text(&mut self, new_text: &str) {
        if self.buffer.rope == new_text {
            return;
        }

        let old_text = self.buffer.to_string();
        self.undo_stack.push_change(old_text, new_text);
        self.buffer = EditBuffer::from_str(new_text);
        self.is_dirty = true;
        self.version += 1;
    }


    pub fn format_selection(&mut self, format_type: &str) {
        let current_text = self.buffer.to_string();
        self.undo_stack.push(current_text);

        if let Some(range) = self.selection.range() {
            let selected_text = self.buffer.rope.slice(range.start..range.end).to_string();
            let new_text = match format_type {
                "bold" => format!("**{}**", selected_text),
                "italic" => format!("*{}*", selected_text),
                "code" => {
                    if selected_text.contains('\n') {
                        format!("```\n{}\n```", selected_text)
                    } else {
                        format!("`{}`", selected_text)
                    }
                }
                "link" => format!("[{}](url)", selected_text),
                "checkbox" => {
                    if selected_text.contains('\n') {
                        let mut lines = Vec::new();
                        for line in selected_text.lines() {
                            if line.trim().is_empty() || line.trim().starts_with("- [ ] ") {
                                lines.push(line.to_string());
                            } else {
                                lines.push(format!("- [ ] {}", line));
                            }
                        }
                        lines.join("\n")
                    } else {
                        if selected_text.trim().is_empty() || selected_text.trim().starts_with("- [ ] ") {
                            selected_text
                        } else {
                            format!("- [ ] {}", selected_text)
                        }
                    }
                }
                "numbered_list" => {
                    if selected_text.contains('\n') {
                        let mut lines = Vec::new();
                        let mut next_num = 1;
                        for line in selected_text.lines() {
                            if line.trim().is_empty() {
                                lines.push(line.to_string());
                            } else {
                                lines.push(format!("{}. {}", next_num, line));
                                next_num += 1;
                            }
                        }
                        lines.join("\n")
                    } else {
                        if selected_text.trim().is_empty() {
                            selected_text
                        } else {
                            format!("1. {}", selected_text)
                        }
                    }
                }
                "bulleted_list" => {
                    if selected_text.contains('\n') {
                        let mut lines = Vec::new();
                        for line in selected_text.lines() {
                            if line.trim().is_empty() {
                                lines.push(line.to_string());
                            } else {
                                lines.push(format!("- {}", line));
                            }
                        }
                        lines.join("\n")
                    } else {
                        if selected_text.trim().is_empty() {
                            selected_text
                        } else {
                            format!("- {}", selected_text)
                        }
                    }
                }
                "indent" => {
                    if selected_text.contains('\n') {
                        let mut lines = Vec::new();
                        for line in selected_text.lines() {
                            if line.trim().is_empty() {
                                lines.push(line.to_string());
                            } else {
                                lines.push(format!("> {}", line));
                            }
                        }
                        lines.join("\n")
                    } else {
                        if selected_text.trim().is_empty() {
                            selected_text
                        } else {
                            format!("> {}", selected_text)
                        }
                    }
                }
                "comment" => {
                    if selected_text.starts_with("<!-- ") && selected_text.ends_with(" -->") {
                        selected_text[5..selected_text.len() - 4].to_string()
                    } else if selected_text.starts_with("<!--") && selected_text.ends_with("-->") {
                        selected_text[4..selected_text.len() - 3].to_string()
                    } else {
                        format!("<!-- {} -->", selected_text)
                    }
                }
                "table" => format!(
                    "| {} | Header 2 |\n| -------- | -------- |\n| Cell 1   | Cell 2   |",
                    selected_text
                ),
                _ => selected_text,
            };

            self.buffer.remove(range.start, range.end);
            self.buffer.insert(range.start, &new_text);
            self.cursor.char_idx = range.start + new_text.chars().count();
            self.selection.clear(self.cursor.char_idx);
        } else {
            let placeholder = match format_type {
                "bold" => "****",
                "italic" => "**",
                "code" => "```\n\n```",
                "link" => "[](url)",
                "checkbox" => "- [ ] ",
                "numbered_list" => "1. ",
                "bulleted_list" => "- ",
                "indent" => "> ",
                "comment" => "<!--  -->",
                "table" => {
                    "| Header 1 | Header 2 |\n| -------- | -------- |\n| Cell 1   | Cell 2   |"
                }
                _ => "",
            };
            self.buffer.insert(self.cursor.char_idx, placeholder);

            let offset = match format_type {
                "bold" => 2,
                "italic" => 1,
                "code" => 4,          // places cursor inside the code block
                "link" => 1,          // inside the bracket
                "checkbox" => 6,      // "- [ ] " has 6 characters
                "numbered_list" => 3, // "1. " has 3 characters
                "bulleted_list" => 2, // "- " has 2 characters
                "indent" => 2,        // "> " has 2 characters
                "comment" => 5,       // "<!-- " has 5 characters
                "table" => 2,         // puts cursor right at "Header 1"
                _ => placeholder.chars().count(),
            };
            self.cursor.char_idx += offset;
            self.selection.clear(self.cursor.char_idx);
        }
        self.is_dirty = true;
        self.version += 1;
    }

    pub fn format_table(&mut self, cols: usize, rows: usize, selected_text: &str) {
        let cols = cols.max(1);
        let rows = rows.max(1);

        let mut table_str = String::new();

        // 1. Headers
        table_str.push('|');
        let first_header = if selected_text.is_empty() {
            "Header 1".to_string()
        } else {
            selected_text.to_string()
        };
        table_str.push_str(&format!(" {} |", first_header));
        for c in 2..=cols {
            table_str.push_str(&format!(" Header {} |", c));
        }
        table_str.push('\n');

        // 2. Separators
        table_str.push('|');
        for _ in 1..=cols {
            table_str.push_str(" -------- |");
        }
        table_str.push('\n');

        // 3. Rows
        for r in 1..=rows {
            table_str.push('|');
            for c in 1..=cols {
                table_str.push_str(&format!(" Cell {}.{} |", r, c));
            }
            if r < rows {
                table_str.push('\n');
            }
        }

        if let Some(range) = self.selection.range() {
            let current_text = self.buffer.to_string();
            self.undo_stack.push(current_text);

            self.buffer.remove(range.start, range.end);
            self.buffer.insert(range.start, &table_str);
            self.cursor.char_idx = range.start + table_str.chars().count();
            self.selection.clear(self.cursor.char_idx);
        } else {
            self.buffer.insert(self.cursor.char_idx, &table_str);
            self.cursor.char_idx += 2;
            self.selection.clear(self.cursor.char_idx);
        }
        self.is_dirty = true;
        self.version += 1;
    }


    pub fn undo(&mut self) {
        let current_text = self.buffer.to_string();
        if let Some(prev) = self.undo_stack.undo(current_text) {
            self.buffer = EditBuffer::from_str(&prev);
            self.cursor.char_idx = self.cursor.char_idx.min(self.buffer.len_chars());
            self.selection.clear(self.cursor.char_idx);
            self.is_dirty = true;
            self.version += 1;
        }
    }

    pub fn redo(&mut self) {
        let current_text = self.buffer.to_string();
        if let Some(next) = self.undo_stack.redo(current_text) {
            self.buffer = EditBuffer::from_str(&next);
            self.cursor.char_idx = self.cursor.char_idx.min(self.buffer.len_chars());
            self.selection.clear(self.cursor.char_idx);
            self.is_dirty = true;
            self.version += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_selection_comment() {
        let mut editor = Editor::new();
        editor.buffer = EditBuffer::from_str("hello world");

        // Test commenting selection
        editor.selection.anchor = 0;
        editor.selection.head = 5; // "hello"
        editor.format_selection("comment");
        assert_eq!(editor.buffer.to_string(), "<!-- hello --> world");

        // Test uncommenting selection
        editor.selection.anchor = 0;
        editor.selection.head = 14; // "<!-- hello -->"
        editor.format_selection("comment");
        assert_eq!(editor.buffer.to_string(), "hello world");

        // Test commenting empty selection (inserts placeholder)
        editor.selection.clear(0);
        editor.cursor.char_idx = 0;
        editor.format_selection("comment");
        assert_eq!(editor.buffer.to_string(), "<!--  -->hello world");
        assert_eq!(editor.cursor.char_idx, 5); // cursor inside comment
    }

    #[test]
    fn test_format_selection_table() {
        let mut editor = Editor::new();
        editor.buffer = EditBuffer::from_str("MyHeader");

        // Test table with selection
        editor.selection.anchor = 0;
        editor.selection.head = 8; // "MyHeader"
        editor.format_selection("table");
        assert_eq!(
            editor.buffer.to_string(),
            "| MyHeader | Header 2 |\n| -------- | -------- |\n| Cell 1   | Cell 2   |"
        );

        // Test table with empty selection (inserts placeholder)
        let mut editor2 = Editor::new();
        editor2.buffer = EditBuffer::from_str("");
        editor2.cursor.char_idx = 0;
        editor2.selection.clear(0);
        editor2.format_selection("table");
        assert_eq!(
            editor2.buffer.to_string(),
            "| Header 1 | Header 2 |\n| -------- | -------- |\n| Cell 1   | Cell 2   |"
        );
        assert_eq!(editor2.cursor.char_idx, 2); // Cursor placed inside "Header 1"

        // Test custom dimensions table
        let mut editor3 = Editor::new();
        editor3.buffer = EditBuffer::from_str("");
        editor3.cursor.char_idx = 0;
        editor3.selection.clear(0);
        editor3.format_table(3, 2, "");
        assert_eq!(
            editor3.buffer.to_string(),
            "| Header 1 | Header 2 | Header 3 |\n| -------- | -------- | -------- |\n| Cell 1.1 | Cell 1.2 | Cell 1.3 |\n| Cell 2.1 | Cell 2.2 | Cell 2.3 |"
        );
        assert_eq!(editor3.cursor.char_idx, 2); // Cursor placed inside "Header 1"
    }
}
