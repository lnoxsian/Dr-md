use crate::app::commands;
use crate::app::state::AppState;
use crate::search::SearchUi;

pub fn render_explorer(ctx: &egui::Context, state: &mut AppState) {
    if state.explorer_visible && !state.focus_mode {
        let screen_width = ctx.screen_rect().width();
        let max_explorer_width = if screen_width < 600.0 {
            (screen_width * 0.35).max(100.0)
        } else {
            300.0
        };
        let default_explorer_width = if screen_width < 600.0 { 120.0 } else { 200.0 };

        let mut panel = egui::SidePanel::left("file_explorer");

        if state.search_active {
            // Clamp the search pane to max_explorer_width with fixed sizing to prevent layout flicker and full-window expansion
            panel = panel.exact_width(max_explorer_width).resizable(false);
        } else {
            panel = panel
                .resizable(true)
                .default_width(default_explorer_width)
                .min_width(default_explorer_width)
                .max_width(max_explorer_width);
        }

        panel.show(ctx, |ui| {
            if state.search_active {
                ui.set_max_width(max_explorer_width);
            }

            if state.search_active {
                if state.vault.root_path.is_some() {
                    let (clicked_result, close_requested) = SearchUi::show(
                        ui,
                        &mut state.search_controller,
                        state.vault.root_path.as_deref(),
                    );

                    if close_requested {
                        state.search_active = false;
                    }

                    if let Some((clicked_path, match_loc)) = clicked_result {
                        commands::execute_open_file(state, clicked_path);
                        if let Some(tab) = state.active_tab_mut() {
                            tab.editor.cursor.char_idx = match_loc.char_offset;
                            tab.editor.selection.anchor = match_loc.char_offset;
                            tab.editor.selection.head =
                                match_loc.char_offset + match_loc.match_length;
                            tab.editor.version += 1;
                            tab.editor_renderer.scroll_to_cursor_requested = true;
                        }
                    }
                } else {
                    ui.vertical_centered(|ui| {
                        ui.label("No folder opened");
                        if ui.button("Open Folder").clicked()
                            && let Some(path) = rfd::FileDialog::new().pick_folder()
                        {
                            commands::execute_open_folder(state, path);
                        }
                    });
                }
            } else if let Some(ref root) = state.vault.root_path {
                let mut active_file = state.vault.active_file.clone();
                let clicked = state.explorer.show(ui, root, &mut active_file);

                if active_file != state.vault.active_file {
                    if active_file.is_none() {
                        if let Some(ref prev_path) = state.vault.active_file
                            && let Some(idx) = state.tabs.iter().position(|t| t.path == *prev_path)
                        {
                            state.close_tab(idx);
                        }
                    } else if let (Some(prev_path), Some(new_path)) =
                        (state.vault.active_file.as_ref(), active_file.as_ref())
                        && let Some(idx) = state.tabs.iter().position(|t| t.path == *prev_path)
                    {
                        state.tabs[idx].path = new_path.clone();
                        state.tabs[idx].editor.active_path = Some(new_path.clone());
                    }
                    state.vault.active_file = active_file.clone();
                }

                if let Some(clicked) = clicked {
                    commands::execute_open_file(state, clicked);
                }
            } else {
                ui.vertical_centered(|ui| {
                    ui.label("No folder opened");
                    if ui.button("Open Folder").clicked()
                        && let Some(path) = rfd::FileDialog::new().pick_folder()
                    {
                        commands::execute_open_folder(state, path);
                    }
                });
            }
        });
    }
}
