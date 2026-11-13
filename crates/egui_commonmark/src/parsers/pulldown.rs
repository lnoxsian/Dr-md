//! Duplicates a lot of stuff for now.

use std::ops::Range;

use crate::{elements::*, Alert, AlertBundle};
use crate::{CommonMarkCache, CommonMarkOptions};

use egui::{self, Id, Pos2, TextStyle, Ui, Vec2};

use pulldown_cmark::{CowStr, HeadingLevel, Options};

fn render_math_to_svg(
    latex: &str,
    font_size: f32,
    text_color: egui::Color32,
) -> Result<String, String> {
    let formula =
        ratex_parser::parse(latex).map_err(|err| format!("RaTeX parse error: {:?}", err))?;
    let layout_options = ratex_layout::LayoutOptions::default();
    let layout_box = ratex_layout::layout(&formula, &layout_options);
    let mut display_list = ratex_layout::to_display_list(&layout_box);

    // Apply Sans-Serif font to math glyphs
    for item in &mut display_list.items {
        if let ratex_types::display_item::DisplayItem::GlyphPath { font, .. } = item {
            match font.as_str() {
                "Math-Italic" | "Main-Italic" => *font = "SansSerif-Italic".to_string(),
                "Main-Regular" => *font = "SansSerif-Regular".to_string(),
                "Main-Bold" | "Math-BoldItalic" | "Main-BoldItalic" => {
                    *font = "SansSerif-Bold".to_string();
                }
                _ => {}
            }
        }
    }

    let options = ratex_svg::SvgOptions {
        font_size: font_size as f64,
        padding: (font_size * 0.15) as f64,
        embed_glyphs: true,
        ..Default::default()
    };
    let svg = ratex_svg::render_to_svg(&display_list, &options);

    // Replace default black color with the current theme text color to support dark mode
    let color_str = format!(
        "rgba({},{},{},{})",
        text_color.r(),
        text_color.g(),
        text_color.b(),
        text_color.a() as f32 / 255.0
    );
    let svg = svg
        .replace("rgba(0,0,0,1)", &color_str)
        .replace("rgb(0,0,0)", &color_str)
        .replace("#000000", &color_str)
        .replace("#000", &color_str);

    Ok(svg)
}

fn encode_math_placeholders(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\\' => '\u{E000}',
            '_' => '\u{E001}',
            '*' => '\u{E002}',
            '[' => '\u{E003}',
            ']' => '\u{E004}',
            '~' => '\u{E005}',
            '`' => '\u{E006}',
            _ => c,
        })
        .collect()
}

fn decode_math_placeholders(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{E000}' => '\\',
            '\u{E001}' => '_',
            '\u{E002}' => '*',
            '\u{E003}' => '[',
            '\u{E004}' => ']',
            '\u{E005}' => '~',
            '\u{E006}' => '`',
            _ => c,
        })
        .collect()
}

fn preprocess_math_blocks(input: &str) -> String {
    let segments = parse_math_segments(input);
    let mut result = String::with_capacity(input.len());
    for segment in segments {
        match segment {
            MathSegment::Text(t) => {
                result.push_str(t);
            }
            MathSegment::InlineMath(m) => {
                result.push('$');
                result.push_str(&encode_math_placeholders(m));
                result.push('$');
            }
            MathSegment::DisplayMath(m) => {
                result.push('$');
                result.push('$');
                result.push_str(&encode_math_placeholders(m));
                result.push('$');
                result.push('$');
            }
        }
    }
    result
}

#[derive(Default, Debug)]
pub struct ScrollableCache {
    available_size: Vec2,
    page_size: Option<Vec2>,
    split_points: Vec<(usize, Pos2, Pos2)>,
}

pub type EventIteratorItem<'e> = (usize, (pulldown_cmark::Event<'e>, Range<usize>));

/// Parse events until a desired end tag is reached or no more events are found.
/// This is needed for multiple events that must be rendered inside a single widget
fn delayed_events<'e>(
    events: &mut impl Iterator<Item = EventIteratorItem<'e>>,
    end_at: pulldown_cmark::TagEnd,
) -> Vec<(pulldown_cmark::Event<'e>, Range<usize>)> {
    let mut curr_event = events.next();
    let mut total_events = Vec::new();
    loop {
        if let Some(event) = curr_event.take() {
            total_events.push(event.1.clone());
            if let (_, (pulldown_cmark::Event::End(tag), _range)) = event {
                if end_at == tag {
                    return total_events;
                }
            }
        } else {
            return total_events;
        }

        curr_event = events.next();
    }
}

fn delayed_events_list_item<'e>(
    events: &mut impl Iterator<Item = EventIteratorItem<'e>>,
) -> Vec<(pulldown_cmark::Event<'e>, Range<usize>)> {
    let mut curr_event = events.next();
    let mut total_events = Vec::new();
    loop {
        if let Some(event) = curr_event.take() {
            total_events.push(event.1.clone());
            if let (_, (pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Item), _range)) = event {
                return total_events;
            }

            if let (_, (pulldown_cmark::Event::Start(pulldown_cmark::Tag::List(_)), _range)) = event
            {
                return total_events;
            }
        } else {
            return total_events;
        }

        curr_event = events.next();
    }
}

type Column<'e> = Vec<(pulldown_cmark::Event<'e>, Range<usize>)>;
type Row<'e> = Vec<Column<'e>>;

struct Table<'e> {
    header: Row<'e>,
    rows: Vec<Row<'e>>,
}

fn parse_row<'e>(
    events: &mut impl Iterator<Item = (pulldown_cmark::Event<'e>, Range<usize>)>,
) -> Vec<Column<'e>> {
    let mut row = Vec::new();
    let mut column = Vec::new();

    for (e, src_span) in events.by_ref() {
        if let pulldown_cmark::Event::End(pulldown_cmark::TagEnd::TableCell) = e {
            row.push(column);
            column = Vec::new();
        }

        if let pulldown_cmark::Event::End(pulldown_cmark::TagEnd::TableHead) = e {
            break;
        }

        if let pulldown_cmark::Event::End(pulldown_cmark::TagEnd::TableRow) = e {
            break;
        }

        column.push((e, src_span));
    }

    row
}

fn parse_table<'e>(events: &mut impl Iterator<Item = EventIteratorItem<'e>>) -> Table<'e> {
    let mut all_events = delayed_events(events, pulldown_cmark::TagEnd::Table)
        .into_iter()
        .peekable();

    let header = parse_row(&mut all_events);

    let mut rows = Vec::new();
    while all_events.peek().is_some() {
        let row = parse_row(&mut all_events);
        rows.push(row);
    }

    Table { header, rows }
}

/// try to parse events as an alert quote block. This ill modify the events
/// to remove the parsed text that should not be rendered.
/// Assumes that the first element is a Paragraph
fn parse_alerts<'a>(
    alerts: &'a AlertBundle,
    events: &mut Vec<(pulldown_cmark::Event<'_>, Range<usize>)>,
) -> Option<&'a Alert> {
    // no point in parsing if there are no alerts to render
    if !alerts.is_empty() {
        let mut alert_ident = "".to_owned();
        let mut alert_ident_ends_at = 0;
        let mut has_extra_line = false;

        for (i, (e, _src_span)) in events.iter().enumerate() {
            if let pulldown_cmark::Event::End(_) = e {
                // > [!TIP]
                // >
                // > Detect the first paragraph
                // In this case the next text will be within a paragraph so it is better to remove
                // the entire paragraph
                alert_ident_ends_at = i;
                has_extra_line = true;
                break;
            }

            if let pulldown_cmark::Event::SoftBreak = e {
                // > [!NOTE]
                // > this is valid and will produce a soft break
                alert_ident_ends_at = i;
                break;
            }

            if let pulldown_cmark::Event::HardBreak = e {
                // > [!NOTE]<whitespace>
                // > this is valid and will produce a hard break
                alert_ident_ends_at = i;
                break;
            }

            if let pulldown_cmark::Event::Text(text) = e {
                alert_ident += text;
            }
        }

        let alert = alerts.try_get_alert(&alert_ident);

        if alert.is_some() {
            // remove the text that identifies it as an alert so that it won't end up in the
            // render
            //
            // FIMXE: performance improvement potential
            if has_extra_line {
                for _ in 0..=alert_ident_ends_at {
                    events.remove(0);
                }
            } else {
                for _ in 0..alert_ident_ends_at {
                    // the first element must be kept as it _should_ be Paragraph
                    events.remove(1);
                }
            }
        }

        alert
    } else {
        None
    }
}

/// Supported pulldown_cmark options
fn parser_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
}

pub struct CommonMarkViewerInternal {
    source_id: Id,
    curr_table: usize,
    text_style: crate::Style,
    list: List,
    link: Option<crate::Link>,
    image: Option<crate::Image>,
    should_insert_newline: bool,
    fenced_code_block: Option<crate::FencedCodeBlock>,
    is_list_item: bool,
    is_table: bool,
    is_blockquote: bool,
    checkbox_events: Vec<CheckboxClickEvent>,
}

pub(crate) struct CheckboxClickEvent {
    pub(crate) checked: bool,
    pub(crate) span: Range<usize>,
}

fn merge_math_events_to_cmark<'a>(
    events: impl Iterator<Item = (pulldown_cmark::Event<'a>, Range<usize>)>,
) -> Vec<(pulldown_cmark::Event<'a>, Range<usize>)> {
    let mut merged = Vec::new();
    let mut iter = events.peekable();

    while let Some((event, span)) = iter.next() {
        match event {
            pulldown_cmark::Event::Text(ref t) if t.starts_with("$$") => {
                if t.len() >= 4 && t.ends_with("$$") {
                    merged.push((pulldown_cmark::Event::Text(t.clone()), span));
                } else {
                    let mut math_content = t.to_string();
                    let mut end_span = span.clone();
                    let mut found_close = false;

                    while let Some((next_ev, next_span)) = iter.peek() {
                        match next_ev {
                            pulldown_cmark::Event::Text(nt) => {
                                math_content.push_str(nt);
                                if nt.ends_with("$$") {
                                    end_span = next_span.clone();
                                    found_close = true;
                                    iter.next();
                                    break;
                                }
                            }
                            pulldown_cmark::Event::SoftBreak | pulldown_cmark::Event::HardBreak => {
                                math_content.push('\n');
                            }
                            _ => {}
                        }
                        end_span = next_span.clone();
                        iter.next();
                    }

                    if found_close {
                        merged.push((
                            pulldown_cmark::Event::Text(math_content.into()),
                            span.start..end_span.end,
                        ));
                    } else {
                        merged.push((pulldown_cmark::Event::Text(t.clone()), span));
                    }
                }
            }
            pulldown_cmark::Event::Text(ref t) if t.starts_with('$') => {
                if t.len() >= 2 && t.ends_with('$') && !t.starts_with("$$") {
                    merged.push((pulldown_cmark::Event::Text(t.clone()), span));
                } else if t.len() > 1 && !t.chars().nth(1).unwrap().is_whitespace() {
                    let mut math_content = t.to_string();
                    let mut end_span = span.clone();
                    let mut found_close = false;

                    while let Some((next_ev, next_span)) = iter.peek() {
                        match next_ev {
                            pulldown_cmark::Event::Text(nt) => {
                                math_content.push_str(nt);
                                if nt.ends_with('$') {
                                    end_span = next_span.clone();
                                    found_close = true;
                                    iter.next();
                                    break;
                                }
                            }
                            pulldown_cmark::Event::SoftBreak | pulldown_cmark::Event::HardBreak => {
                                math_content.push(' ');
                            }
                            _ => {}
                        }
                        end_span = next_span.clone();
                        iter.next();
                    }

                    if found_close {
                        merged.push((
                            pulldown_cmark::Event::Text(math_content.into()),
                            span.start..end_span.end,
                        ));
                    } else {
                        merged.push((pulldown_cmark::Event::Text(t.clone()), span));
                    }
                } else {
                    merged.push((pulldown_cmark::Event::Text(t.clone()), span));
                }
            }
            _ => {
                merged.push((event, span));
            }
        }
    }

    merged
}

enum MathSegment<'a> {
    Text(&'a str),
    InlineMath(&'a str),
    DisplayMath(&'a str),
}

fn parse_math_segments(input: &str) -> Vec<MathSegment<'_>> {
    let mut segments = Vec::new();
    let mut start = 0;
    let mut pos = 0;
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();

    while pos < len {
        if pos + 1 < len && chars[pos] == '$' && chars[pos + 1] == '$' {
            if pos > start {
                segments.push(MathSegment::Text(&input[get_char_range(input, start, pos)]));
            }
            let block_start = pos;
            pos += 2;
            let mut found_close = false;
            while pos + 1 < len {
                if chars[pos] == '$' && chars[pos + 1] == '$' {
                    found_close = true;
                    break;
                }
                pos += 1;
            }
            if found_close {
                let math_content = &input[get_char_range(input, block_start + 2, pos)];
                segments.push(MathSegment::DisplayMath(math_content));
                pos += 2;
                start = pos;
            } else {
                pos = block_start + 1;
                start = block_start;
            }
        } else if chars[pos] == '$' {
            if pos + 1 < len && !chars[pos + 1].is_whitespace() {
                if pos > start {
                    segments.push(MathSegment::Text(&input[get_char_range(input, start, pos)]));
                }
                let math_start = pos;
                pos += 1;
                let mut found_close = false;
                while pos < len {
                    if chars[pos] == '\n' {
                        break;
                    }
                    if chars[pos] == '$' && pos > 0 && !chars[pos - 1].is_whitespace() {
                        found_close = true;
                        break;
                    }
                    pos += 1;
                }
                if found_close {
                    let math_content = &input[get_char_range(input, math_start + 1, pos)];
                    segments.push(MathSegment::InlineMath(math_content));
                    pos += 1;
                    start = pos;
                } else {
                    pos = math_start + 1;
                    start = math_start;
                }
            } else {
                pos += 1;
            }
        } else {
            pos += 1;
        }
    }

    if start < len {
        segments.push(MathSegment::Text(&input[get_char_range(input, start, len)]));
    }

    segments
}

fn get_char_range(s: &str, char_start: usize, char_end: usize) -> std::ops::Range<usize> {
    let mut byte_start = 0;
    let mut byte_end = 0;
    for (i, (byte_idx, _)) in s.char_indices().enumerate() {
        if i == char_start {
            byte_start = byte_idx;
        }
        if i == char_end {
            byte_end = byte_idx;
            break;
        }
    }
    if char_end >= s.chars().count() {
        byte_end = s.len();
    }
    byte_start..byte_end
}

impl CommonMarkViewerInternal {
    pub fn new(source_id: Id) -> Self {
        Self {
            source_id,
            curr_table: 0,
            text_style: crate::Style::default(),
            list: List::default(),
            link: None,
            image: None,
            should_insert_newline: true,
            is_list_item: false,
            fenced_code_block: None,
            is_table: false,
            is_blockquote: false,
            checkbox_events: Vec::new(),
        }
    }
}

impl CommonMarkViewerInternal {
    /// Be aware that this acquires egui::Context internally.
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        text: &str,
        populate_split_points: bool,
    ) -> (egui::InnerResponse<()>, Vec<CheckboxClickEvent>) {
        let max_width = options.max_width(ui);
        let layout = egui::Layout::left_to_right(egui::Align::BOTTOM).with_main_wrap(true);

        let re = ui.allocate_ui_with_layout(egui::vec2(max_width, 0.0), layout, |ui| {
            ui.set_max_width(max_width);
            ui.spacing_mut().item_spacing.x = 0.0;
            let height = ui.text_style_height(&TextStyle::Body);
            ui.set_row_height(height);

            let preprocessed_text = preprocess_math_blocks(text);
            let raw_events = pulldown_cmark::Parser::new_ext(&preprocessed_text, parser_options())
                .into_offset_iter();
            let merged_events = merge_math_events_to_cmark(raw_events);
            let mut events = merged_events.into_iter().enumerate();

            while let Some((index, (e, src_span))) = events.next() {
                let start_position = ui.next_widget_position();
                let is_element_end = matches!(e, pulldown_cmark::Event::End(_));
                let should_add_split_point = self.list.is_inside_a_list() && is_element_end;

                self.process_event(ui, &mut events, e, src_span, cache, options, max_width);

                if populate_split_points && should_add_split_point {
                    let scroll_cache = cache.scroll(&self.source_id);
                    let end_position = ui.next_widget_position();

                    let split_point_exists = scroll_cache
                        .split_points
                        .iter()
                        .any(|(i, _, _)| *i == index);

                    if !split_point_exists {
                        scroll_cache
                            .split_points
                            .push((index, start_position, end_position));
                    }
                }
            }

            cache.scroll(&self.source_id).page_size = Some(ui.next_widget_position().to_vec2());
        });
        (re, std::mem::take(&mut self.checkbox_events))
    }

    pub(crate) fn show_scrollable(
        &mut self,
        ui: &mut egui::Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        text: &str,
    ) {
        let available_size = ui.available_size();
        let scroll_id = self.source_id.with("_scroll_area");

        let Some(page_size) = cache.scroll(&self.source_id).page_size else {
            egui::ScrollArea::vertical()
                .id_source(scroll_id)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    self.show(ui, cache, options, text, true);
                });
            // Prevent repopulating points twice at startup
            cache.scroll(&self.source_id).available_size = available_size;
            return;
        };

        let preprocessed_text = preprocess_math_blocks(text);
        let raw_events = pulldown_cmark::Parser::new_ext(&preprocessed_text, parser_options())
            .into_offset_iter();
        let events = merge_math_events_to_cmark(raw_events);

        let num_rows = events.len();

        egui::ScrollArea::vertical()
            .id_source(scroll_id)
            // Elements have different widths, so the scroll area cannot try to shrink to the
            // content, as that will mean that the scroll bar will move when loading elements
            // with different widths.
            .auto_shrink([false, true])
            .show_viewport(ui, |ui, viewport| {
                ui.set_height(page_size.y);
                let layout = egui::Layout::left_to_right(egui::Align::BOTTOM).with_main_wrap(true);

                let max_width = options.max_width(ui);
                ui.allocate_ui_with_layout(egui::vec2(max_width, 0.0), layout, |ui| {
                    ui.set_max_width(max_width);
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let scroll_cache = cache.scroll(&self.source_id);

                    // finding the first element that's not in the viewport anymore
                    let (first_event_index, _, first_end_position) = scroll_cache
                        .split_points
                        .iter()
                        .filter(|(_, _, end_position)| end_position.y < viewport.min.y)
                        .nth_back(1)
                        .copied()
                        .unwrap_or((0, Pos2::ZERO, Pos2::ZERO));

                    // finding the last element that's just outside the viewport
                    let last_event_index = scroll_cache
                        .split_points
                        .iter()
                        .filter(|(_, start_position, _)| start_position.y > viewport.max.y)
                        .nth(1)
                        .map(|(index, _, _)| *index)
                        .unwrap_or(num_rows);

                    ui.allocate_space(first_end_position.to_vec2());

                    // only rendering the elements that are inside the viewport
                    let mut events = events
                        .into_iter()
                        .enumerate()
                        .skip(first_event_index)
                        .take(last_event_index - first_event_index);

                    while let Some((_, (e, src_span))) = events.next() {
                        self.process_event(ui, &mut events, e, src_span, cache, options, max_width);
                    }
                });
            });

        // Forcing full re-render to repopulate split points for the new size
        let scroll_cache = cache.scroll(&self.source_id);
        if available_size != scroll_cache.available_size {
            scroll_cache.available_size = available_size;
            scroll_cache.page_size = None;
            scroll_cache.split_points.clear();
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn process_event<'e>(
        &mut self,
        ui: &mut Ui,
        events: &mut impl Iterator<Item = EventIteratorItem<'e>>,
        event: pulldown_cmark::Event,
        src_span: Range<usize>,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
    ) {
        self.event(ui, event, src_span, cache, options, max_width);

        self.item_list_wrapping(events, max_width, cache, options, ui);
        self.fenced_code_block(events, max_width, cache, options, ui);
        self.table(events, cache, options, ui, max_width);
        self.blockquote(events, max_width, cache, options, ui);
    }

    fn item_list_wrapping<'e>(
        &mut self,
        events: &mut impl Iterator<Item = EventIteratorItem<'e>>,
        max_width: f32,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        ui: &mut Ui,
    ) {
        if self.is_list_item {
            self.is_list_item = false;

            let item_events = delayed_events_list_item(events);
            let mut events_iter = item_events.into_iter().enumerate();

            // Required to ensure that the content of the list item is aligned with
            // the * or - when wrapping
            ui.horizontal_wrapped(|ui| {
                while let Some((_, (e, src_span))) = events_iter.next() {
                    self.process_event(
                        ui,
                        &mut events_iter,
                        e,
                        src_span,
                        cache,
                        options,
                        max_width,
                    );
                }
            });
        }
    }

    fn blockquote<'e>(
        &mut self,
        events: &mut impl Iterator<Item = EventIteratorItem<'e>>,
        max_width: f32,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        ui: &mut Ui,
    ) {
        if self.is_blockquote {
            let mut collected_events = delayed_events(events, pulldown_cmark::TagEnd::BlockQuote);

            if let Some(alert) = parse_alerts(&options.alerts, &mut collected_events) {
                alert.ui(ui, |ui| {
                    for (event, src_span) in collected_events.into_iter() {
                        self.event(ui, event, src_span, cache, options, max_width);
                    }
                })
            } else {
                if self.should_insert_newline {
                    newline(ui)
                }
                blockquote(ui, ui.visuals().weak_text_color(), |ui| {
                    self.text_style.quote = true;
                    for (event, src_span) in collected_events {
                        self.event(ui, event, src_span, cache, options, max_width);
                    }
                    self.text_style.quote = false;
                });
            }

            if self.should_insert_newline {
                newline(ui);
            }

            self.is_blockquote = false;
        }
    }

    fn fenced_code_block<'e>(
        &mut self,
        events: &mut impl Iterator<Item = EventIteratorItem<'e>>,
        max_width: f32,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        ui: &mut Ui,
    ) {
        while self.fenced_code_block.is_some() {
            if let Some((_, (e, src_span))) = events.next() {
                self.event(ui, e, src_span, cache, options, max_width);
            } else {
                break;
            }
        }
    }

    fn table<'e>(
        &mut self,
        events: &mut impl Iterator<Item = EventIteratorItem<'e>>,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        ui: &mut Ui,
        max_width: f32,
    ) {
        if self.is_table {
            newline(ui);

            let id = self.source_id.with(self.curr_table);
            self.curr_table += 1;

            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_max_width(max_width);
                let Table { header, rows } = parse_table(events);

                egui::ScrollArea::horizontal()
                    .id_source(id)
                    .max_width(max_width)
                    .show(ui, |ui| {
                        egui::Grid::new(id.with("grid"))
                            .striped(true)
                            .show(ui, |ui| {
                                for col in header {
                                    ui.horizontal(|ui| {
                                        for (e, src_span) in col {
                                            self.should_insert_newline = false;
                                            self.event(ui, e, src_span, cache, options, max_width);
                                        }
                                    });
                                }

                                ui.end_row();

                                for row in rows {
                                    for col in row {
                                        ui.horizontal(|ui| {
                                            for (e, src_span) in col {
                                                self.should_insert_newline = false;
                                                self.event(
                                                    ui, e, src_span, cache, options, max_width,
                                                );
                                            }
                                        });
                                    }

                                    ui.end_row();
                                }
                            });
                    });
            });

            self.is_table = false;
            self.should_insert_newline = true;
            newline(ui);
        }
    }

    fn event(
        &mut self,
        ui: &mut Ui,
        event: pulldown_cmark::Event,
        src_span: Range<usize>,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
    ) {
        match event {
            pulldown_cmark::Event::Start(tag) => self.start_tag(ui, tag, options),
            pulldown_cmark::Event::End(tag) => self.end_tag(ui, tag, cache, options, max_width),
            pulldown_cmark::Event::Text(text) => {
                self.event_text(text, ui, cache);
            }
            pulldown_cmark::Event::Code(text) => {
                self.text_style.code = true;
                self.event_text(text, ui, cache);
                self.text_style.code = false;
            }
            pulldown_cmark::Event::InlineHtml(_) => {}
            pulldown_cmark::Event::Html(_) => {}
            pulldown_cmark::Event::FootnoteReference(footnote) => {
                footnote_start(ui, &footnote);
            }
            pulldown_cmark::Event::SoftBreak => {
                soft_break(ui);
            }
            pulldown_cmark::Event::HardBreak => newline(ui),
            pulldown_cmark::Event::Rule => {
                newline(ui);
                ui.add(egui::Separator::default().horizontal());
                // This does not add a new line, but instead ends the separator
                newline(ui);
            }
            pulldown_cmark::Event::TaskListMarker(mut checkbox) => {
                if options.mutable {
                    if ui
                        .add(egui::Checkbox::without_text(&mut checkbox))
                        .clicked()
                    {
                        self.checkbox_events.push(CheckboxClickEvent {
                            checked: checkbox,
                            span: src_span,
                        });
                    }
                } else {
                    ui.add(ImmutableCheckbox::without_text(&mut checkbox));
                }
            }
        }
    }

    fn event_text(&mut self, text: CowStr, ui: &mut Ui, cache: &mut CommonMarkCache) {
        if self.fenced_code_block.is_some() || self.image.is_some() {
            let rich_text = self.text_style.to_richtext(ui, &text);
            if let Some(image) = &mut self.image {
                image.alt_text.push(rich_text);
            } else if let Some(block) = &mut self.fenced_code_block {
                block.content.push_str(&text);
            }
            return;
        }

        let segments = parse_math_segments(&text);
        for segment in segments {
            match segment {
                MathSegment::Text(t) => {
                    let rich_text = self.text_style.to_richtext(ui, t);
                    if let Some(link) = &mut self.link {
                        link.text.push(rich_text);
                    } else {
                        ui.label(rich_text);
                    }
                }
                MathSegment::InlineMath(m) => {
                    let decoded_m = decode_math_placeholders(m);
                    let font_size = ui
                        .style()
                        .text_styles
                        .get(&egui::TextStyle::Body)
                        .map_or(14.0, |d| d.size);
                    let text_color = if ui.visuals().dark_mode {
                        egui::Color32::WHITE
                    } else {
                        ui.visuals().text_color()
                    };

                    // Create a unique hash for the LaTeX content, font size, and text color
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    use std::hash::Hash;
                    use std::hash::Hasher;
                    decoded_m.hash(&mut hasher);
                    (font_size as u32).hash(&mut hasher);
                    text_color.hash(&mut hasher);
                    let hash = hasher.finish();

                    let uri = format!("bytes://math_{}.svg", hash);

                    // Get or compile SVG bytes
                    let bytes = cache.math_cache.entry(hash).or_insert_with(|| {
                        match render_math_to_svg(&decoded_m, font_size, text_color) {
                            Ok(svg_string) => svg_string.into_bytes(),
                            Err(_) => Vec::new(),
                        }
                    });

                    if !bytes.is_empty() {
                        let image = egui::Image::from_bytes(uri, bytes.clone())
                            .fit_to_original_size(1.0)
                            .max_width(ui.available_width());
                        ui.add(image);
                    } else {
                        // Fallback to plain text on error
                        let mut rich_text = self.text_style.to_richtext(ui, &decoded_m);
                        if ui.visuals().dark_mode {
                            rich_text = rich_text.color(egui::Color32::WHITE);
                        }
                        if let Some(link) = &mut self.link {
                            link.text.push(rich_text);
                        } else {
                            ui.label(rich_text);
                        }
                    }
                }
                MathSegment::DisplayMath(m) => {
                    let decoded_m = decode_math_placeholders(m);
                    let font_size = ui
                        .style()
                        .text_styles
                        .get(&egui::TextStyle::Body)
                        .map_or(14.0, |d| d.size)
                        * 1.2;
                    let text_color = if ui.visuals().dark_mode {
                        egui::Color32::WHITE
                    } else {
                        ui.visuals().text_color()
                    };

                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    use std::hash::Hash;
                    use std::hash::Hasher;
                    decoded_m.hash(&mut hasher);
                    (font_size as u32).hash(&mut hasher);
                    text_color.hash(&mut hasher);
                    let hash = hasher.finish();

                    let uri = format!("bytes://math_{}.svg", hash);

                    let bytes = cache.math_cache.entry(hash).or_insert_with(|| {
                        match render_math_to_svg(&decoded_m, font_size, text_color) {
                            Ok(svg_string) => svg_string.into_bytes(),
                            Err(_) => Vec::new(),
                        }
                    });

                    if !bytes.is_empty() {
                        if self.should_insert_newline {
                            newline(ui);
                        }
                        ui.horizontal(|ui| {
                            ui.label("    ");
                            let image = egui::Image::from_bytes(uri, bytes.clone())
                                .fit_to_original_size(1.0)
                                .max_width(ui.available_width());
                            ui.add(image);
                        });
                        newline(ui);
                    } else {
                        // Fallback to plain text on error
                        if self.should_insert_newline {
                            newline(ui);
                        }
                        ui.horizontal(|ui| {
                            ui.label("    ");
                            let mut rich_text =
                                self.text_style.to_richtext(ui, &decoded_m).monospace();
                            if ui.visuals().dark_mode {
                                rich_text = rich_text.color(egui::Color32::WHITE);
                            }
                            ui.label(rich_text);
                        });
                        newline(ui);
                    }
                }
            }
        }
    }

    fn start_tag(&mut self, ui: &mut Ui, tag: pulldown_cmark::Tag, options: &CommonMarkOptions) {
        match tag {
            pulldown_cmark::Tag::Paragraph => {
                if self.should_insert_newline {
                    newline(ui);
                }
                self.should_insert_newline = true;
            }
            pulldown_cmark::Tag::Heading { level, .. } => {
                newline(ui);
                self.text_style.heading = Some(match level {
                    HeadingLevel::H1 => 0,
                    HeadingLevel::H2 => 1,
                    HeadingLevel::H3 => 2,
                    HeadingLevel::H4 => 3,
                    HeadingLevel::H5 => 4,
                    HeadingLevel::H6 => 5,
                });
            }
            pulldown_cmark::Tag::BlockQuote => {
                self.is_blockquote = true;
            }
            pulldown_cmark::Tag::CodeBlock(c) => {
                if let pulldown_cmark::CodeBlockKind::Fenced(lang) = c {
                    self.fenced_code_block = Some(crate::FencedCodeBlock {
                        lang: lang.to_string(),
                        content: "".to_string(),
                    });

                    if self.should_insert_newline {
                        newline(ui);
                    }
                }

                self.text_style.code = true;
            }
            pulldown_cmark::Tag::List(point) => {
                if let Some(number) = point {
                    self.list.start_level_with_number(number);
                } else {
                    self.list.start_level_without_number();
                }
            }
            pulldown_cmark::Tag::Item => {
                self.is_list_item = true;
                self.should_insert_newline = false;
                self.list.start_item(ui, options);
            }
            pulldown_cmark::Tag::FootnoteDefinition(note) => {
                self.should_insert_newline = false;
                footnote(ui, &note);
            }
            pulldown_cmark::Tag::Table(_) => {
                self.is_table = true;
            }
            pulldown_cmark::Tag::TableHead => {}
            pulldown_cmark::Tag::TableRow => {}
            pulldown_cmark::Tag::TableCell => {}
            pulldown_cmark::Tag::Emphasis => {
                self.text_style.emphasis = true;
            }
            pulldown_cmark::Tag::Strong => {
                self.text_style.strong = true;
            }
            pulldown_cmark::Tag::Strikethrough => {
                self.text_style.strikethrough = true;
            }
            pulldown_cmark::Tag::Link { dest_url, .. } => {
                self.link = Some(crate::Link {
                    destination: dest_url.to_string(),
                    text: Vec::new(),
                });
            }
            pulldown_cmark::Tag::Image { dest_url, .. } => {
                self.image = Some(crate::Image::new(&dest_url, options));
            }
            pulldown_cmark::Tag::HtmlBlock => {}
            pulldown_cmark::Tag::MetadataBlock(_) => {}
        }
    }

    fn end_tag(
        &mut self,
        ui: &mut Ui,
        tag: pulldown_cmark::TagEnd,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
    ) {
        match tag {
            pulldown_cmark::TagEnd::Paragraph => {
                newline(ui);
            }
            pulldown_cmark::TagEnd::Heading { .. } => {
                newline(ui);
                self.text_style.heading = None;
            }
            pulldown_cmark::TagEnd::BlockQuote => {}
            pulldown_cmark::TagEnd::CodeBlock => {
                self.end_code_block(ui, cache, options, max_width);
            }
            pulldown_cmark::TagEnd::List(_) => {
                self.list.end_level(ui);

                if !self.list.is_inside_a_list() {
                    self.should_insert_newline = true;
                }
            }
            pulldown_cmark::TagEnd::Item => {}
            pulldown_cmark::TagEnd::FootnoteDefinition => {}
            pulldown_cmark::TagEnd::Table => {}
            pulldown_cmark::TagEnd::TableHead => {}
            pulldown_cmark::TagEnd::TableRow => {}
            pulldown_cmark::TagEnd::TableCell => {
                // Ensure space between cells
                ui.label("  ");
            }
            pulldown_cmark::TagEnd::Emphasis => {
                self.text_style.emphasis = false;
            }
            pulldown_cmark::TagEnd::Strong => {
                self.text_style.strong = false;
            }
            pulldown_cmark::TagEnd::Strikethrough => {
                self.text_style.strikethrough = false;
            }
            pulldown_cmark::TagEnd::Link => {
                if let Some(link) = self.link.take() {
                    link.end(ui, cache);
                }
            }
            pulldown_cmark::TagEnd::Image => {
                if let Some(image) = self.image.take() {
                    image.end(ui, options);
                }
            }
            pulldown_cmark::TagEnd::HtmlBlock => {}
            pulldown_cmark::TagEnd::MetadataBlock(_) => {}
        }
    }

    fn end_code_block(
        &mut self,
        ui: &mut Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
    ) {
        if let Some(block) = self.fenced_code_block.take() {
            block.end(ui, cache, options, max_width);
            self.text_style.code = false;
            if self.should_insert_newline {
                newline(ui);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_math_segments() {
        let segments = parse_math_segments("Hello $x^2$ world $$y = mx + b$$ end");
        assert_eq!(segments.len(), 5);

        match &segments[0] {
            MathSegment::Text(t) => assert_eq!(*t, "Hello "),
            _ => panic!("Expected text"),
        }
        match &segments[1] {
            MathSegment::InlineMath(m) => assert_eq!(*m, "x^2"),
            _ => panic!("Expected inline math"),
        }
        match &segments[2] {
            MathSegment::Text(t) => assert_eq!(*t, " world "),
            _ => panic!("Expected text"),
        }
        match &segments[3] {
            MathSegment::DisplayMath(m) => assert_eq!(*m, "y = mx + b"),
            _ => panic!("Expected display math"),
        }
        match &segments[4] {
            MathSegment::Text(t) => assert_eq!(*t, " end"),
            _ => panic!("Expected text"),
        }
    }

    #[test]
    fn test_multiline_inline_math_rejection() {
        let input = "echo $USER\n```bash\nflatpak install flathub com.github.tchx84.Flatseal\n```\necho $USER";
        let preprocessed = preprocess_math_blocks(input);
        assert_eq!(preprocessed, input);
    }

    #[test]
    fn test_benchmark_events() {
        let input = r"
# Display Math

$$
E = mc^2
$$

---

$$
f(x)=x^2+2x+1
$$
";
        let parser = pulldown_cmark::Parser::new_ext(input, pulldown_cmark::Options::all())
            .into_offset_iter();
        let merged = merge_math_events_to_cmark(parser);
        for (event, _) in merged {
            println!("Merged Event: {:?}", event);
        }
    }

    #[test]
    fn test_ratex_compilation() {
        let latex = r"E = mc^2";
        let svg = render_math_to_svg(latex, 14.0, egui::Color32::BLACK).unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("<path"));
    }

    #[test]
    fn test_sans_serif_math() {
        let latex = r"E = mc^2";
        let formula = ratex_parser::parse(latex).unwrap();
        let layout_options = ratex_layout::LayoutOptions::default();
        let layout_box = ratex_layout::layout(&formula, &layout_options);
        let mut display_list = ratex_layout::to_display_list(&layout_box);
        for item in &mut display_list.items {
            if let ratex_types::display_item::DisplayItem::GlyphPath { font, .. } = item {
                match font.as_str() {
                    "Math-Italic" | "Main-Italic" => *font = "SansSerif-Italic".to_string(),
                    "Main-Regular" => *font = "SansSerif-Regular".to_string(),
                    "Main-Bold" | "Math-BoldItalic" | "Main-BoldItalic" => {
                        *font = "SansSerif-Bold".to_string()
                    }
                    _ => {}
                }
            }
        }
        let options = ratex_svg::SvgOptions {
            font_size: 14.0,
            padding: 2.0,
            embed_glyphs: true,
            ..Default::default()
        };
        let svg = ratex_svg::render_to_svg(&display_list, &options);
        assert!(svg.contains("<svg"));
        assert!(svg.contains("<path"));
        eprintln!("SVG generated successfully! Length = {}", svg.len());
    }

    #[test]
    fn test_math_dark_mode_white_symbols() {
        let latex = r"E = mc^2";
        let svg = render_math_to_svg(latex, 14.0, egui::Color32::WHITE).unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("rgba(255,255,255,1)"));
        assert!(!svg.contains("rgba(0,0,0,1)"));
    }

    #[test]
    fn test_matrix_rendering() {
        let latex = r"\begin{matrix} 1 & 2 \\ 3 & 4 \end{matrix}";
        let svg = render_math_to_svg(latex, 14.0, egui::Color32::BLACK).unwrap();
        println!("Matrix SVG:\n{}", svg);
    }

    #[test]
    fn test_multiline_equation_rendering() {
        let latex = "a = b \\\\\nc = d";
        let svg = render_math_to_svg(latex, 14.0, egui::Color32::BLACK).unwrap();
        println!("Multiline SVG:\n{}", svg);
    }

    #[test]
    fn test_pipeline_matrix_rendering() {
        let input = "$$\n\\begin{matrix}\n1 & 2 \\\\\n3 & 4\n\\end{matrix}\n$$";
        let preprocessed = preprocess_math_blocks(input);
        let parser = pulldown_cmark::Parser::new_ext(&preprocessed, pulldown_cmark::Options::all())
            .into_offset_iter();
        let merged = merge_math_events_to_cmark(parser);
        for (event, _) in merged {
            if let pulldown_cmark::Event::Text(t) = event {
                println!("Text Event: {:?}", t);
                let segments = parse_math_segments(&t);
                for seg in segments {
                    if let MathSegment::DisplayMath(m) = seg {
                        let decoded_m = decode_math_placeholders(m);
                        println!("Display Math: {:?}", decoded_m);
                        let svg =
                            render_math_to_svg(&decoded_m, 14.0, egui::Color32::BLACK).unwrap();
                        println!("SVG:\n{}", svg);
                    }
                }
            }
        }
    }

    #[test]
    fn test_preprocess_math_blocks() {
        let input = "This costs $5 and that costs $10. But here is inline math: $x \\approx 2$.";
        let preprocessed = preprocess_math_blocks(input);
        assert_eq!(
            preprocessed,
            "This costs $5 and that costs $10. But here is inline math: $x \u{E000}approx 2$."
        );
    }

    #[test]
    fn test_pipeline_limit_rendering() {
        let input = r"Limit: $\lim_{x\to0}\frac{\sin x}{x}=1$";
        let preprocessed = preprocess_math_blocks(input);
        println!("Preprocessed: {:?}", preprocessed);
        let parser = pulldown_cmark::Parser::new_ext(&preprocessed, pulldown_cmark::Options::all())
            .into_offset_iter();
        let merged = merge_math_events_to_cmark(parser);
        for (event, _) in merged {
            if let pulldown_cmark::Event::Text(t) = event {
                println!("Text Event: {:?}", t);
                let segments = parse_math_segments(&t);
                for seg in segments {
                    if let MathSegment::InlineMath(m) = seg {
                        let decoded_m = decode_math_placeholders(m);
                        println!("Inline Math: {:?}", decoded_m);
                        let svg = render_math_to_svg(&decoded_m, 14.0, egui::Color32::BLACK);
                        println!("SVG Result: {:?}", svg.is_ok());
                        if let Err(e) = svg {
                            println!("SVG Error: {}", e);
                        }
                    }
                }
            }
        }
    }
}
