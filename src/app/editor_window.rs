use crate::app::state::{AppState, ViewMode};

pub fn render_editor_window(ctx: &egui::Context, state: &mut AppState) {
    let panel_frame =
        egui::Frame::central_panel(&ctx.style()).inner_margin(egui::Margin::symmetric(0.0, 0.0));
    egui::CentralPanel::default()
        .frame(panel_frame)
        .show(ctx, |ui| {
            if !state.tabs.is_empty() {
                // 1. Render Tab Bar
                crate::app::tab_bar::render_tab_bar(ui, state);

                // 2. Render separator line
                let sep_rect = egui::Rect::from_min_size(
                    egui::pos2(ui.max_rect().min.x, ui.cursor().top()),
                    egui::vec2(ui.available_width(), 1.0),
                );
                ui.painter().rect_filled(
                    sep_rect,
                    0.0,
                    ui.visuals().widgets.noninteractive.bg_stroke.color,
                );
                ui.add_space(1.0);

                // 3. Render Editor / Preview Content inside a child UI filling the rest of the window
                let content_rect = ui.available_rect_before_wrap();
                let mut content_ui =
                    ui.child_ui(content_rect, egui::Layout::top_down(egui::Align::Min));
                content_ui.set_clip_rect(content_rect);

                if let Some(active_idx) = state.active_tab_index {
                    let font_size = state.config.font_size;
                    let line_numbers = state.config.line_numbers;
                    let cursor_style = state.config.cursor_style;
                    let theme = state.config.theme.as_str();

                    let mut new_split_ratio = None;

                    if let Some(tab) = state.tabs.get_mut(active_idx) {
                        match tab.view_mode {
                            ViewMode::Preview => {
                                state.preview.show(
                                    &mut content_ui,
                                    &mut tab.editor,
                                    &mut tab.editor_renderer,
                                    &tab.path,
                                    state.vault.root_path.as_deref(),
                                    font_size,
                                    theme,
                                    state.config.mirror_mode,
                                    true,
                                    state.config.preview_max_width,
                                );
                            }
                            ViewMode::Split => {
                                let min_rect = content_ui.max_rect();
                                // Allocate the entire space so egui knows we are occupying it
                                content_ui.allocate_rect(min_rect, egui::Sense::hover());

                                let available_width = min_rect.width();
                                let available_height = min_rect.height();

                                if available_width < 600.0 {
                                    // Vertical Split (Top-to-Bottom)
                                    let separator_height = 2.0;
                                    let drag_handle_height = 8.0;
                                    let total_height = available_height - separator_height;
                                    let top_height = (total_height * state.split_ratio)
                                        .max(100.0)
                                        .min(total_height - 100.0);
                                    let bottom_height = total_height - top_height;

                                    // Top Rect
                                    let top_rect = egui::Rect::from_min_size(
                                        min_rect.min,
                                        egui::vec2(available_width, top_height),
                                    );

                                    // Separator Rect (visible thin line)
                                    let sep_rect = egui::Rect::from_min_size(
                                        egui::pos2(min_rect.min.x, min_rect.min.y + top_height),
                                        egui::vec2(available_width, separator_height),
                                    );

                                    // Drag Handle Rect (wider click target)
                                    let drag_rect = egui::Rect::from_min_size(
                                        egui::pos2(
                                            min_rect.min.x,
                                            min_rect.min.y + top_height
                                                - (drag_handle_height - separator_height) / 2.0,
                                        ),
                                        egui::vec2(available_width, drag_handle_height),
                                    );

                                    // Bottom Rect
                                    let bottom_rect = egui::Rect::from_min_size(
                                        egui::pos2(
                                            min_rect.min.x,
                                            min_rect.min.y + top_height + separator_height,
                                        ),
                                        egui::vec2(available_width, bottom_height),
                                    );

                                    // Top Pane: Editor
                                    let mut top_ui = content_ui.child_ui(
                                        top_rect,
                                        egui::Layout::top_down(egui::Align::Min),
                                    );
                                    top_ui.set_clip_rect(top_rect);
                                    tab.editor_renderer.show(
                                        &mut top_ui,
                                        &mut tab.editor,
                                        state.vault.root_path.as_deref(),
                                        font_size,
                                        line_numbers,
                                        cursor_style,
                                        state.config.tab_width,
                                    );

                                    // Separator / Drag handle
                                    let sep_response =
                                        content_ui.allocate_rect(drag_rect, egui::Sense::drag());

                                    // Paint separator line
                                    let color = if sep_response.dragged() {
                                        content_ui.visuals().selection.bg_fill
                                    } else if sep_response.hovered() {
                                        content_ui.visuals().selection.bg_fill.linear_multiply(0.8)
                                    } else {
                                        content_ui.visuals().widgets.noninteractive.bg_stroke.color
                                    };
                                    content_ui.painter().rect_filled(sep_rect, 0.0, color);

                                    // Update split_ratio based on drag
                                    if sep_response.dragged()
                                        && let Some(mouse_pos) =
                                            content_ui.input(|i| i.pointer.latest_pos())
                                    {
                                        let relative_y = mouse_pos.y - min_rect.top();
                                        new_split_ratio =
                                            Some((relative_y / available_height).clamp(0.1, 0.9));
                                    }

                                    // Change cursor to vertical resize icon
                                    if sep_response.hovered() || sep_response.dragged() {
                                        content_ui.output_mut(|o| {
                                            o.cursor_icon = egui::CursorIcon::ResizeVertical
                                        });
                                    }

                                    // Bottom Pane: Preview
                                    let mut bottom_ui = content_ui.child_ui(
                                        bottom_rect,
                                        egui::Layout::top_down(egui::Align::Min),
                                    );
                                    bottom_ui.set_clip_rect(bottom_rect);
                                    state.preview.show(
                                        &mut bottom_ui,
                                        &mut tab.editor,
                                        &mut tab.editor_renderer,
                                        &tab.path,
                                        state.vault.root_path.as_deref(),
                                        font_size,
                                        theme,
                                        state.config.mirror_mode,
                                        false,
                                        state.config.preview_max_width,
                                    );
                                } else {
                                    // Horizontal Split (Side-by-Side)
                                    let separator_width = 2.0;
                                    let drag_handle_width = 8.0;
                                    let total_width = available_width - separator_width;
                                    let left_width = (total_width * state.split_ratio)
                                        .max(100.0)
                                        .min(total_width - 100.0);
                                    let right_width = total_width - left_width;

                                    // Left Rect
                                    let left_rect = egui::Rect::from_min_size(
                                        min_rect.min,
                                        egui::vec2(left_width, available_height),
                                    );

                                    // Separator Rect (visible thin line)
                                    let sep_rect = egui::Rect::from_min_size(
                                        egui::pos2(min_rect.min.x + left_width, min_rect.min.y),
                                        egui::vec2(separator_width, available_height),
                                    );

                                    // Drag Handle Rect (wider click target)
                                    let drag_rect = egui::Rect::from_min_size(
                                        egui::pos2(
                                            min_rect.min.x + left_width
                                                - (drag_handle_width - separator_width) / 2.0,
                                            min_rect.min.y,
                                        ),
                                        egui::vec2(drag_handle_width, available_height),
                                    );

                                    // Right Rect
                                    let right_rect = egui::Rect::from_min_size(
                                        egui::pos2(
                                            min_rect.min.x + left_width + separator_width,
                                            min_rect.min.y,
                                        ),
                                        egui::vec2(right_width, available_height),
                                    );

                                    // Left Pane: Editor
                                    let mut left_ui = content_ui.child_ui(
                                        left_rect,
                                        egui::Layout::top_down(egui::Align::Min),
                                    );
                                    left_ui.set_clip_rect(left_rect);
                                    tab.editor_renderer.show(
                                        &mut left_ui,
                                        &mut tab.editor,
                                        state.vault.root_path.as_deref(),
                                        font_size,
                                        line_numbers,
                                        cursor_style,
                                        state.config.tab_width,
                                    );

                                    // Separator / Drag handle
                                    let sep_response =
                                        content_ui.allocate_rect(drag_rect, egui::Sense::drag());

                                    // Paint separator line
                                    let color = if sep_response.dragged() {
                                        content_ui.visuals().selection.bg_fill
                                    } else if sep_response.hovered() {
                                        content_ui.visuals().selection.bg_fill.linear_multiply(0.8)
                                    } else {
                                        content_ui.visuals().widgets.noninteractive.bg_stroke.color
                                    };
                                    content_ui.painter().rect_filled(sep_rect, 0.0, color);

                                    // Update split_ratio based on drag
                                    if sep_response.dragged()
                                        && let Some(mouse_pos) =
                                            content_ui.input(|i| i.pointer.latest_pos())
                                    {
                                        let relative_x = mouse_pos.x - min_rect.left();
                                        new_split_ratio =
                                            Some((relative_x / available_width).clamp(0.1, 0.9));
                                    }

                                    // Change cursor to horizontal resize icon
                                    if sep_response.hovered() || sep_response.dragged() {
                                        content_ui.output_mut(|o| {
                                            o.cursor_icon = egui::CursorIcon::ResizeHorizontal
                                        });
                                    }

                                    // Right Pane: Preview
                                    let mut right_ui = content_ui.child_ui(
                                        right_rect,
                                        egui::Layout::top_down(egui::Align::Min),
                                    );
                                    right_ui.set_clip_rect(right_rect);
                                    state.preview.show(
                                        &mut right_ui,
                                        &mut tab.editor,
                                        &mut tab.editor_renderer,
                                        &tab.path,
                                        state.vault.root_path.as_deref(),
                                        font_size,
                                        theme,
                                        state.config.mirror_mode,
                                        false,
                                        state.config.preview_max_width,
                                    );
                                }
                            }
                        }

                        if tab.view_mode != ViewMode::Preview {
                            crate::editor::completion::render_completion_popup(
                                ui,
                                &mut tab.editor,
                                &mut tab.editor_renderer,
                            );
                        }
                    }
                    if let Some(ratio) = new_split_ratio {
                        state.split_ratio = ratio;
                    }
                }
            } else {
                let is_dark = ui.visuals().dark_mode;
                let logo_texture = if is_dark {
                    state.logo_dark_mode.get_or_insert_with(|| {
                        let image_bytes = include_bytes!("../../assets/logo/dr-md-inapp-dark.png");
                        let image = image::load_from_memory_with_format(
                            image_bytes,
                            image::ImageFormat::Png,
                        )
                        .expect("Failed to load dark-mode logo image");
                        let size = [image.width() as _, image.height() as _];
                        let image_buffer = image.to_rgba8();
                        let pixels = image_buffer.as_flat_samples();
                        let color_image =
                            egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                        ctx.load_texture("logo_dark", color_image, Default::default())
                    })
                } else {
                    state.logo_light_mode.get_or_insert_with(|| {
                        let image_bytes = include_bytes!("../../assets/logo/dr-md-inapp-light.png");
                        let image = image::load_from_memory_with_format(
                            image_bytes,
                            image::ImageFormat::Png,
                        )
                        .expect("Failed to load light-mode logo image");
                        let size = [image.width() as _, image.height() as _];
                        let image_buffer = image.to_rgba8();
                        let pixels = image_buffer.as_flat_samples();
                        let color_image =
                            egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                        ctx.load_texture("logo_light", color_image, Default::default())
                    })
                };

                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() / 4.0);
                    ui.add(egui::Image::new(&*logo_texture).max_width(128.0));
                    ui.add_space(16.0);
                    ui.label(
                        egui::RichText::new("Welcome to Doctor. Markdown")
                            .heading()
                            .strong(),
                    );
                    ui.add_space(12.0);
                    ui.add(egui::Label::new(
                        "Press Ctrl+N to create a new note\n\
                         Press Ctrl+Shift+N to create a new folder\n\
                         or Ctrl+O to open a workspace folder.",
                    ));
                });
            }
        });
}
