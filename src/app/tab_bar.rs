use crate::app::state::AppState;

pub fn render_tab_bar(ui: &mut egui::Ui, state: &mut AppState) {
    let active_tab_changed = match (state.last_active_tab_index, state.active_tab_index) {
        (Some(last), Some(current)) => {
            if last != current {
                state.last_active_tab_index = Some(current);
                true
            } else {
                false
            }
        }
        (None, Some(current)) => {
            state.last_active_tab_index = Some(current);
            false
        }
        _ => {
            state.last_active_tab_index = state.active_tab_index;
            false
        }
    };

    let tab_bar_frame = egui::Frame::none()
        .fill(ui.visuals().panel_fill)
        .inner_margin(egui::Margin::symmetric(8.0, 0.0));

    let inner_res = tab_bar_frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;

            // Calculate dynamic max char limit per tab to fit all tabs responsively
            let num_tabs = state.tabs.len();
            let max_char_len = if num_tabs > 0 {
                let available_width = ui.available_width() - 21.0;

                // 1. Calculate total width needed if we don't truncate at all
                let mut total_full_width = 0.0;
                for tab in &state.tabs {
                    let filename = tab.path.file_name().unwrap_or_default().to_string_lossy();
                    let char_count = filename.chars().count();
                    total_full_width += 32.0 + (char_count as f32) * 7.2;
                }

                if total_full_width <= available_width {
                    // Plenty of space: do not truncate
                    usize::MAX
                } else {
                    // Not enough space: calculate the uniform max tab width that will fit them all
                    let max_tab_width = available_width / (num_tabs as f32);
                    let computed_len = ((max_tab_width - 32.0) / 7.2) as isize;
                    computed_len.max(4) as usize
                }
            } else {
                usize::MAX
            };

            // Render Tabs scroll area
            egui::ScrollArea::horizontal()
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let mut tab_to_close = None;
                        let mut tab_to_switch = None;
                        let mut tab_to_swap = None;

                        for idx in 0..state.tabs.len() {
                            let is_active = state.active_tab_index == Some(idx);
                            let tab_path = state.tabs[idx].path.clone();
                            let filename = tab_path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string();
                            let is_dirty = state.tabs[idx].editor.is_dirty;

                            let tab_id = ui.id().with(&tab_path);
                            let tab_rect = ui
                                .memory(|mem| mem.data.get_temp::<egui::Rect>(tab_id))
                                .unwrap_or(egui::Rect::NOTHING);

                            let is_tab_hovered = ui.rect_contains_pointer(tab_rect);

                            // Only truncate filenames with more than 2 words (separated by spaces, underscores, or hyphens)
                            let word_count = {
                                let base = if let Some(dot_idx) = filename.rfind('.') {
                                    &filename[..dot_idx]
                                } else {
                                    &filename
                                };
                                base.split([' ', '_', '-'])
                                    .filter(|s| !s.is_empty())
                                    .count()
                            };

                            let display_filename = if word_count > 2 {
                                truncate_filename(&filename, max_char_len)
                            } else {
                                filename.clone()
                            };

                            let bg = if is_active {
                                ui.visuals().panel_fill
                            } else if is_tab_hovered {
                                ui.visuals().widgets.hovered.weak_bg_fill
                            } else {
                                egui::Color32::TRANSPARENT
                            };

                            let mut close_rect = egui::Rect::NOTHING;

                            let tab_inner_resp = ui.vertical(|ui| {
                                let tab_frame = egui::Frame::none()
                                    .fill(bg)
                                    .inner_margin(egui::Margin::symmetric(6.0, 6.0));

                                tab_frame.show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 8.0;

                                        let text_color = if is_active {
                                            state.config.theme_accent.color()
                                        } else {
                                            ui.visuals().widgets.noninteractive.fg_stroke.color
                                        };

                                        let mut rich_text = egui::RichText::new(&display_filename)
                                            .color(text_color);
                                        if is_active {
                                            rich_text = rich_text.strong();
                                        }

                                        ui.add(egui::Label::new(rich_text));

                                        // Close / Dirty dot spacer (only allocated when hovered or dirty)
                                        if is_tab_hovered || is_dirty {
                                            let (crect, _) = ui.allocate_exact_size(
                                                egui::vec2(12.0, 12.0),
                                                egui::Sense::click(),
                                            );
                                            close_rect = crect;

                                            let painter = ui.painter();
                                            let is_close_hovered =
                                                ui.rect_contains_pointer(close_rect);
                                            let accent = state.config.theme_accent.color();
                                            if is_close_hovered {
                                                painter.text(
                                                    close_rect.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    "×",
                                                    egui::FontId::monospace(14.0),
                                                    accent,
                                                );
                                            } else if is_tab_hovered {
                                                painter.text(
                                                    close_rect.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    "×",
                                                    egui::FontId::monospace(14.0),
                                                    accent.linear_multiply(0.6),
                                                );
                                            } else if is_dirty {
                                                painter.circle_filled(
                                                    close_rect.center(),
                                                    4.0,
                                                    accent,
                                                );
                                            }
                                        }
                                    });
                                });
                            });

                            let new_tab_rect = tab_inner_resp.response.rect;
                            ui.memory_mut(|mem| {
                                mem.data.insert_temp::<egui::Rect>(tab_id, new_tab_rect)
                            });

                            if is_active {
                                // Draw accent line at the bottom of the tab to connect it nicely
                                let accent = state.config.theme_accent.color();
                                let bottom_line = egui::Rect::from_min_max(
                                    egui::pos2(new_tab_rect.min.x, new_tab_rect.max.y - 1.0),
                                    egui::pos2(new_tab_rect.max.x, new_tab_rect.max.y),
                                );
                                ui.painter().rect_filled(bottom_line, 0.0, accent);

                                if active_tab_changed {
                                    ui.scroll_to_rect(new_tab_rect, None);
                                }
                            }

                            let response = ui.interact(new_tab_rect, tab_id, egui::Sense::drag());

                            if response.hovered() || response.dragged() {
                                egui::show_tooltip_at_pointer(
                                    ui.ctx(),
                                    egui::Id::new("tab_tooltip"),
                                    |ui| {
                                        ui.label(&filename);
                                    },
                                );
                            }

                            if response.dragged() {
                                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);

                                let press_origin = ui.input(|i| i.pointer.press_origin());
                                let is_drag_on_close =
                                    press_origin.is_some_and(|pos| close_rect.contains(pos));

                                if !is_drag_on_close
                                    && let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos())
                                {
                                    let mut target_idx = None;

                                    // Check right neighbor (dragging right)
                                    if idx + 1 < state.tabs.len() {
                                        let other_idx = idx + 1;
                                        let other_path = &state.tabs[other_idx].path;
                                        let other_id = ui.id().with(other_path);
                                        let other_rect = ui
                                            .memory(|mem| mem.data.get_temp::<egui::Rect>(other_id))
                                            .unwrap_or(egui::Rect::NOTHING);
                                        if other_rect != egui::Rect::NOTHING
                                            && mouse_pos.x > other_rect.center().x
                                        {
                                            target_idx = Some(other_idx);
                                        }
                                    }

                                    // Check left neighbor (dragging left)
                                    if target_idx.is_none() && idx > 0 {
                                        let other_idx = idx - 1;
                                        let other_path = &state.tabs[other_idx].path;
                                        let other_id = ui.id().with(other_path);
                                        let other_rect = ui
                                            .memory(|mem| mem.data.get_temp::<egui::Rect>(other_id))
                                            .unwrap_or(egui::Rect::NOTHING);
                                        if other_rect != egui::Rect::NOTHING
                                            && mouse_pos.x < other_rect.center().x
                                        {
                                            target_idx = Some(other_idx);
                                        }
                                    }

                                    if let Some(other_idx) = target_idx {
                                        tab_to_swap = Some((idx, other_idx));
                                    }
                                }
                            } else if response.hovered() {
                                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                            }

                            // Switch/close tab on click/press (excluding close button area)
                            if ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary))
                                && let Some(press_pos) = ui.input(|i| i.pointer.press_origin())
                                && new_tab_rect.contains(press_pos)
                                && !close_rect.contains(press_pos)
                            {
                                tab_to_switch = Some(idx);
                            }
                            // Close button click check
                            if ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary))
                                && let Some(click_pos) = ui.input(|i| i.pointer.interact_pos())
                                && close_rect.contains(click_pos)
                            {
                                tab_to_close = Some(idx);
                            }
                            if ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Middle))
                                && let Some(press_pos) = ui.input(|i| i.pointer.press_origin())
                                && new_tab_rect.contains(press_pos)
                            {
                                tab_to_close = Some(idx);
                            }
                        }

                        if let Some(idx) = tab_to_close {
                            state.close_tab(idx);
                        } else if let Some(idx) = tab_to_switch {
                            state.switch_tab(idx);
                        } else if let Some((idx, other_idx)) = tab_to_swap {
                            state.tabs.swap(idx, other_idx);
                            if state.active_tab_index == Some(idx) {
                                state.active_tab_index = Some(other_idx);
                            } else if state.active_tab_index == Some(other_idx) {
                                state.active_tab_index = Some(idx);
                            }
                            state.sync_session_state();
                        }
                    });
                });
        });
    });

    let scroll_delta = ui.input(|i| i.raw_scroll_delta);
    if ui.rect_contains_pointer(inner_res.response.rect) && scroll_delta.y != 0.0 {
        if (state.tab_scroll_accum > 0.0 && scroll_delta.y < 0.0)
            || (state.tab_scroll_accum < 0.0 && scroll_delta.y > 0.0)
        {
            state.tab_scroll_accum = 0.0;
        }

        state.tab_scroll_accum += scroll_delta.y;

        let threshold = 20.0;
        let mut switched = false;
        let mut new_active_idx = state.active_tab_index;

        if let Some(active_idx) = state.active_tab_index {
            let num_tabs = state.tabs.len();
            if num_tabs > 1 {
                if state.tab_scroll_accum >= threshold {
                    if active_idx > 0 {
                        new_active_idx = Some(active_idx - 1);
                        switched = true;
                    }
                    state.tab_scroll_accum = 0.0;
                } else if state.tab_scroll_accum <= -threshold {
                    if active_idx + 1 < num_tabs {
                        new_active_idx = Some(active_idx + 1);
                        switched = true;
                    }
                    state.tab_scroll_accum = 0.0;
                }
            }
        }

        if switched && let Some(idx) = new_active_idx {
            state.switch_tab(idx);
        }
    } else if state.tab_scroll_accum != 0.0 {
        state.tab_scroll_accum *= 0.8;
        if state.tab_scroll_accum.abs() < 1.0 {
            state.tab_scroll_accum = 0.0;
        }
    }
}

fn truncate_filename(filename: &str, max_len: usize) -> String {
    if filename.chars().count() <= max_len {
        return filename.to_string();
    }

    if let Some(idx) = filename.rfind('.') {
        let extension = &filename[idx..];
        let base = &filename[..idx];

        let base_len = if max_len > extension.len() {
            let len = max_len - extension.len();
            if base.chars().count() >= 3 {
                len.max(3)
            } else {
                len
            }
        } else {
            if base.chars().count() >= 3 {
                3
            } else {
                base.chars().count()
            }
        };

        let truncated_base: String = base.chars().take(base_len).collect();
        return format!("{}{}", truncated_base, extension);
    }

    let base_len = if filename.chars().count() >= 3 {
        max_len.max(3)
    } else {
        max_len
    };
    let truncated: String = filename.chars().take(base_len).collect();
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_filename() {
        assert_eq!(truncate_filename("short.md", 15), "short.md");
        assert_eq!(
            truncate_filename("filename_is_really_long.md", 15),
            "filename_is_.md"
        );
        assert_eq!(
            truncate_filename("no_extension_at_all_long", 15),
            "no_extension_at"
        );
        assert_eq!(
            truncate_filename("long.extension.md", 15),
            "long.extensi.md"
        );

        // Verify minimum base length of 3 is preserved when max_len is very small
        assert_eq!(truncate_filename("filename_is_really_long.md", 4), "fil.md");
        assert_eq!(truncate_filename("no_extension_at_all_long", 2), "no_");
    }
}
