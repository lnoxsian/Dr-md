use crate::search::controller::SearchController;
use crate::search::result::MatchLocation;
use std::path::{Path, PathBuf};

pub struct SearchUi;

impl SearchUi {
    /// Renders the Vault Search panel.
    /// Returns `(clicked_result, close_requested)`.
    pub fn show(
        ui: &mut egui::Ui,
        controller: &mut SearchController,
        vault_root: Option<&Path>,
    ) -> (Option<(PathBuf, MatchLocation)>, bool) {
        let mut clicked_result = None;
        let mut close_requested = false;

        let max_w = ui.available_width();
        ui.set_max_width(max_w);
        ui.style_mut().wrap = Some(true);

        ui.vertical(|ui| {
            // Header: Search Input
            ui.add_space(4.0);
            let text_edit = egui::TextEdit::singleline(&mut controller.query_input)
                .hint_text("Search files and content... (Ctrl+P)")
                .desired_width(ui.available_width());

            let res = ui.add(text_edit);
            if controller.focus_input {
                res.request_focus();
                controller.focus_input = false;
            }

            if res.changed() {
                controller.on_input_changed();
                if let Some(root) = vault_root {
                    controller.trigger_search(root);
                }
                ui.ctx().request_repaint();
            }

            // Status / Error Row
            ui.add_space(4.0);
            if let Some(ref err) = controller.error_message {
                ui.colored_label(egui::Color32::from_rgb(230, 80, 80), err);
            } else if controller.is_indexing {
                let (cur, tot) = controller.indexing_progress;
                ui.colored_label(
                    egui::Color32::GRAY,
                    format!("Indexing vault: {cur}/{tot}..."),
                );
            } else if controller.is_searching {
                ui.colored_label(
                    egui::Color32::GRAY,
                    format!(
                        "Searching... ({} matches in {} files)",
                        controller.total_matches, controller.total_files
                    ),
                );
            } else if !controller.active_query.is_empty() {
                if controller.total_matches == 0 {
                    ui.colored_label(egui::Color32::GRAY, "No matches found");
                } else {
                    let cap_note = if controller.total_matches >= 5000 {
                        " (capped at 5,000 matches)"
                    } else {
                        ""
                    };
                    ui.colored_label(
                        egui::Color32::GRAY,
                        format!(
                            "{} results in {} files ({}ms){}",
                            controller.total_matches,
                            controller.total_files,
                            controller.elapsed_ms,
                            cap_note
                        ),
                    );
                }
            }

            ui.separator();

            // Keyboard navigation in search results
            let input = ui.input(|i| i.clone());
            if input.key_pressed(egui::Key::Escape) {
                close_requested = true;
            } else if input.key_pressed(egui::Key::ArrowDown) {
                controller.select_next_match();
            } else if input.key_pressed(egui::Key::ArrowUp) {
                controller.select_prev_match();
            } else if input.key_pressed(egui::Key::Enter)
                && let Some(match_info) = controller.get_selected_match()
            {
                clicked_result = Some(match_info);
            }

            // Results List
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .max_width(max_w)
                .show(ui, |ui| {
                    ui.set_max_width(max_w);
                    for (file_idx, file_res) in controller.results.iter_mut().enumerate() {
                        let is_file_selected =
                            controller.selected_match.map(|(f, _)| f) == Some(file_idx);
                        let file_id = ui.make_persistent_id(("search_file", file_idx));

                        let header_label =
                            format!("{} ({})", file_res.rel_path, file_res.matches.len());
                        let label_resp = ui.selectable_label(is_file_selected, header_label);
                        if label_resp.clicked() {
                            controller.selected_match = Some((file_idx, 0));
                            let target_match =
                                file_res.matches.first().cloned().unwrap_or_else(|| {
                                    MatchLocation {
                                        line_number: 1,
                                        col_number: 1,
                                        char_offset: 0,
                                        match_length: 0,
                                        snippet: file_res.rel_path.clone(),
                                        snippet_match_range: 0..0,
                                    }
                                });
                            clicked_result = Some((file_res.file_path.clone(), target_match));
                        }

                        let header_interact =
                            ui.interact(label_resp.rect, file_id, egui::Sense::click());
                        if header_interact.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if header_interact.clicked() && clicked_result.is_none() {
                            controller.selected_match = Some((file_idx, 0));
                            let target_match =
                                file_res.matches.first().cloned().unwrap_or_else(|| {
                                    MatchLocation {
                                        line_number: 1,
                                        col_number: 1,
                                        char_offset: 0,
                                        match_length: 0,
                                        snippet: file_res.rel_path.clone(),
                                        snippet_match_range: 0..0,
                                    }
                                });
                            clicked_result = Some((file_res.file_path.clone(), target_match));
                        }

                        if !file_res.is_collapsed {
                            for (match_idx, match_loc) in file_res.matches.iter().enumerate() {
                                let is_selected =
                                    controller.selected_match == Some((file_idx, match_idx));
                                let row_id =
                                    ui.make_persistent_id(("search_match", file_idx, match_idx));

                                let row_resp = ui.horizontal(|ui| {
                                    ui.add_space(14.0); // Indent

                                    // Line badge
                                    ui.colored_label(
                                        egui::Color32::from_gray(140),
                                        format!("L{}:", match_loc.line_number),
                                    );

                                    // Snippet with highlighted match
                                    render_highlighted_snippet(ui, match_loc, is_selected);
                                });

                                let interact = ui.interact(
                                    row_resp.response.rect,
                                    row_id,
                                    egui::Sense::click(),
                                );
                                if interact.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    ui.painter().rect_filled(
                                        row_resp.response.rect,
                                        2.0,
                                        if ui.visuals().dark_mode {
                                            egui::Color32::from_white_alpha(15)
                                        } else {
                                            egui::Color32::from_black_alpha(15)
                                        },
                                    );
                                }

                                if is_selected {
                                    let rect = row_resp.response.rect;
                                    ui.painter().rect_stroke(
                                        rect,
                                        2.0,
                                        egui::Stroke::new(
                                            1.0_f32,
                                            egui::Color32::from_rgb(80, 140, 220),
                                        ),
                                    );
                                }

                                if interact.clicked() {
                                    controller.selected_match = Some((file_idx, match_idx));
                                    clicked_result =
                                        Some((file_res.file_path.clone(), match_loc.clone()));
                                }
                            }
                        }
                    }
                });
        });

        (clicked_result, close_requested)
    }
}

/// Renders a snippet line with the matched characters highlighted.
fn render_highlighted_snippet(ui: &mut egui::Ui, match_loc: &MatchLocation, _is_selected: bool) {
    let mut job = egui::text::LayoutJob::default();
    let text = &match_loc.snippet;
    let range = &match_loc.snippet_match_range;

    let avail_w = ui.available_width();
    job.wrap.max_width = (avail_w - 20.0).max(60.0);
    job.wrap.break_anywhere = false;

    let normal_color = if ui.visuals().dark_mode {
        egui::Color32::from_gray(210)
    } else {
        egui::Color32::from_gray(50)
    };

    let highlight_bg = if ui.visuals().dark_mode {
        egui::Color32::from_rgb(180, 130, 30)
    } else {
        egui::Color32::from_rgb(255, 230, 100)
    };

    let highlight_text = if ui.visuals().dark_mode {
        egui::Color32::WHITE
    } else {
        egui::Color32::BLACK
    };

    let char_indices: Vec<(usize, char)> = text.char_indices().collect();
    let total_chars = char_indices.len();

    let start_char = range.start.min(total_chars);
    let end_char = range.end.min(total_chars);

    let byte_start = char_indices
        .get(start_char)
        .map(|&(b, _)| b)
        .unwrap_or(text.len());
    let byte_end = char_indices
        .get(end_char)
        .map(|&(b, _)| b)
        .unwrap_or(text.len());

    let before = &text[..byte_start];
    let matched = &text[byte_start..byte_end];
    let after = &text[byte_end..];

    let font_id = egui::FontId::monospace(12.0);

    // Text before match
    if !before.is_empty() {
        job.append(
            before,
            0.0,
            egui::TextFormat {
                font_id: font_id.clone(),
                color: normal_color,
                ..Default::default()
            },
        );
    }

    // Matched text (highlighted)
    if !matched.is_empty() {
        job.append(
            matched,
            0.0,
            egui::TextFormat {
                font_id: font_id.clone(),
                color: highlight_text,
                background: highlight_bg,
                ..Default::default()
            },
        );
    }

    // Text after match
    if !after.is_empty() {
        job.append(
            after,
            0.0,
            egui::TextFormat {
                font_id,
                color: normal_color,
                ..Default::default()
            },
        );
    }

    ui.label(job);
}
