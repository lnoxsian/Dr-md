use crate::app::commands;
use crate::app::state::{AppState, ViewMode};

/// Renders a uniform menu item with left-aligned label and right-aligned shortcut text.
fn menu_item(
    ui: &mut egui::Ui,
    label: &str,
    shortcut: impl Into<egui::WidgetText>,
) -> egui::Response {
    let button = egui::Button::new(label)
        .shortcut_text(shortcut)
        .wrap(false);
    ui.add(button)
}

/// Renders an enabled/disabled uniform menu item.
fn menu_item_enabled(
    ui: &mut egui::Ui,
    enabled: bool,
    label: &str,
    shortcut: impl Into<egui::WidgetText>,
) -> egui::Response {
    let button = egui::Button::new(label)
        .shortcut_text(shortcut)
        .wrap(false);
    ui.add_enabled(enabled, button)
}

/// Renders a uniform menu checkbox item with a native egui checkbox on the left and right-aligned shortcut.
fn menu_checkbox(
    ui: &mut egui::Ui,
    checked: &mut bool,
    label: &str,
    shortcut: impl Into<egui::WidgetText>,
) -> egui::Response {
    let shortcut_widget = shortcut.into().color(ui.visuals().weak_text_color());
    ui.horizontal(|ui| {
        let mut response = ui.checkbox(checked, label);
        if !shortcut_widget.is_empty() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let shortcut_resp = ui.add(
                    egui::Label::new(shortcut_widget)
                        .sense(egui::Sense::click()),
                );
                if shortcut_resp.clicked() {
                    *checked = !*checked;
                    response.mark_changed();
                }
            });
        }
        response
    })
    .inner
}

pub fn render_menu_bar(ctx: &egui::Context, state: &mut AppState) {
    if !state.focus_mode {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    ui.set_min_width(240.0);
                    ui.style_mut().wrap = Some(false);
                    if menu_item(ui, "New Note", "Ctrl+N").clicked() {
                        if let Some(ref root) = state.vault.root_path {
                            state.explorer_visible = true;
                            let target_dir = state
                                .explorer
                                .selected_folder
                                .clone()
                                .unwrap_or_else(|| root.clone());
                            state.explorer.start_creation(
                                crate::explorer::tree::CreatingType::File {
                                    parent_dir: target_dir,
                                },
                            );
                        }
                        ui.close_menu();
                    }
                    if menu_item(ui, "New Folder", "Ctrl+Shift+N").clicked() {
                        if let Some(ref root) = state.vault.root_path {
                            state.explorer_visible = true;
                            let target_dir = state
                                .explorer
                                .selected_folder
                                .clone()
                                .unwrap_or_else(|| root.clone());
                            state.explorer.start_creation(
                                crate::explorer::tree::CreatingType::Folder {
                                    parent_dir: target_dir,
                                },
                            );
                        }
                        ui.close_menu();
                    }
                    if menu_item(ui, "Open Folder", "Ctrl+O").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            commands::execute_open_folder(state, path);
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if menu_item(ui, "Save", "Ctrl+S").clicked() {
                        commands::execute_save(state);
                        ui.close_menu();
                    }
                    if menu_item(ui, "Save As", "Ctrl+Shift+S").clicked() {
                        if let Some(path) = rfd::FileDialog::new().save_file() {
                            commands::execute_save_as(state, path);
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if menu_item(ui, "Close Note", "Ctrl+W").clicked() {
                        if let Some(idx) = state.active_tab_index {
                            state.close_tab(idx);
                        }
                        ui.close_menu();
                    }
                });

                ui.menu_button("Edit", |ui| {
                    ui.set_min_width(240.0);
                    ui.style_mut().wrap = Some(false);
                    let has_active_tab = state.active_tab().is_some();
                    ui.set_enabled(has_active_tab);

                    if menu_item(ui, "Cut", "Ctrl+X").clicked() {
                        let id = state.editor_id();
                        if let Some(mut text_state) =
                            egui::widgets::text_edit::TextEditState::load(ctx, id)
                            && let Some(range) = text_state.cursor.char_range() {
                                let start = range.primary.index.min(range.secondary.index);
                                let end = range.primary.index.max(range.secondary.index);
                                let sorted = start..end;
                                if !sorted.is_empty()
                                    && let Some(tab) = state.active_tab_mut() {
                                        let text_to_copy = tab
                                            .editor
                                            .buffer
                                            .rope
                                            .slice(sorted.clone())
                                            .to_string();
                                        ctx.copy_text(text_to_copy);

                                        tab.editor.buffer.remove(sorted.start, sorted.end);
                                        tab.editor.cursor.char_idx = sorted.start;
                                        tab.editor.selection.clear(sorted.start);
                                        tab.editor.is_dirty = true;

                                        tab.editor_renderer.content_buffer =
                                            tab.editor.buffer.to_string();

                                        let cursor = egui::text::CCursor::new(sorted.start);
                                        text_state.cursor.set_char_range(Some(
                                            egui::text::CCursorRange::two(cursor, cursor),
                                        ));
                                        text_state.store(ctx, id);
                                    }
                            }
                        ui.close_menu();
                    }
                    if menu_item(ui, "Copy", "Ctrl+C").clicked() {
                        let id = state.editor_id();
                        if let Some(text_state) =
                            egui::widgets::text_edit::TextEditState::load(ctx, id)
                            && let Some(range) = text_state.cursor.char_range() {
                                let start = range.primary.index.min(range.secondary.index);
                                let end = range.primary.index.max(range.secondary.index);
                                let sorted = start..end;
                                if !sorted.is_empty()
                                    && let Some(tab) = state.active_tab() {
                                        let text_to_copy =
                                            tab.editor.buffer.rope.slice(sorted).to_string();
                                        ctx.copy_text(text_to_copy);
                                    }
                            }
                        ui.close_menu();
                    }
                    if menu_item(ui, "Paste", "Ctrl+V").clicked() {
                        if let Ok(mut clipboard) = arboard::Clipboard::new() {
                            let paste_text = clipboard.get_text().unwrap_or_default();
                            if !paste_text.is_empty() {
                                let id = state.editor_id();
                                if let Some(mut text_state) =
                                    egui::widgets::text_edit::TextEditState::load(ctx, id)
                                    && let Some(tab) = state.active_tab_mut() {
                                        let range =
                                            text_state.cursor.char_range().unwrap_or_else(|| {
                                                let len = tab.editor.buffer.len_chars();
                                                let cursor = egui::text::CCursor::new(len);
                                                egui::text::CCursorRange::two(cursor, cursor)
                                            });
                                        let start = range.primary.index.min(range.secondary.index);
                                        let end = range.primary.index.max(range.secondary.index);
                                        let sorted = start..end;

                                        if !sorted.is_empty() {
                                            tab.editor.buffer.remove(sorted.start, sorted.end);
                                        }

                                        tab.editor.buffer.insert(sorted.start, &paste_text);
                                        tab.editor.cursor.char_idx =
                                            sorted.start + paste_text.chars().count();
                                        tab.editor.selection.clear(tab.editor.cursor.char_idx);
                                        tab.editor.is_dirty = true;

                                        tab.editor_renderer.content_buffer =
                                            tab.editor.buffer.to_string();

                                        let cursor =
                                            egui::text::CCursor::new(tab.editor.cursor.char_idx);
                                        text_state.cursor.set_char_range(Some(
                                            egui::text::CCursorRange::two(cursor, cursor),
                                        ));
                                        text_state.store(ctx, id);
                                    }
                            }
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if menu_item(ui, "Undo", "Ctrl+Z").clicked() {
                        state.sync_cursor_from_egui(ctx);
                        if let Some(tab) = state.active_tab_mut() {
                            tab.editor.undo();
                        }
                        ui.close_menu();
                    }
                    if menu_item(ui, "Redo", "Ctrl+Shift+Z").clicked() {
                        state.sync_cursor_from_egui(ctx);
                        if let Some(tab) = state.active_tab_mut() {
                            tab.editor.redo();
                        }
                        ui.close_menu();
                    }
                    if menu_item(ui, "Select All", "Ctrl+A").clicked() {
                        state.sync_cursor_from_egui(ctx);
                        let id = state.editor_id();
                        if let Some(mut text_state) =
                            egui::widgets::text_edit::TextEditState::load(ctx, id)
                            && let Some(tab) = state.active_tab() {
                                let len = tab.editor.buffer.len_chars();
                                let anchor = egui::text::CCursor::new(0);
                                let head = egui::text::CCursor::new(len);
                                text_state.cursor.set_char_range(Some(
                                    egui::text::CCursorRange::two(anchor, head),
                                ));
                                text_state.store(ctx, id);
                            }
                        ui.close_menu();
                    }
                    ui.separator();
                    if menu_item(ui, "Find", "Ctrl+F").clicked() {
                        state.sync_cursor_from_egui(ctx);
                        if let Some(tab) = state.active_tab_mut() {
                            tab.editor_renderer.find_visible = true;
                            tab.editor_renderer.focus_search_input = true;
                            tab.editor_renderer.update_find_matches();
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    // Formatting
                    if menu_item(ui, "Bold", "Ctrl+B").clicked() {
                        state.sync_cursor_from_egui(ctx);
                        if let Some(tab) = state.active_tab_mut() {
                            tab.editor.format_selection("bold");
                        }
                        ui.close_menu();
                    }
                    if menu_item(ui, "Italic", "Ctrl+I").clicked() {
                        state.sync_cursor_from_egui(ctx);
                        if let Some(tab) = state.active_tab_mut() {
                            tab.editor.format_selection("italic");
                        }
                        ui.close_menu();
                    }
                    if menu_item(ui, "Link", "Ctrl+K").clicked() {
                        state.sync_cursor_from_egui(ctx);
                        if let Some(tab) = state.active_tab_mut() {
                            tab.editor.format_selection("link");
                        }
                        ui.close_menu();
                    }
                    if menu_item(ui, "Comment", "Ctrl+/").clicked() {
                        state.sync_cursor_from_egui(ctx);
                        if let Some(tab) = state.active_tab_mut() {
                            tab.editor.format_selection("comment");
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if menu_item(ui, "Code Block", "Ctrl+Shift+C").clicked() {
                        state.sync_cursor_from_egui(ctx);
                        if let Some(tab) = state.active_tab_mut() {
                            tab.editor.format_selection("code");
                        }
                        ui.close_menu();
                    }
                    if menu_item(ui, "Checkbox", "Ctrl+L").clicked() {
                        state.sync_cursor_from_egui(ctx);
                        if let Some(tab) = state.active_tab_mut() {
                            tab.editor.format_selection("checkbox");
                        }
                        ui.close_menu();
                    }
                    if menu_item(ui, "Table", "Ctrl+Shift+T").clicked() {
                        state.insert_table_dialog_open = true;
                        state.insert_table_dialog_pos = state
                            .active_tab()
                            .and_then(|t| t.editor.cursor_screen_pos)
                            .or_else(|| ctx.input(|i| i.pointer.latest_pos()));
                        ui.close_menu();
                    }
                    if menu_item(ui, "Numbered List", "").clicked() {
                        state.sync_cursor_from_egui(ctx);
                        if let Some(tab) = state.active_tab_mut() {
                            tab.editor.format_selection("numbered_list");
                        }
                        ui.close_menu();
                    }
                    if menu_item(ui, "Bulleted List", "").clicked() {
                        state.sync_cursor_from_egui(ctx);
                        if let Some(tab) = state.active_tab_mut() {
                            tab.editor.format_selection("bulleted_list");
                        }
                        ui.close_menu();
                    }
                    if menu_item(ui, "Blockquote", "").clicked() {
                        state.sync_cursor_from_egui(ctx);
                        if let Some(tab) = state.active_tab_mut() {
                            tab.editor.format_selection("indent");
                        }
                        ui.close_menu();
                    }
                });

                ui.menu_button("View", |ui| {
                    ui.set_min_width(240.0);
                    ui.style_mut().wrap = Some(false);
                    let has_active_tab = state.active_tab().is_some();
                    let current_view_mode = state.active_tab().map(|t| t.view_mode);

                    ui.add_enabled_ui(has_active_tab, |ui| {
                        let mut is_split = current_view_mode == Some(ViewMode::Split);
                        if menu_checkbox(ui, &mut is_split, "Split", "Ctrl+1").changed() {
                            if let Some(tab) = state.active_tab_mut() {
                                tab.view_mode = if is_split {
                                    ViewMode::Split
                                } else {
                                    ViewMode::Preview
                                };
                                state.sync_session_state();
                            }
                        }

                        let mut is_preview = current_view_mode == Some(ViewMode::Preview);
                        if menu_checkbox(ui, &mut is_preview, "Preview", "Ctrl+2").changed() {
                            if let Some(tab) = state.active_tab_mut() {
                                tab.view_mode = if is_preview {
                                    ViewMode::Preview
                                } else {
                                    ViewMode::Split
                                };
                                state.sync_session_state();
                            }
                        }
                    });
                    ui.separator();
                    menu_checkbox(ui, &mut state.explorer_visible, "Explorer", "Ctrl+E");
                    if menu_checkbox(ui, &mut state.focus_mode, "Focus", "F11").changed() {
                        state.explorer_visible = !state.focus_mode;
                    }
                });

                ui.menu_button("Navigate", |ui| {
                    ui.set_min_width(240.0);
                    ui.style_mut().wrap = Some(false);
                    if menu_item(ui, "Search Files", "Ctrl+P").clicked() {
                        state.explorer_visible = true;
                        state.explorer.toggle_search();
                        ui.close_menu();
                    }
                    ui.separator();
                    let has_tabs = !state.tabs.is_empty();
                    if menu_item_enabled(ui, has_tabs, "Next Tab", "Ctrl+Tab").clicked() {
                        if let Some(idx) = state.active_tab_index {
                            let next_idx = (idx + 1) % state.tabs.len();
                            state.switch_tab(next_idx);
                            ctx.memory_mut(|mem| {
                                if let Some(id) = mem.focused() {
                                    mem.surrender_focus(id);
                                }
                            });
                        }
                        ui.close_menu();
                    }
                    if menu_item_enabled(ui, has_tabs, "Previous Tab", "Ctrl+Shift+Tab").clicked() {
                        if let Some(idx) = state.active_tab_index {
                            let prev_idx = if idx == 0 {
                                state.tabs.len() - 1
                            } else {
                                idx - 1
                            };
                            state.switch_tab(prev_idx);
                            ctx.memory_mut(|mem| {
                                if let Some(id) = mem.focused() {
                                    mem.surrender_focus(id);
                                }
                            });
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if menu_item_enabled(ui, has_tabs, "Go Back", "Alt+Left").clicked() {
                        if let Some(idx) = state.active_tab_index {
                            let prev_idx = if idx == 0 {
                                state.tabs.len() - 1
                            } else {
                                idx - 1
                            };
                            state.switch_tab(prev_idx);
                            ctx.memory_mut(|mem| {
                                if let Some(id) = mem.focused() {
                                    mem.surrender_focus(id);
                                }
                            });
                        }
                        ui.close_menu();
                    }
                    if menu_item_enabled(ui, has_tabs, "Go Forward", "Alt+Right").clicked() {
                        if let Some(idx) = state.active_tab_index {
                            let next_idx = (idx + 1) % state.tabs.len();
                            state.switch_tab(next_idx);
                            ctx.memory_mut(|mem| {
                                if let Some(id) = mem.focused() {
                                    mem.surrender_focus(id);
                                }
                            });
                        }
                        ui.close_menu();
                    }
                });

                ui.menu_button("Settings", |ui| {
                    ui.set_min_width(240.0);
                    ui.label("Base Theme:");
                    ui.horizontal(|ui| {
                        let themes = [
                            ("dark", egui::Color32::from_rgb(30, 30, 30), "Dark Theme"),
                            (
                                "light",
                                egui::Color32::from_rgb(220, 220, 220),
                                "Light Theme",
                            ),
                            (
                                "solarized_dark",
                                egui::Color32::from_rgb(0, 43, 54),
                                "Solarized Dark",
                            ),
                            (
                                "solarized_light",
                                egui::Color32::from_rgb(253, 246, 227),
                                "Solarized Light",
                            ),
                            (
                                "gruvbox_dark",
                                egui::Color32::from_rgb(40, 40, 40),
                                "Gruvbox Medium Dark",
                            ),
                            (
                                "gruvbox_light",
                                egui::Color32::from_rgb(251, 241, 199),
                                "Gruvbox Light",
                            ),
                        ];

                        for &(theme_id, color, name) in &themes {
                            let is_selected = state.config.theme == theme_id;

                            let (rect, response) = ui
                                .allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());

                            let response = response.on_hover_text(name);

                            let painter = ui.painter();
                            let rounding = egui::Rounding::same(8.0); // Circular

                            let fill = if response.hovered() {
                                color.linear_multiply(0.8)
                            } else {
                                color
                            };

                            painter.rect_filled(rect, rounding, fill);

                            if is_selected {
                                painter.rect_stroke(
                                    rect.expand(2.0),
                                    egui::Rounding::same(10.0),
                                    egui::Stroke::new(2.0_f32, ui.visuals().text_color()),
                                );
                            } else if response.hovered() {
                                painter.rect_stroke(
                                    rect.expand(1.0),
                                    egui::Rounding::same(9.0),
                                    egui::Stroke::new(
                                        1.0_f32,
                                        ui.visuals().text_color().linear_multiply(0.5),
                                    ),
                                );
                            }

                            if response.clicked() {
                                state.config.theme = theme_id.to_string();
                                let _ = state.config.save();
                                crate::config::apply_theme(ui.ctx(), &state.config);
                            }
                        }
                    });
                    ui.separator();

                    ui.label("Theme Accent:");
                    ui.horizontal(|ui| {
                        use crate::config::ThemeAccent;
                        for accent in ThemeAccent::all() {
                            let color = accent.color();
                            let is_selected = state.config.theme_accent == *accent;

                            let (rect, response) = ui
                                .allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());

                            let response = response.on_hover_text(accent.name());

                            let painter = ui.painter();
                            let rounding = egui::Rounding::same(8.0); // Circular

                            let fill = if response.hovered() {
                                color.linear_multiply(0.8)
                            } else {
                                color
                            };

                            painter.rect_filled(rect, rounding, fill);

                            if is_selected {
                                painter.rect_stroke(
                                    rect.expand(2.0),
                                    egui::Rounding::same(10.0),
                                    egui::Stroke::new(2.0_f32, ui.visuals().text_color()),
                                );
                            } else if response.hovered() {
                                painter.rect_stroke(
                                    rect.expand(1.0),
                                    egui::Rounding::same(9.0),
                                    egui::Stroke::new(
                                        1.0_f32,
                                        ui.visuals().text_color().linear_multiply(0.5),
                                     ),
                                 );
                             }

                            if response.clicked() {
                                state.config.theme_accent = *accent;
                                let _ = state.config.save();
                                crate::config::apply_theme(ui.ctx(), &state.config);
                            }
                        }
                    });
                    ui.separator();

                    ui.label("Font Size:");
                    if ui
                        .add(egui::Slider::new(&mut state.config.font_size, 10.0..=30.0))
                        .changed()
                    {
                        let _ = state.config.save();
                        crate::config::apply_theme(ui.ctx(), &state.config);
                    }
                    ui.separator();
                    ui.label("Content Width:");
                    if ui
                        .add(
                            egui::Slider::new(&mut state.config.preview_max_width, 20.0..=100.0)
                                .custom_formatter(|n, _| format!("{:.0}%", n)),
                        )
                        .on_hover_text("Maximum content width percentage in preview pane.")
                        .changed()
                    {
                        let _ = state.config.save();
                    }
                    ui.separator();
                    if ui
                        .checkbox(&mut state.config.line_numbers, "Line Numbers")
                        .changed()
                    {
                        let _ = state.config.save();
                    }
                    if ui
                        .checkbox(&mut state.config.autosave, "Autosave")
                        .changed()
                    {
                        let _ = state.config.save();
                    }
                    if ui
                        .checkbox(&mut state.config.reopen_last_files, "Reopen Files")
                        .changed()
                    {
                        state.sync_session_state();
                    }
                    if ui
                        .checkbox(&mut state.config.mirror_mode, "Mirror Scroll")
                        .changed()
                    {
                        let _ = state.config.save();
                    }
                    if ui
                        .checkbox(&mut state.config.gpu_acceleration, "GPU Acceleration")
                        .on_hover_text("Enable GPU hardware acceleration or disable to fallback to CPU software rendering (requires app restart)")
                        .changed()
                    {
                        let _ = state.config.save();
                    }
                    ui.separator();
                    ui.label("Cursor Style:");
                    let mut cursor_style = state.config.cursor_style;
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_value(
                                &mut cursor_style,
                                crate::config::CursorStyle::IBeam,
                                "Beam",
                            )
                            .changed()
                        {
                            state.config.cursor_style = cursor_style;
                            let _ = state.config.save();
                            crate::config::apply_theme(ui.ctx(), &state.config);
                        }
                        if ui
                            .selectable_value(
                                &mut cursor_style,
                                crate::config::CursorStyle::Block,
                                "Block",
                            )
                            .changed()
                        {
                            state.config.cursor_style = cursor_style;
                            let _ = state.config.save();
                            crate::config::apply_theme(ui.ctx(), &state.config);
                        }
                        if ui
                            .selectable_value(
                                &mut cursor_style,
                                crate::config::CursorStyle::Underline,
                                "Underline",
                            )
                            .changed()
                        {
                            state.config.cursor_style = cursor_style;
                            let _ = state.config.save();
                            crate::config::apply_theme(ui.ctx(), &state.config);
                        }
                    });
                });
            });
        });
    }
}
