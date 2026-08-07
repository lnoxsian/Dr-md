use crate::editor::Editor;
use crate::editor::renderer::EditorRenderer;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

#[derive(Clone, Debug)]
pub struct ZoomModalState {
    pub uri: String,
    pub bytes: std::sync::Arc<[u8]>,
    pub zoom: f32,
    pub scroll_pos: egui::Vec2,
}

pub struct MarkdownPreview {
    pub cache: CommonMarkCache,
    pub cached_content: String,
    pub processed_content: String,
    pub last_version: usize,
    pub last_path: Option<std::path::PathBuf>,
    pub last_content_height: f32,
    pub last_viewport_height: f32,
    pub last_cursor_char_idx: Option<usize>,
    pub scroll_target_y: Option<f32>,
    pub active_zoom: Option<ZoomModalState>,
}

impl MarkdownPreview {
    pub fn new() -> Self {
        Self {
            cache: CommonMarkCache::default(),
            cached_content: String::new(),
            processed_content: String::new(),
            last_version: usize::MAX,
            last_path: None,
            last_content_height: 0.0,
            last_viewport_height: 0.0,
            last_cursor_char_idx: None,
            scroll_target_y: None,
            active_zoom: None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        editor: &mut Editor,
        editor_renderer: &mut EditorRenderer,
        tab_path: &std::path::Path,
        font_size: f32,
        theme: &str,
        mirror_mode: bool,
        is_preview_mode: bool,
        preview_max_width: f32,
    ) {


        if let Some((uri, bytes)) = self.cache.zoomed_image_request.take() {
            self.active_zoom = Some(ZoomModalState {
                uri,
                bytes,
                zoom: 1.0,
                scroll_pos: egui::Vec2::ZERO,
            });
        }

        if is_preview_mode {
            editor_renderer.show_find_panel(ui, editor, false);
        }

        let path_changed = self.last_path.as_deref() != Some(tab_path);
        if path_changed || editor.version != self.last_version {
            if path_changed {
                self.active_zoom = None;
            }
            self.cached_content = editor.buffer.to_string();
            self.processed_content = super::parser::preprocess_wiki_links(&self.cached_content);
            self.last_version = editor.version;
            self.last_path = Some(tab_path.to_path_buf());
        }

        let mut processed = self.processed_content.clone();
        let processed_old = processed.clone();

        let cursor_idx = editor.cursor.char_idx;
        if mirror_mode && self.last_cursor_char_idx != Some(cursor_idx) {
            self.last_cursor_char_idx = Some(cursor_idx);
            let line_idx = editor.buffer.rope.char_to_line(cursor_idx);
            let total_lines = editor.buffer.rope.len_lines();
            let ratio = if total_lines > 1 {
                line_idx as f32 / (total_lines - 1) as f32
            } else {
                0.0
            };
            let max_scroll = (self.last_content_height - self.last_viewport_height).max(0.0);
            let target_y = ratio * self.last_content_height;
            // Center the edit line within the preview's viewport height
            let target_offset = (target_y - self.last_viewport_height / 2.0).clamp(0.0, max_scroll);
            self.scroll_target_y = Some(target_offset);
        } else if !mirror_mode {
            self.last_cursor_char_idx = None;
            self.scroll_target_y = None;
        }

        let mut scroll_area = egui::ScrollArea::vertical()
            .id_source("markdown_preview_scroll")
            .auto_shrink([false; 2]);

        if let Some(target_y) = self.scroll_target_y.take() {
            scroll_area = scroll_area.vertical_scroll_offset(target_y);
        }

        let scroll_output = scroll_area.show(ui, |ui| {
            let available_w = ui.available_width();
            let min_padding = 16.0;
            let pct = preview_max_width.clamp(20.0, 100.0);
            let side_padding = if pct < 100.0 {
                let target_w = available_w * (pct / 100.0);
                ((available_w - target_w) / 2.0).max(min_padding)
            } else {
                min_padding
            };
            let content_w = (available_w - side_padding * 2.0).max(10.0);

            egui::Frame::none()
                .inner_margin(egui::Margin::symmetric(side_padding, 16.0))
                .show(ui, |ui| {
                    ui.set_max_width(content_w);
                    let mut style = ui.style().as_ref().clone();
                    style.interaction.selectable_labels = true;
                    let body_font = egui::FontId::new(font_size, egui::FontFamily::Proportional);
                    let heading_font =
                        egui::FontId::new(font_size * 1.4, egui::FontFamily::Proportional);
                    let monospace_font = egui::FontId::new(font_size, egui::FontFamily::Monospace);

                    style.text_styles.insert(egui::TextStyle::Body, body_font);
                    style
                        .text_styles
                        .insert(egui::TextStyle::Heading, heading_font);
                    style
                        .text_styles
                        .insert(egui::TextStyle::Monospace, monospace_font);

                    ui.set_style(style);

                    let mut viewer = CommonMarkViewer::new("markdown_viewer");
                    match theme {
                        "solarized_dark" => {
                            viewer = viewer.syntax_theme_dark("Solarized (dark)");
                        }
                        "solarized_light" => {
                            viewer = viewer.syntax_theme_light("Solarized (light)");
                        }
                        _ => {}
                    }

                    viewer.show_mut(ui, &mut self.cache, &mut processed);

                    // Highlight matching search keywords inside the preview pane
                    if editor_renderer.find_visible && !editor_renderer.find_query.is_empty() {
                        let mut highlight_rects = Vec::new();
                        ui.painter().for_each_shape(|clipped_shape| {
                            if let egui::Shape::Text(text_shape) = &clipped_shape.shape {
                                let text = &text_shape.galley.job.text;
                                if text.is_empty() {
                                    return;
                                }

                                let matches = editor_renderer.find_matches_in_text(text);

                                for range in matches {
                                    let pos_start = text_shape
                                        .galley
                                        .pos_from_ccursor(egui::text::CCursor::new(range.start));
                                    let pos_end = text_shape
                                        .galley
                                        .pos_from_ccursor(egui::text::CCursor::new(range.end));

                                    if pos_start.min.y == pos_end.min.y {
                                        let local_rect = egui::Rect::from_min_max(
                                            pos_start.min,
                                            egui::pos2(pos_end.min.x, pos_start.max.y),
                                        );
                                        let screen_rect =
                                            local_rect.translate(text_shape.pos.to_vec2());
                                        highlight_rects.push(screen_rect);
                                    } else {
                                        let local_rect = egui::Rect::from_min_max(
                                            pos_start.min,
                                            egui::pos2(
                                                text_shape.galley.rect.max.x,
                                                pos_start.max.y,
                                            ),
                                        );
                                        let screen_rect =
                                            local_rect.translate(text_shape.pos.to_vec2());
                                        highlight_rects.push(screen_rect);
                                    }
                                }
                            }
                        });

                        let highlight_color =
                            egui::Color32::from_rgba_unmultiplied(255, 220, 0, 90);
                        for rect in highlight_rects {
                            ui.painter().rect_filled(
                                rect,
                                egui::Rounding::same(2.0),
                                highlight_color,
                            );
                        }
                    }

                    // Add bottom padding inside scroll viewport
                    ui.add_space(100.0);
                });
        });

        self.last_content_height = scroll_output.content_size.y;
        self.last_viewport_height = scroll_output.inner_rect.height();

        if processed != processed_old {
            // Apply checkbox changes back to content line by line
            let old_lines: Vec<&str> = processed_old.lines().collect();
            let new_lines: Vec<&str> = processed.lines().collect();
            let mut orig_lines: Vec<String> =
                self.cached_content.lines().map(|s| s.to_string()).collect();

            if old_lines.len() == new_lines.len() && old_lines.len() == orig_lines.len() {
                for i in 0..old_lines.len() {
                    if old_lines[i] != new_lines[i] {
                        let old_trimmed = old_lines[i].trim_start();
                        let new_trimmed = new_lines[i].trim_start();

                        let is_toggle = (old_trimmed.starts_with("- [ ]")
                            && new_trimmed.starts_with("- [x]"))
                            || (old_trimmed.starts_with("- [x]")
                                && new_trimmed.starts_with("- [ ]"))
                            || (old_trimmed.starts_with("* [ ]")
                                && new_trimmed.starts_with("* [x]"))
                            || (old_trimmed.starts_with("* [x]")
                                && new_trimmed.starts_with("* [ ]"));

                        if is_toggle {
                            if let Some(pos) = orig_lines[i].find("- [") {
                                let current_char = orig_lines[i]
                                    .as_bytes()
                                    .get(pos + 3)
                                    .copied()
                                    .unwrap_or(b' ');
                                let new_char = if current_char == b'x' { ' ' } else { 'x' };
                                let mut orig_chars: Vec<char> = orig_lines[i].chars().collect();
                                let mut char_pos = 0;
                                for (c_idx, _) in orig_lines[i].char_indices() {
                                    if c_idx == pos + 3 {
                                        break;
                                    }
                                    char_pos += 1;
                                }
                                if char_pos < orig_chars.len() {
                                    orig_chars[char_pos] = new_char;
                                    orig_lines[i] = orig_chars.into_iter().collect();
                                }
                            } else if let Some(pos) = orig_lines[i].find("* [") {
                                let current_char = orig_lines[i]
                                    .as_bytes()
                                    .get(pos + 3)
                                    .copied()
                                    .unwrap_or(b' ');
                                let new_char = if current_char == b'x' { ' ' } else { 'x' };
                                let mut orig_chars: Vec<char> = orig_lines[i].chars().collect();
                                let mut char_pos = 0;
                                for (c_idx, _) in orig_lines[i].char_indices() {
                                    if c_idx == pos + 3 {
                                        break;
                                    }
                                    char_pos += 1;
                                }
                                if char_pos < orig_chars.len() {
                                    orig_chars[char_pos] = new_char;
                                    orig_lines[i] = orig_chars.into_iter().collect();
                                }
                            }
                        }
                    }
                }
                self.cached_content = orig_lines.join("\n");
                self.processed_content = super::parser::preprocess_wiki_links(&self.cached_content);
                editor.set_text(&self.cached_content);
                self.last_version = editor.version;
            }
        }



        if let Some(zoom_state) = &mut self.active_zoom {
            let mut close = false;
            let pane_rect = ui.max_rect();

            // Mouse Wheel Zooming (works with scroll wheel or Ctrl + scroll wheel)
            let scroll_y = ui.ctx().input(|i| {
                if i.smooth_scroll_delta.y != 0.0 {
                    i.smooth_scroll_delta.y
                } else {
                    i.raw_scroll_delta.y
                }
            });

            if scroll_y != 0.0 {
                let factor = if scroll_y > 0.0 { 1.15 } else { 0.85 };
                zoom_state.zoom = (zoom_state.zoom * factor).clamp(0.25, 10.0);
            }

            egui::Area::new(egui::Id::new("mermaid_zoom_fullscreen"))
                .order(egui::Order::Foreground)
                .fixed_pos(pane_rect.min)
                .show(ui.ctx(), |ui| {
                    let mut content_ui = ui.child_ui(
                        pane_rect,
                        egui::Layout::top_down(egui::Align::Min),
                    );
                    content_ui.set_clip_rect(pane_rect);

                    egui::Frame::none()
                        .fill(ui.visuals().window_fill)
                        .stroke(ui.visuals().window_stroke)
                        .inner_margin(egui::Margin::same(10.0))
                        .show(&mut content_ui, |ui| {
                            egui::ScrollArea::both()
                                .auto_shrink([false; 2])
                                .horizontal_scroll_offset(zoom_state.scroll_pos.x)
                                .vertical_scroll_offset(zoom_state.scroll_pos.y)
                                .show(ui, |ui| {
                                    let img = egui::Image::from_bytes(
                                        zoom_state.uri.clone(),
                                        zoom_state.bytes.clone(),
                                    )
                                    .fit_to_original_size(zoom_state.zoom);
                                    ui.add(img);
                                });

                            let (primary_down, pointer_delta) = ui.ctx().input(|i| {
                                (i.pointer.primary_down(), i.pointer.delta())
                            });

                            if primary_down && pointer_delta != egui::Vec2::ZERO {
                                zoom_state.scroll_pos -= pointer_delta;
                                zoom_state.scroll_pos.x = zoom_state.scroll_pos.x.max(0.0);
                                zoom_state.scroll_pos.y = zoom_state.scroll_pos.y.max(0.0);
                                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
                            } else {
                                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grab);
                            }
                        });
                });

            if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
                close = true;
            }

            if close {
                self.active_zoom = None;
            }
        }
    }


}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::Editor;
    use crate::editor::renderer::EditorRenderer;

    #[test]
    fn test_preview_search_highlighting() {
        let ctx = egui::Context::default();
        let mut preview = MarkdownPreview::new();
        let mut editor = Editor::new();
        let mut renderer = EditorRenderer::new();

        editor.set_text("Hello world! This is a test file with Hello keywords.");
        renderer.content_buffer = editor.buffer.to_string();
        renderer.find_visible = true;
        renderer.find_query = "Hello".to_string();
        renderer.update_find_matches();

        let _output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                preview.show(
                    ui,
                    &mut editor,
                    &mut renderer,
                    std::path::Path::new("test.md"),
                    14.0,
                    "dark",
                    false,
                    true,
                    1000.0,
                );
            });
        });

        // The preview ran successfully and highlighted matches!
        assert_eq!(renderer.matches.len(), 2);
    }
}
