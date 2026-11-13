use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CreatingType {
    File { parent_dir: PathBuf },
    Folder { parent_dir: PathBuf },
    Renaming { path: PathBuf },
}

#[derive(Clone, Debug)]
pub enum ClipboardAction {
    Copy(Vec<PathBuf>),
    Cut(Vec<PathBuf>),
}

pub struct FileTree {
    pub search_visible: bool,
    pub search_query: String,
    pub focus_search: bool,
    pub search_selected_index: usize,
    pub creating_type: Option<CreatingType>,
    pub name_buffer: String,
    pub focus_input: bool,
    pub clipboard: Option<ClipboardAction>,
    pub selected_folder: Option<PathBuf>,
    pub selected_items: HashSet<PathBuf>,
    pub dragged_items: Vec<PathBuf>,
    pub hovered_folder: Option<PathBuf>,
    pub last_hovered_folder: Option<PathBuf>,
    pub drag_select_rect: Option<egui::Rect>,
    pub initial_selected_items: HashSet<PathBuf>,
    pub drag_started_on_item: bool,
}

impl FileTree {
    pub fn new() -> Self {
        Self {
            search_visible: false,
            search_query: String::new(),
            focus_search: false,
            search_selected_index: 0,
            creating_type: None,
            name_buffer: String::new(),
            focus_input: false,
            clipboard: None,
            selected_folder: None,
            selected_items: HashSet::new(),
            dragged_items: Vec::new(),
            hovered_folder: None,
            last_hovered_folder: None,
            drag_select_rect: None,
            initial_selected_items: HashSet::new(),
            drag_started_on_item: false,
        }
    }

    #[allow(dead_code)]
    pub fn toggle_search(&mut self) {
        self.search_visible = !self.search_visible;
        if self.search_visible {
            self.focus_search = true;
        } else {
            self.search_query.clear();
        }
    }

    pub fn start_creation(&mut self, creation: CreatingType) {
        self.creating_type = Some(creation);
        self.name_buffer.clear();
        self.focus_input = true;
    }

    pub fn start_rename(&mut self, path: PathBuf) {
        self.name_buffer = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        self.creating_type = Some(CreatingType::Renaming { path });
        self.focus_input = true;
    }

    pub fn collect_matching_files(
        &self,
        dir: &Path,
        root: &Path,
        query: &str,
        matches: &mut Vec<(PathBuf, String, String)>,
    ) {
        let query_lower = query.to_lowercase();
        self.collect_matching_files_inner(dir, root, &query_lower, matches);
    }

    fn collect_matching_files_inner(
        &self,
        dir: &Path,
        root: &Path,
        query_lower: &str,
        matches: &mut Vec<(PathBuf, String, String)>,
    ) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') {
                    continue;
                }
                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                if is_dir {
                    self.collect_matching_files_inner(&path, root, query_lower, matches);
                } else {
                    let is_md = path
                        .extension()
                        .map(|e| e == "md" || e == "markdown")
                        .unwrap_or(false);
                    if is_md {
                        let rel_path = path.strip_prefix(root).unwrap_or(&path);
                        let matches_name = name.to_lowercase().contains(query_lower);
                        let matches_path = if matches_name {
                            true
                        } else {
                            rel_path
                                .to_string_lossy()
                                .to_lowercase()
                                .contains(query_lower)
                        };
                        if matches_path {
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
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        root: &Path,
        active_file: &mut Option<PathBuf>,
    ) -> Option<PathBuf> {
        self.last_hovered_folder = self.hovered_folder.take();

        if self.search_visible {
            let esc_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));
            if esc_pressed {
                self.search_visible = false;
                self.search_query.clear();
            } else {
                let mut close_search = false;
                let mut text_edit_lost_focus = false;

                ui.add_space(4.0);
                let input_w = ui.available_width();

                let text_edit = egui::TextEdit::singleline(&mut self.search_query)
                    .hint_text("Search files...")
                    .desired_width(f32::INFINITY);

                let response = ui.add_sized(egui::vec2(input_w, 24.0), text_edit);

                if self.focus_search {
                    response.request_focus();
                    self.focus_search = false;
                } else if response.lost_focus() {
                    text_edit_lost_focus = true;
                }

                let query = self.search_query.trim().to_string();
                let mut clicked_file = None;

                if !query.is_empty() {
                    let mut matches = Vec::new();
                    self.collect_matching_files(root, root, &query, &mut matches);
                    matches.sort_by(|a, b| a.1.cmp(&b.1));

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        if matches.is_empty() {
                            ui.weak("No matching files found");
                        } else {
                            if self.search_selected_index >= matches.len() {
                                self.search_selected_index = 0;
                            }

                            let down_pressed = ui.input(|i| i.key_pressed(egui::Key::ArrowDown));
                            let up_pressed = ui.input(|i| i.key_pressed(egui::Key::ArrowUp));
                            let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));

                            if down_pressed {
                                if self.search_selected_index + 1 < matches.len() {
                                    self.search_selected_index += 1;
                                }
                            } else if up_pressed {
                                if self.search_selected_index > 0 {
                                    self.search_selected_index -= 1;
                                }
                            } else if enter_pressed && !matches.is_empty() {
                                clicked_file = Some(matches[self.search_selected_index].0.clone());
                                close_search = true;
                            }

                            for (idx, (path, name, parent_dir)) in matches.into_iter().enumerate() {
                                let is_selected = self.search_selected_index == idx
                                    || self.selected_items.contains(&path);

                                let label_text = if parent_dir.is_empty() {
                                    name.clone()
                                } else {
                                    format!("{} ({})", name, parent_dir)
                                };

                                let item_w = ui.available_width();
                                let (rect, response) = ui.allocate_exact_size(
                                    egui::vec2(item_w, 22.0),
                                    egui::Sense::click(),
                                );

                                if ui.is_rect_visible(rect) {
                                    if is_selected {
                                        ui.painter().rect_filled(
                                            rect,
                                            2.0,
                                            ui.visuals().selection.bg_fill,
                                        );
                                    } else if response.hovered() {
                                        ui.painter().rect_filled(
                                            rect,
                                            2.0,
                                            ui.visuals().widgets.hovered.bg_fill,
                                        );
                                    }

                                    let text_color = if is_selected {
                                        ui.visuals().selection.stroke.color
                                    } else {
                                        ui.visuals().text_color()
                                    };

                                    let mut child_ui = ui
                                        .child_ui(rect.shrink2(egui::vec2(4.0, 1.0)), *ui.layout());
                                    child_ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(label_text).color(text_color),
                                        )
                                        .truncate(true),
                                    );
                                }

                                if response.clicked() {
                                    self.search_selected_index = idx;
                                    self.selected_items.clear();
                                    self.selected_items.insert(path.clone());
                                    clicked_file = Some(path);
                                    close_search = true;
                                }
                            }
                        }
                    });
                }

                if clicked_file.is_some()
                    || close_search
                    || (text_edit_lost_focus && clicked_file.is_none())
                {
                    self.search_visible = false;
                    self.search_query.clear();
                    if clicked_file.is_some() {
                        return clicked_file;
                    }
                } else {
                    return None;
                }
            }
        }

        // Standard hierarchical tree view inside scroll area
        let mut clicked_file = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Handle marquee drag selection input
            let pointer = ui.input(|i| i.pointer.clone());
            if !pointer.any_down() {
                self.drag_started_on_item = false;
            }

            if pointer.any_down()
                && !self.focus_input
                && self.dragged_items.is_empty()
                && !self.drag_started_on_item
            {
                if pointer.is_decidedly_dragging()
                    && let Some(press_origin) = pointer.press_origin()
                    && ui.clip_rect().contains(press_origin)
                {
                    let latest_pos = pointer.latest_pos().unwrap_or(press_origin);
                    let selection_rect = egui::Rect::from_two_pos(press_origin, latest_pos);

                    if self.drag_select_rect.is_none() {
                        let ctrl_pressed = ui.input(|i| i.modifiers.command);
                        if ctrl_pressed {
                            self.initial_selected_items = self.selected_items.clone();
                        } else {
                            self.initial_selected_items.clear();
                        }
                    }

                    self.drag_select_rect = Some(selection_rect);
                    self.selected_items = self.initial_selected_items.clone();
                }
            } else {
                self.drag_select_rect = None;
            }

            self.render_dir(ui, root, &mut clicked_file, active_file, true);

            // Render expanding empty space at the bottom of the tree for root actions
            let remaining_space = ui.available_size();
            if remaining_space.y > 10.0 {
                let (_rect, response) = ui.allocate_at_least(remaining_space, egui::Sense::click());

                if response.clicked() {
                    self.selected_items.clear();
                    self.selected_folder = None;
                }

                if response.hovered() && ui.input(|i| i.pointer.any_released()) {
                    let dragged = std::mem::take(&mut self.dragged_items);
                    let valid_dragged: Vec<_> =
                        dragged.into_iter().filter(|src| src != root).collect();
                    if !valid_dragged.is_empty() {
                        self.move_items(&valid_dragged, root, active_file);
                    }
                }

                response.context_menu(|ui| {
                    if ui.button("New File").clicked() {
                        self.start_creation(CreatingType::File {
                            parent_dir: root.to_path_buf(),
                        });
                        ui.close_menu();
                    }
                    if ui.button("New Folder").clicked() {
                        self.start_creation(CreatingType::Folder {
                            parent_dir: root.to_path_buf(),
                        });
                        ui.close_menu();
                    }
                    ui.separator();
                    let can_paste = self.clipboard.is_some();
                    if ui
                        .add_enabled(can_paste, egui::Button::new("Paste"))
                        .clicked()
                    {
                        self.paste_item(root, active_file);
                        ui.close_menu();
                    }
                });
            }

            // Handle drag tooltip
            if !self.dragged_items.is_empty() {
                if let Some(mouse_pos) = ui.ctx().pointer_latest_pos() {
                    egui::Area::new(egui::Id::new("drag_icon"))
                        .order(egui::Order::Tooltip)
                        .fixed_pos(mouse_pos + egui::vec2(10.0, 10.0))
                        .show(ui.ctx(), |ui| {
                            if self.dragged_items.len() == 1 {
                                let name = self.dragged_items[0]
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy();
                                ui.label(name);
                            } else {
                                ui.label(format!("Moving {} items", self.dragged_items.len()));
                            }
                        });
                }

                if ui.input(|i| i.pointer.any_released()) {
                    self.dragged_items.clear();
                }
            }

            // Paint marquee selection rectangle
            if let Some(rect) = self.drag_select_rect {
                let fill_color = ui.visuals().selection.bg_fill.linear_multiply(0.15);
                let stroke_color = ui.visuals().selection.bg_fill;
                ui.painter().rect(
                    rect,
                    0.0,
                    fill_color,
                    egui::Stroke::new(1.0_f32, stroke_color),
                );
                ui.ctx().request_repaint();
            }
        });

        clicked_file
    }

    fn render_dir(
        &mut self,
        ui: &mut egui::Ui,
        path: &Path,
        clicked_file: &mut Option<PathBuf>,
        active_file: &mut Option<PathBuf>,
        is_root: bool,
    ) {
        let mut rendered_count = 0;
        if let Some(creating) = &self.creating_type {
            match creating {
                CreatingType::File { parent_dir } if parent_dir == path => {
                    self.render_creation_input(ui, path, true, clicked_file, active_file);
                    rendered_count += 1;
                }
                CreatingType::Folder { parent_dir } if parent_dir == path => {
                    self.render_creation_input(ui, path, false, clicked_file, active_file);
                    rendered_count += 1;
                }
                _ => {}
            }
        }

        if let Ok(entries) = fs::read_dir(path) {
            let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
            self.sort_entries(&mut entries);

            for entry in entries {
                let entry_path = entry.path();
                let file_name = entry.file_name().to_string_lossy().into_owned();

                if file_name.starts_with('.') {
                    continue;
                }

                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

                let is_renaming = match &self.creating_type {
                    Some(CreatingType::Renaming { path: rename_path }) => {
                        rename_path == &entry_path
                    }
                    _ => false,
                };

                if is_renaming {
                    self.render_creation_input(
                        ui,
                        entry_path.parent().unwrap(),
                        !is_dir,
                        clicked_file,
                        active_file,
                    );
                    rendered_count += 1;
                    continue;
                }

                if is_dir {
                    self.render_folder_node(ui, entry_path, file_name, clicked_file, active_file);
                    rendered_count += 1;
                } else {
                    let is_md = entry_path
                        .extension()
                        .map(|e| e == "md" || e == "markdown")
                        .unwrap_or(false);
                    if is_md {
                        self.render_file_node(ui, entry_path, file_name, clicked_file, active_file);
                        rendered_count += 1;
                    }
                }
            }
        }

        if !is_root && rendered_count == 0 {
            ui.weak("..");
        }
    }

    fn sort_entries(&self, entries: &mut [fs::DirEntry]) {
        entries.sort_by_key(|e| {
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            (!is_dir, e.file_name())
        });
    }

    fn render_folder_node(
        &mut self,
        ui: &mut egui::Ui,
        entry_path: PathBuf,
        file_name: String,
        clicked_file: &mut Option<PathBuf>,
        active_file: &mut Option<PathBuf>,
    ) {
        let id = ui.make_persistent_id(&entry_path);

        if let Some(creating) = &self.creating_type {
            match creating {
                CreatingType::File { parent_dir } | CreatingType::Folder { parent_dir }
                    if parent_dir == &entry_path =>
                {
                    ui.data_mut(|d| d.insert_temp(id, true));
                }
                _ => {}
            }
        }

        let is_selected = self.selected_items.contains(&entry_path);
        let is_hovered_by_drag = !self.dragged_items.is_empty()
            && self.last_hovered_folder.as_ref() == Some(&entry_path);

        let folder_label = if is_selected || is_hovered_by_drag {
            let mut job = egui::text::LayoutJob::default();
            let accent_color = ui.visuals().hyperlink_color;
            let text_color = ui.visuals().text_color();
            job.append(
                &file_name,
                0.0,
                egui::TextFormat {
                    font_id: egui::TextStyle::Body.resolve(ui.style()),
                    color: text_color,
                    underline: egui::Stroke::new(1.0_f32, accent_color),
                    ..Default::default()
                },
            );
            egui::WidgetText::from(job)
        } else {
            egui::WidgetText::from(&file_name)
        };

        let collapsing_response = egui::CollapsingHeader::new(folder_label)
            .id_source(&entry_path)
            .icon(|ui, openness, response| {
                let stroke = ui.style().interact(response).fg_stroke;
                let text = if openness > 0.5 { "-" } else { "+" };
                let font_id = egui::TextStyle::Button.resolve(ui.style());
                ui.painter().text(
                    response.rect.center(),
                    egui::Align2::CENTER_CENTER,
                    text,
                    font_id,
                    stroke.color,
                );
            })
            .show(ui, |ui| {
                self.render_dir(ui, &entry_path, clicked_file, active_file, false);
            });

        let header_response = ui.interact(
            collapsing_response.header_response.rect,
            collapsing_response.header_response.id,
            egui::Sense::click_and_drag(),
        );

        if let Some(press_origin) = ui.input(|i| i.pointer.press_origin())
            && header_response.rect.contains(press_origin)
        {
            self.drag_started_on_item = true;
        }

        // Check marquee selection intersection
        if let Some(select_rect) = self.drag_select_rect
            && select_rect.intersects(header_response.rect)
        {
            self.selected_items.insert(entry_path.clone());
            self.selected_folder = Some(entry_path.clone());
        }

        if header_response.drag_started() {
            if self.selected_items.contains(&entry_path) {
                self.dragged_items = self.selected_items.iter().cloned().collect();
            } else {
                self.dragged_items = vec![entry_path.clone()];
            }
        }

        let is_hovered = ui.rect_contains_pointer(header_response.rect);
        if is_hovered {
            if !self.dragged_items.is_empty() {
                self.hovered_folder = Some(entry_path.clone());
            }
            if ui.input(|i| i.pointer.any_released()) {
                let dragged = std::mem::take(&mut self.dragged_items);
                let valid_dragged: Vec<_> = dragged
                    .into_iter()
                    .filter(|src| src != &entry_path && !entry_path.starts_with(src))
                    .collect();
                if !valid_dragged.is_empty() {
                    self.move_items(&valid_dragged, &entry_path, active_file);
                }
            }
        }

        if header_response.clicked() {
            let ctrl_pressed = ui.input(|i| i.modifiers.command);
            if ctrl_pressed {
                if self.selected_items.contains(&entry_path) {
                    self.selected_items.remove(&entry_path);
                    if self.selected_folder.as_ref() == Some(&entry_path) {
                        self.selected_folder = None;
                    }
                } else {
                    self.selected_items.insert(entry_path.clone());
                    self.selected_folder = Some(entry_path.clone());
                }
            } else {
                self.selected_items.clear();
                self.selected_items.insert(entry_path.clone());
                self.selected_folder = Some(entry_path.clone());
            }
        }

        collapsing_response.header_response.context_menu(|ui| {
            if !self.selected_items.contains(&entry_path) {
                self.selected_items.clear();
                self.selected_items.insert(entry_path.clone());
                self.selected_folder = Some(entry_path.clone());
            }
            self.show_folder_context_menu(ui, &entry_path, active_file);
        });
    }

    fn render_file_node(
        &mut self,
        ui: &mut egui::Ui,
        entry_path: PathBuf,
        file_name: String,
        clicked_file: &mut Option<PathBuf>,
        active_file: &mut Option<PathBuf>,
    ) {
        let is_md = entry_path
            .extension()
            .map(|e| e == "md" || e == "markdown")
            .unwrap_or(false);
        if !is_md {
            return;
        }

        let is_active = self.selected_items.contains(&entry_path);
        let label = ui.selectable_label(is_active, file_name);
        let label_response = ui.interact(label.rect, label.id, egui::Sense::click_and_drag());

        if let Some(press_origin) = ui.input(|i| i.pointer.press_origin())
            && label_response.rect.contains(press_origin)
        {
            self.drag_started_on_item = true;
        }

        // Check marquee selection intersection
        if let Some(select_rect) = self.drag_select_rect
            && select_rect.intersects(label_response.rect)
        {
            self.selected_items.insert(entry_path.clone());
        }

        if label_response.drag_started() {
            if self.selected_items.contains(&entry_path) {
                self.dragged_items = self.selected_items.iter().cloned().collect();
            } else {
                self.dragged_items = vec![entry_path.clone()];
            }
        }

        let is_hovered = ui.rect_contains_pointer(label_response.rect);
        if is_hovered && ui.input(|i| i.pointer.any_released()) {
            let dragged = std::mem::take(&mut self.dragged_items);
            if let Some(parent) = entry_path.parent() {
                let valid_dragged: Vec<_> = dragged
                    .into_iter()
                    .filter(|src| src != parent && !parent.starts_with(src))
                    .collect();
                if !valid_dragged.is_empty() {
                    self.move_items(&valid_dragged, parent, active_file);
                }
            }
        }

        if label_response.clicked() {
            let ctrl_pressed = ui.input(|i| i.modifiers.command);
            if ctrl_pressed {
                if self.selected_items.contains(&entry_path) {
                    self.selected_items.remove(&entry_path);
                } else {
                    self.selected_items.insert(entry_path.clone());
                    *clicked_file = Some(entry_path.clone());
                }
            } else {
                self.selected_items.clear();
                self.selected_items.insert(entry_path.clone());
                self.selected_folder = None;
                *clicked_file = Some(entry_path.clone());
            }
        }

        label.context_menu(|ui| {
            if !self.selected_items.contains(&entry_path) {
                self.selected_items.clear();
                self.selected_items.insert(entry_path.clone());
                self.selected_folder = None;
            }
            self.show_file_context_menu(ui, &entry_path, active_file);
        });
    }

    fn show_folder_context_menu(
        &mut self,
        ui: &mut egui::Ui,
        entry_path: &Path,
        active_file: &mut Option<PathBuf>,
    ) {
        let multi_count = self.selected_items.len();

        if multi_count <= 1 {
            if ui.button("New File").clicked() {
                self.start_creation(CreatingType::File {
                    parent_dir: entry_path.to_path_buf(),
                });
                ui.close_menu();
            }
            if ui.button("New Folder").clicked() {
                self.start_creation(CreatingType::Folder {
                    parent_dir: entry_path.to_path_buf(),
                });
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Rename").clicked() {
                self.start_rename(entry_path.to_path_buf());
                ui.close_menu();
            }
        }

        let cut_label = if multi_count > 1 {
            format!("Cut {} items", multi_count)
        } else {
            "Cut".to_string()
        };
        if ui.button(cut_label).clicked() {
            self.clipboard = Some(ClipboardAction::Cut(
                self.selected_items.iter().cloned().collect(),
            ));
            ui.close_menu();
        }

        let copy_label = if multi_count > 1 {
            format!("Copy {} items", multi_count)
        } else {
            "Copy".to_string()
        };
        if ui.button(copy_label).clicked() {
            self.clipboard = Some(ClipboardAction::Copy(
                self.selected_items.iter().cloned().collect(),
            ));
            ui.close_menu();
        }

        let can_paste = self.clipboard.is_some();
        if ui
            .add_enabled(can_paste, egui::Button::new("Paste"))
            .clicked()
        {
            self.paste_item(entry_path, active_file);
            ui.close_menu();
        }
        ui.separator();

        let delete_label = if multi_count > 1 {
            format!("Delete {} items", multi_count)
        } else {
            "Delete".to_string()
        };
        if ui.button(delete_label).clicked() {
            let items: Vec<_> = self.selected_items.iter().cloned().collect();
            self.delete_items(&items, active_file);
            self.selected_items.clear();
            self.selected_folder = None;
            ui.close_menu();
        }
    }

    fn show_file_context_menu(
        &mut self,
        ui: &mut egui::Ui,
        entry_path: &Path,
        active_file: &mut Option<PathBuf>,
    ) {
        let multi_count = self.selected_items.len();

        if multi_count <= 1 && ui.button("Rename").clicked() {
            self.start_rename(entry_path.to_path_buf());
            ui.close_menu();
        }

        let cut_label = if multi_count > 1 {
            format!("Cut {} items", multi_count)
        } else {
            "Cut".to_string()
        };
        if ui.button(cut_label).clicked() {
            self.clipboard = Some(ClipboardAction::Cut(
                self.selected_items.iter().cloned().collect(),
            ));
            ui.close_menu();
        }

        let copy_label = if multi_count > 1 {
            format!("Copy {} items", multi_count)
        } else {
            "Copy".to_string()
        };
        if ui.button(copy_label).clicked() {
            self.clipboard = Some(ClipboardAction::Copy(
                self.selected_items.iter().cloned().collect(),
            ));
            ui.close_menu();
        }

        ui.separator();

        let delete_label = if multi_count > 1 {
            format!("Delete {} items", multi_count)
        } else {
            "Delete".to_string()
        };
        if ui.button(delete_label).clicked() {
            let items: Vec<_> = self.selected_items.iter().cloned().collect();
            self.delete_items(&items, active_file);
            self.selected_items.clear();
            ui.close_menu();
        }
    }

    fn render_creation_input(
        &mut self,
        ui: &mut egui::Ui,
        _parent: &Path,
        is_file: bool,
        clicked_file: &mut Option<PathBuf>,
        active_file: &mut Option<PathBuf>,
    ) {
        ui.horizontal(|ui| {
            let input_w = ui.available_width();
            let text_edit = egui::TextEdit::singleline(&mut self.name_buffer)
                .hint_text(if is_file {
                    "file_name.md"
                } else {
                    "Folder name"
                })
                .desired_width(f32::INFINITY);

            let response = ui.add_sized(egui::vec2(input_w, 24.0), text_edit);

            if self.focus_input {
                response.request_focus();
                self.focus_input = false;
            }

            let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
            let esc_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));

            if esc_pressed {
                self.creating_type = None;
                self.name_buffer.clear();
            } else if enter_pressed {
                let name = self.name_buffer.trim().to_string();
                if !name.is_empty() {
                    match self.creating_type.clone() {
                        Some(CreatingType::Renaming { path: src_path }) => {
                            let mut dest = src_path.parent().unwrap().join(&name);
                            if src_path.is_file() && dest.extension().is_none() {
                                if let Some(ext) = src_path.extension() {
                                    dest.set_extension(ext);
                                } else {
                                    dest.set_extension("md");
                                }
                            }
                            if src_path.exists() && src_path != dest {
                                let unique_dest = self.get_unique_destination(&dest);
                                if let Err(e) = fs::rename(&src_path, &unique_dest) {
                                    tracing::error!("Failed to rename: {:?}", e);
                                } else {
                                    if let Some(active) = active_file {
                                        if *active == src_path {
                                            *active = unique_dest;
                                        } else if active.starts_with(&src_path)
                                            && let Ok(relative) = active.strip_prefix(&src_path)
                                        {
                                            *active = unique_dest.join(relative);
                                        }
                                    }
                                }
                            }
                        }
                        Some(CreatingType::File { parent_dir }) => {
                            let mut dest = parent_dir.join(&name);
                            if dest.extension().is_none() {
                                dest.set_extension("md");
                            }
                            fs::write(&dest, "").ok();
                            *clicked_file = Some(dest);
                        }
                        Some(CreatingType::Folder { parent_dir }) => {
                            let dest = parent_dir.join(&name);
                            fs::create_dir_all(&dest).ok();
                        }
                        None => {}
                    }
                }
                self.creating_type = None;
                self.name_buffer.clear();
            } else if response.lost_focus() {
                self.creating_type = None;
                self.name_buffer.clear();
            }
        });
    }

    fn copy_dir_all(&self, src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
        fs::create_dir_all(&dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                self.copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
            } else {
                fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
            }
        }
        Ok(())
    }

    fn get_unique_destination(&self, dest: &Path) -> PathBuf {
        if !dest.exists() {
            return dest.to_path_buf();
        }
        let parent = dest.parent().unwrap();
        let file_stem = dest.file_stem().unwrap().to_string_lossy();
        let extension = dest
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();

        let mut base_stem = file_stem.to_string();
        let mut counter = 1;

        if let Some(pos) = file_stem.rfind('_') {
            let suffix = &file_stem[pos + 1..];
            if !suffix.is_empty()
                && suffix.chars().all(|c| c.is_ascii_digit())
                && let Ok(num) = suffix.parse::<usize>()
            {
                base_stem = file_stem[..pos].to_string();
                counter = num + 1;
            }
        }

        loop {
            let candidate = parent.join(format!("{}_{}{}", base_stem, counter, extension));
            if !candidate.exists() {
                return candidate;
            }
            counter += 1;
        }
    }

    pub fn move_items(&self, srcs: &[PathBuf], dest_dir: &Path, active_file: &mut Option<PathBuf>) {
        for src in srcs {
            if src.exists() && dest_dir.is_dir() {
                // Skip if the item is already in the destination folder
                if src.parent() == Some(dest_dir) {
                    continue;
                }

                let filename = src.file_name().unwrap();
                let target = dest_dir.join(filename);
                let unique_target = self.get_unique_destination(&target);
                if unique_target.starts_with(src) {
                    tracing::error!("Cannot move a directory inside itself");
                    continue;
                }

                let result = if src.is_file() {
                    if fs::rename(src, &unique_target).is_err() {
                        if let Err(e) = fs::copy(src, &unique_target) {
                            Err(e)
                        } else {
                            fs::remove_file(src)
                        }
                    } else {
                        Ok(())
                    }
                } else {
                    if fs::rename(src, &unique_target).is_err() {
                        if let Err(e) = self.copy_dir_all(src, &unique_target) {
                            Err(e)
                        } else {
                            fs::remove_dir_all(src)
                        }
                    } else {
                        Ok(())
                    }
                };

                if let Err(e) = result {
                    tracing::error!("Failed to move item: {:?}", e);
                } else {
                    if let Some(active) = active_file {
                        if *active == *src {
                            *active = unique_target;
                        } else if active.starts_with(src)
                            && let Ok(relative) = active.strip_prefix(src)
                        {
                            *active = unique_target.join(relative);
                        }
                    }
                }
            }
        }
    }

    pub fn paste_item(&mut self, dest_dir: &Path, active_file: &mut Option<PathBuf>) {
        if let Some(action) = self.clipboard.clone() {
            match action {
                ClipboardAction::Copy(srcs) => {
                    for src in srcs {
                        if src.exists() {
                            let filename = src.file_name().unwrap();
                            let target = dest_dir.join(filename);
                            let unique_target = self.get_unique_destination(&target);
                            if src.is_file() {
                                if let Err(e) = fs::copy(&src, &unique_target) {
                                    tracing::error!("Failed to copy file: {:?}", e);
                                }
                            } else {
                                if let Err(e) = self.copy_dir_all(&src, &unique_target) {
                                    tracing::error!("Failed to copy directory: {:?}", e);
                                }
                            }
                        }
                    }
                }
                ClipboardAction::Cut(srcs) => {
                    self.move_items(&srcs, dest_dir, active_file);
                    self.clipboard = None;
                }
            }
        }
    }

    pub fn delete_items(&mut self, paths: &[PathBuf], active_file: &mut Option<PathBuf>) {
        for path in paths {
            let result = if path.is_file() {
                fs::remove_file(path)
            } else {
                fs::remove_dir_all(path)
            };

            if let Err(e) = result {
                tracing::error!("Failed to delete item: {:?}", e);
            } else {
                if let Some(active) = active_file
                    && (active == path || active.starts_with(path))
                {
                    *active_file = None;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_tree_drag_started_on_item_prevents_marquee() {
        let mut tree = FileTree::new();
        let file1 = PathBuf::from("file1.md");
        let file2 = PathBuf::from("file2.md");
        tree.selected_items.insert(file1.clone());
        tree.selected_items.insert(file2.clone());

        // 1. With drag_started_on_item = true, dragging should NOT start marquee select or clear selection
        tree.drag_started_on_item = true;

        let ctx = egui::Context::default();

        // Frame 1: Press
        let mut raw_input1 = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1000.0, 1000.0),
            )),
            ..Default::default()
        };
        raw_input1.events.push(egui::Event::PointerButton {
            pos: egui::pos2(10.0, 10.0),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        });
        let _ = ctx.run(raw_input1, |_| {});

        // Frame 2: Move and Show
        let mut raw_input2 = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1000.0, 1000.0),
            )),
            ..Default::default()
        };
        raw_input2
            .events
            .push(egui::Event::PointerMoved(egui::pos2(50.0, 50.0))); // > 3.0 delta

        let _ = ctx.run(raw_input2, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                // Call show
                let mut active_file = None;
                let root = PathBuf::from("/non_existent_directory_for_test");
                tree.show(ui, &root, &mut active_file);
            });
        });

        // Marquee select rect should NOT be active
        assert!(tree.drag_select_rect.is_none());
        // Selection should NOT be cleared
        assert_eq!(tree.selected_items.len(), 2);
        assert!(tree.selected_items.contains(&file1));
        assert!(tree.selected_items.contains(&file2));

        // 2. With drag_started_on_item = false, dragging SHOULD start marquee select and clear selection
        tree.drag_started_on_item = false;

        // Reset ctx for a clean test state
        let ctx = egui::Context::default();

        // Frame 1: Press
        let mut raw_input1 = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1000.0, 1000.0),
            )),
            ..Default::default()
        };
        raw_input1.events.push(egui::Event::PointerButton {
            pos: egui::pos2(10.0, 10.0),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        });
        let _ = ctx.run(raw_input1, |_| {});

        // Frame 2: Move and Show
        let mut raw_input2 = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1000.0, 1000.0),
            )),
            ..Default::default()
        };
        raw_input2
            .events
            .push(egui::Event::PointerMoved(egui::pos2(50.0, 50.0)));

        let _ = ctx.run(raw_input2, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut active_file = None;
                let root = PathBuf::from("/non_existent_directory_for_test");
                tree.show(ui, &root, &mut active_file);
            });
        });

        // Marquee select rect SHOULD be active
        assert!(tree.drag_select_rect.is_some());
        // Selection SHOULD be cleared since Ctrl was not pressed
        assert!(tree.selected_items.is_empty());
    }

    #[test]
    fn test_file_tree_collect_matching_files() {
        let temp_dir = std::env::temp_dir().join("drmd_search_test");
        let sub_dir = temp_dir.join("subfolder");
        let _ = fs::create_dir_all(&sub_dir);

        let file1 = temp_dir.join("alpha_note.md");
        let file2 = sub_dir.join("beta_test.md");
        let file3 = temp_dir.join("gamma_ignored.txt");

        let _ = fs::write(&file1, "alpha content");
        let _ = fs::write(&file2, "beta content");
        let _ = fs::write(&file3, "gamma content");

        let tree = FileTree::new();
        let mut matches = Vec::new();
        tree.collect_matching_files(&temp_dir, &temp_dir, "beta", &mut matches);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].1, "beta_test.md");
        assert_eq!(matches[0].2, "subfolder");

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
