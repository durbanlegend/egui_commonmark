use std::iter::Peekable;
use std::ops::Range;

use crate::{CommonMarkCache, CommonMarkOptions};

use egui::{self, Id, Pos2, TextStyle, Ui};

use crate::List;
use egui_commonmark_backend::elements::*;
use egui_commonmark_backend::misc::*;
use egui_commonmark_backend::pulldown::*;
use pulldown_cmark::{CowStr, HeadingLevel};

/// Newline logic is constructed by the following:
/// All elements try to insert a newline before them (if they are allowed)
/// and end their own line.
struct Newline {
    /// Whether a newline should not be inserted before a widget. This is only for
    /// the first widget.
    should_not_start_newline_forced: bool,
    /// Whether an element should insert a newline before it
    should_start_newline: bool,
    /// Whether an element should end it's own line using a newline
    /// This will have to be set to false in cases such as when blocks are within
    /// a list.
    should_end_newline: bool,
    /// only false when the widget is the last one.
    should_end_newline_forced: bool,
}

impl Default for Newline {
    fn default() -> Self {
        Self {
            should_not_start_newline_forced: true,
            should_start_newline: true,
            should_end_newline: true,
            should_end_newline_forced: true,
        }
    }
}

impl Newline {
    pub fn can_insert_end(&self) -> bool {
        self.should_end_newline && self.should_end_newline_forced
    }

    pub fn can_insert_start(&self) -> bool {
        self.should_start_newline && !self.should_not_start_newline_forced
    }

    pub fn try_insert_start(&self, ui: &mut Ui) {
        if self.can_insert_start() {
            newline(ui);
        }
    }

    pub fn try_insert_end(&self, ui: &mut Ui) {
        if self.can_insert_end() {
            newline(ui);
        }
    }
}

#[derive(Default)]
struct DefinitionList {
    is_first_item: bool,
    is_def_list_def: bool,
}

pub struct CommonMarkViewerInternal {
    curr_table: usize,
    text_style: Style,
    list: List,
    link: Option<Link>,
    image: Option<Image>,
    line: Newline,
    code_block: Option<CodeBlock>,

    /// Only populated if the html_fn option has been set
    html_block: String,
    is_list_item: bool,
    def_list: DefinitionList,
    is_table: bool,
    is_blockquote: bool,
    checkbox_events: Vec<CheckboxClickEvent>,
    deferred_scroll_to_heading: Option<String>,
}

pub(crate) struct CheckboxClickEvent {
    pub(crate) checked: bool,
    pub(crate) span: Range<usize>,
}

impl CommonMarkViewerInternal {
    pub fn new() -> Self {
        Self {
            curr_table: 0,
            text_style: Style::default(),
            list: List::default(),
            link: None,
            image: None,
            line: Newline::default(),
            is_list_item: false,
            def_list: Default::default(),
            code_block: None,
            html_block: String::new(),
            is_table: false,
            is_blockquote: false,
            checkbox_events: Vec::new(),
            deferred_scroll_to_heading: None,
        }
    }
}

fn parser_options_extras(
    is_math_enabled: bool,
    is_scroll_to_heading_enabled: bool,
) -> pulldown_cmark::Options {
    let mut result = parser_options();
    if is_math_enabled {
        result |= pulldown_cmark::Options::ENABLE_MATH;
    }
    if is_scroll_to_heading_enabled {
        result |= pulldown_cmark::Options::ENABLE_HEADING_ATTRIBUTES;
    }
    result
}

impl CommonMarkViewerInternal {
    /// Be aware that this acquires egui::Context internally.
    /// If split Id is provided then split points will be populated
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        text: &str,
        split_points_id: Option<Id>,
    ) -> (egui::InnerResponse<()>, Vec<CheckboxClickEvent>) {
        let max_width = options.max_width(ui);
        let layout = egui::Layout::left_to_right(egui::Align::BOTTOM).with_main_wrap(true);

        let re = ui.allocate_ui_with_layout(egui::vec2(max_width, 0.0), layout, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let height = ui.text_style_height(&TextStyle::Body);
            ui.set_row_height(height);

            let mut events = pulldown_cmark::Parser::new_ext(
                text,
                parser_options_extras(options.math_fn.is_some(), options.enable_scroll_to_heading),
            )
            .into_offset_iter()
            .enumerate()
            .peekable();

            // Capture the y-coordinate of the content's top in screen space.
            // ui.next_widget_position() returns screen-space coordinates; at scroll_offset=0
            // (guaranteed by the .scroll_offset(ZERO) on the full-render ScrollArea) this
            // equals screen_top — the panel's y-offset.  By subtracting it from every
            // split_point position and from page_size.y, we normalise them to virtual
            // (content-relative) coordinates where 0 = top of document.
            //
            // This matters because viewport.min.y and viewport.max.y from show_viewport
            // are in virtual space (= scroll_offset and scroll_offset + visible_height),
            // so the split_point filters and allocate_space must use the same space.
            //
            // heading_y_positions is stored in screen-space y (NOT normalised).
            // show_scrollable uses  pos2(0, y - viewport.min.y)  with scroll_to_rect,
            // which correctly cancels the screen_top offset regardless of scroll position.
            let content_origin_y = ui.next_widget_position().y;

            while let Some((index, (e, src_span))) = events.next() {
                let start_position = ui.next_widget_position(); // screen-space (normalised below)
                // Record heading y-positions for the heading-scroll fast path.
                if let (
                    Some(sid),
                    pulldown_cmark::Event::Start(pulldown_cmark::Tag::Heading {
                        id: Some(id), ..
                    }),
                ) = (split_points_id, &e)
                {
                    // Stored in screen-space y (not normalised).  The viewport-path
                    // navigator uses  pos2(0, y - viewport.min.y)  with scroll_to_rect,
                    // which correctly cancels the screen_top offset — see show_scrollable.
                    // Use cursor.min.y (top of row) not next_widget_position().y
                    // (which returns cursor.max.y for BOTTOM-aligned layouts).
                    // scroll_to_cursor() in start_tag uses cursor.min.y in its
                    // offset formula, so we must store the same coordinate.
                    scroll_cache(cache, &sid)
                        .heading_y_positions
                        .insert(id.to_string(), ui.cursor().min.y);
                    // eprintln!("Inserted heading cursor.min.y: id={}, y={}", id.to_string(), ui.cursor().min.y);
                }
                // Split points are only safe at stateless top-level boundaries.
                // Paragraph, Heading and CodeBlock are provably safe: the renderer
                // has no pending state after them.  End(List), End(BlockQuote),
                // End(Table) etc. are excluded even at the top level because the
                // list-level stack is empty in a fresh renderer, so any Start(List)
                // consumed by an inner wrapper (item_list_wrapping / blockquote /
                // table) before the outer loop sees Start(Item) would leave the
                // stack empty and trigger the unreachable!() in start_item.
                // The guard !is_inside_a_list() prevents recording a split point
                // while inside a list (e.g. End(Paragraph) inside a list item).
                let is_safe_block_end = !self.list.is_inside_a_list()
                    && matches!(
                        e,
                        pulldown_cmark::Event::End(
                            pulldown_cmark::TagEnd::Paragraph
                                | pulldown_cmark::TagEnd::Heading { .. }
                                | pulldown_cmark::TagEnd::CodeBlock
                        )
                    );

                if events.peek().is_none() {
                    self.line.should_end_newline_forced = false;
                }

                self.process_event(ui, &mut events, e, src_span, cache, options, max_width);

                let should_add_split_point = is_safe_block_end;

                if let Some(source_id) = split_points_id
                    && should_add_split_point
                {
                    let scroll_cache = scroll_cache(cache, &source_id);
                    let end_position = ui.next_widget_position(); // screen-space

                    let split_point_exists = scroll_cache
                        .split_points
                        .iter()
                        .any(|(i, _, _)| *i == index);

                    if !split_point_exists {
                        // Normalise to virtual (content-relative) coordinates so that
                        // the positions are directly comparable to viewport.min/max.y.
                        let vstart =
                            egui::pos2(start_position.x, start_position.y - content_origin_y);
                        let vend = egui::pos2(end_position.x, end_position.y - content_origin_y);
                        scroll_cache.split_points.push((index, vstart, vend));
                        // eprintln!(
                        //     "Pushed split_point: index={index}, vstart=({},{}) vend=({},{})",
                        //     vstart.x, vstart.y, vend.x, vend.y
                        // )
                    }
                }

                if index == 0 {
                    self.line.should_not_start_newline_forced = false;
                }
            }

            // deferral to make it consistent no matter whether the target is before or after the link
            *cache.scroll_to_id_target_mut() = self.deferred_scroll_to_heading.take();

            if let Some(source_id) = split_points_id {
                // Normalise page_size.y to virtual height (total content height, 0-based).
                // ui.set_height(page_size.y) in show_scrollable uses this to set the
                // virtual document height for the scroll area, so it must not include
                // the screen_top (content_origin_y) offset.
                let final_y = ui.next_widget_position().y;
                scroll_cache(cache, &source_id).page_size =
                    Some(egui::vec2(max_width, final_y - content_origin_y));
            }
        });

        (re, std::mem::take(&mut self.checkbox_events))
    }

    pub(crate) fn show_scrollable(
        &mut self,
        source_id: Id,
        ui: &mut egui::Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        text: &str,
    ) {
        let available_size = ui.available_size();
        let scroll_id = source_id.with("_scroll_area");
        // eprintln!("scroll_id={scroll_id:?}, available_size={available_size}");

        if !options.use_viewport_cache {
            // ── Simple full-document render path ───────────────────────────────────────
            // Render the entire document on every frame; egui clips what is off-screen.
            // This gives flawless scroll accuracy and is fast enough for most documents.
            // Stale viewport-cache state from a previous cached run is cleared so that
            // re-enabling the cache later triggers a fresh full render.
            {
                let sc = scroll_cache(cache, &source_id);
                sc.page_size = None;
                sc.split_points.clear();
                sc.heading_y_positions.clear();
            }
            egui::ScrollArea::vertical()
                .id_salt(scroll_id)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    let delta =
                        std::mem::replace(&mut cache.pending_scroll_delta, egui::Vec2::ZERO);
                    if delta != egui::Vec2::ZERO {
                        ui.scroll_with_delta(delta);
                    }
                    // Passing None disables split-point recording; TOC navigation works
                    // via the original ui.scroll_to_cursor() path in start_tag().
                    self.show(ui, cache, options, text, None);
                });
            return;
        }

        let Some(page_size) = scroll_cache(cache, &source_id).page_size else {
            // Force scroll to top so that ui.next_widget_position() records positive
            // screen-space y values in split_points and heading_y_positions.
            //
            // Without this, if the cache is cleared while the ScrollArea remembers a
            // large scroll offset (e.g. after reloading from EOD), the content UI
            // starts at `screen_top - scroll_offset` which is deeply negative.
            // Those negative positions end up in split_points, and the subsequent
            // viewport render calls ui.allocate_space() with a negative size, which
            // triggers the debug assertion in placer::next_space and panics.
            //
            // egui 0.35 guarantees that scroll_offset() overrides persisted state
            // (scroll_area.rs line 742: state.offset.y = offset_y.unwrap_or(...)),
            // so this reliably resets the scroll for the layout-measurement pass.
            // The viewport render on the next frame then begins at offset 0.
            egui::ScrollArea::vertical()
                .id_salt(scroll_id)
                .scroll_offset(egui::Vec2::ZERO)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    // Apply any pending keyboard/programmatic scroll delta.
                    let delta =
                        std::mem::replace(&mut cache.pending_scroll_delta, egui::Vec2::ZERO);
                    if delta != egui::Vec2::ZERO {
                        ui.scroll_with_delta(delta);
                    }
                    self.show(ui, cache, options, text, Some(source_id));
                });
            // Prevent repopulating points twice at startup
            scroll_cache(cache, &source_id).available_size = available_size;
            return;
        };

        let events = pulldown_cmark::Parser::new_ext(
            text,
            parser_options_extras(options.math_fn.is_some(), options.enable_scroll_to_heading),
        )
        .into_offset_iter()
        .collect::<Vec<_>>();

        let num_rows = events.len();

        // Resolve scroll_to_id_target via cached heading y-positions so that
        // TOC navigation works even when the target heading is outside the
        // currently visible viewport slice.
        let pending_scroll_y: Option<f32> = {
            let slug_owned = cache.scroll_to_id_target().map(|s| s.to_owned());
            if let Some(ref slug) = slug_owned {
                let sc = scroll_cache(cache, &source_id);
                if let Some(&y) = sc.heading_y_positions.get(slug) {
                    cache.scroll_to_id_target_mut().take(); // consumed
                    Some(y)
                } else {
                    None // not yet known; keep target for the full-render path
                }
            } else {
                None
            }
        };
        let pending_delta = std::mem::replace(&mut cache.pending_scroll_delta, egui::Vec2::ZERO);

        egui::ScrollArea::vertical()
            .id_salt(scroll_id)
            // Elements have different widths, so the scroll area cannot try to shrink to the
            // content, as that will mean that the scroll bar will move when loading elements
            // with different widths.
            .auto_shrink([false, true])
            .show_viewport(ui, |ui, viewport| {
                // Apply heading jump and keyboard delta inside the scroll area.
                if let Some(y) = pending_scroll_y {
                    // heading_y_positions stores screen-space y (recorded at scroll_offset=0).
                    //
                    // scroll_to_rect formula (Align::TOP, center_factor=0):
                    //   min  = content_ui.min_rect().min.y  = screen_top - current_scroll
                    //   offset = y_rect - min
                    //   delta  = offset - item_spacing - current_scroll
                    //   target = current_scroll + delta = y_rect - screen_top - item_spacing
                    //
                    // We need target = virtual_y_heading - item_spacing, so:
                    //   y_rect = virtual_y_heading + screen_top - current_scroll
                    //          = (screen_space_y) - current_scroll
                    //          = y - viewport.min.y
                    //
                    // This formula was previously confirmed working.
                    let r = egui::Rect::from_min_size(
                        egui::pos2(0.0, y - viewport.min.y),
                        egui::Vec2::ZERO,
                    );
                    ui.scroll_to_rect(r, Some(egui::Align::TOP));
                }
                if pending_delta != egui::Vec2::ZERO {
                    ui.scroll_with_delta(pending_delta);
                }

                ui.set_height(page_size.y);
                let layout = egui::Layout::left_to_right(egui::Align::BOTTOM).with_main_wrap(true);

                let max_width = options.max_width(ui);
                ui.allocate_ui_with_layout(egui::vec2(max_width, 0.0), layout, |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let scroll_cache = scroll_cache(cache, &source_id);

                    // Rolling-segment rendering: expose content in a window of
                    // [render_above, render_below] rather than just the tight viewport.
                    //
                    // Padding by one viewport-height above and below gives three benefits:
                    //  1. The viewport is always filled even when images or other dynamic
                    //     elements expand the layout after the initial measurement pass.
                    //  2. Smooth scrolling: the rendered window shifts only when the user
                    //     scrolls outside the pad zone, rather than on every frame.
                    //  3. Window-resize reflows are absorbed: the available-size change
                    //     invalidates the cache and triggers a fresh full render, but the
                    //     larger pad prevents a blank flash during the transition.
                    //
                    // All positions are in virtual (content-relative) space (0 = top),
                    // matching viewport.min/max.y from show_viewport exactly.
                    let viewport_height = viewport.max.y - viewport.min.y;
                    let render_above = (viewport.min.y - viewport_height).max(0.0);
                    let render_below = viewport.max.y + viewport_height;

                    // Last split point whose END is before render_above (inclusive).
                    // Using .last() rather than .nth_back(N): the viewport_height pad
                    // already provides the over-render buffer, so we need no extra margin.
                    let (first_event_index, _, first_end_position) = scroll_cache
                        .split_points
                        .iter()
                        .filter(|(_, _, end_position)| end_position.y < render_above)
                        .last()
                        .copied()
                        .unwrap_or((0, Pos2::ZERO, Pos2::ZERO));

                    // First split point whose START is past render_below.
                    let last_event_index = scroll_cache
                        .split_points
                        .iter()
                        .filter(|(_, start_position, _)| start_position.y > render_below)
                        .next()
                        .map(|(index, _, _)| *index)
                        .unwrap_or(num_rows);

                    // eprintln!(
                    //     "first_end_position=({},{})",
                    //     first_end_position.x, first_end_position.y
                    // );
                    // Defensive clamp: allocate_space asserts non-negative size.
                    // Negative values should no longer occur after the scroll_offset(ZERO)
                    // fix above, but guard here to prevent panics from any future
                    // edge-case that re-introduces them.
                    let safe_end =
                        egui::pos2(first_end_position.x.max(0.0), first_end_position.y.max(0.0));
                    ui.allocate_space(safe_end.to_vec2());

                    // only rendering the elements that are inside the viewport
                    let mut events = events
                        .into_iter()
                        .enumerate()
                        .skip(first_event_index)
                        .take(last_event_index - first_event_index)
                        .peekable();

                    while let Some((i, (e, src_span))) = events.next() {
                        if events.peek().is_none() {
                            self.line.should_end_newline_forced = false;
                        }

                        self.process_event(ui, &mut events, e, src_span, cache, options, max_width);

                        if i == 0 {
                            self.line.should_not_start_newline_forced = false;
                        }
                    }
                });
            });

        // Forcing full re-render to repopulate split points for the new size
        let scroll_cache = scroll_cache(cache, &source_id);
        if available_size != scroll_cache.available_size {
            scroll_cache.available_size = available_size;
            scroll_cache.page_size = None;
            scroll_cache.split_points.clear();
            scroll_cache.heading_y_positions.clear();
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn process_event<'e>(
        &mut self,
        ui: &mut Ui,
        events: &mut Peekable<impl Iterator<Item = EventIteratorItem<'e>>>,
        event: pulldown_cmark::Event,
        src_span: Range<usize>,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
    ) {
        self.event(ui, event, src_span, cache, options, max_width);

        self.def_list_def_wrapping(events, max_width, cache, options, ui);
        self.item_list_wrapping(events, max_width, cache, options, ui);
        self.table(events, cache, options, ui, max_width);
        self.blockquote(events, max_width, cache, options, ui);
    }

    fn def_list_def_wrapping<'e>(
        &mut self,
        events: &mut Peekable<impl Iterator<Item = EventIteratorItem<'e>>>,
        max_width: f32,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        ui: &mut Ui,
    ) {
        if self.def_list.is_def_list_def {
            self.def_list.is_def_list_def = false;

            let item_events = delayed_events(events, |tag| {
                matches!(tag, pulldown_cmark::TagEnd::DefinitionListDefinition)
            });

            let mut events_iter = item_events.into_iter().enumerate().peekable();

            self.line.try_insert_start(ui);

            // Proccess a single event separately so that we do not insert spaces where we do not
            // want them
            self.line.should_start_newline = false;
            if let Some((_, (e, src_span))) = events_iter.next() {
                self.process_event(ui, &mut events_iter, e, src_span, cache, options, max_width);
            }

            ui.label(" ".repeat(options.indentation_spaces));
            self.line.should_start_newline = true;
            self.line.should_end_newline = false;
            // Required to ensure that the content is aligned with the identation
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
            self.line.should_end_newline = true;

            // Only end the definition items line if it is not the last element in the list
            if !matches!(
                events.peek(),
                Some((
                    _,
                    (
                        pulldown_cmark::Event::End(pulldown_cmark::TagEnd::DefinitionList),
                        _
                    )
                ))
            ) {
                self.line.try_insert_end(ui);
            }
        }
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
            let mut events_iter = item_events.into_iter().enumerate().peekable();

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
        events: &mut Peekable<impl Iterator<Item = EventIteratorItem<'e>>>,
        max_width: f32,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        ui: &mut Ui,
    ) {
        if self.is_blockquote {
            let mut collected_events = delayed_events(events, |tag| {
                matches!(tag, pulldown_cmark::TagEnd::BlockQuote(_))
            });
            self.line.try_insert_start(ui);

            // Currently the blockquotes are made in such a way that they need a newline at the end
            // and the start so when this is the first element in the markdown the newline must be
            // manually enabled
            self.line.should_not_start_newline_forced = false;
            if let Some(alert) = parse_alerts(&options.alerts, &mut collected_events) {
                egui_commonmark_backend::alert_ui(alert, ui, |ui| {
                    for (event, src_span) in collected_events {
                        self.event(ui, event, src_span, cache, options, max_width);
                    }
                })
            } else {
                blockquote(ui, ui.visuals().weak_text_color(), |ui| {
                    self.text_style.quote = true;
                    for (event, src_span) in collected_events {
                        self.event(ui, event, src_span, cache, options, max_width);
                    }
                    self.text_style.quote = false;
                });
            }

            if events.peek().is_none() {
                self.line.should_end_newline_forced = false;
            }

            self.line.try_insert_end(ui);
            self.is_blockquote = false;
        }
    }

    fn table<'e>(
        &mut self,
        events: &mut Peekable<impl Iterator<Item = EventIteratorItem<'e>>>,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        ui: &mut Ui,
        max_width: f32,
    ) {
        if self.is_table {
            self.line.try_insert_start(ui);
            if ui.available_width() < max_width - 1.0 {
                ui.label("\n\n");
            }

            let id = ui.id().with("_table").with(self.curr_table);
            self.curr_table += 1;

            egui::Frame::group(ui.style()).show(ui, |ui| {
                let Table { header, rows } = parse_table(events);

                ui.spacing_mut().scroll.content_margin.bottom = ui.spacing().scroll.bar_width as i8;
                egui::ScrollArea::horizontal()
                    .id_salt(id.with("_hscroll"))
                    .show(ui, |ui| {
                        egui::Grid::new(id).striped(true).show(ui, |ui| {
                            for col in header {
                                ui.horizontal(|ui| {
                                    for (e, src_span) in col {
                                        let tmp_start = std::mem::replace(
                                            &mut self.line.should_start_newline,
                                            false,
                                        );
                                        let tmp_end = std::mem::replace(
                                            &mut self.line.should_end_newline,
                                            false,
                                        );
                                        self.event(ui, e, src_span, cache, options, max_width);
                                        self.line.should_start_newline = tmp_start;
                                        self.line.should_end_newline = tmp_end;
                                    }
                                });
                            }

                            ui.end_row();

                            for row in rows {
                                for col in row {
                                    ui.horizontal(|ui| {
                                        for (e, src_span) in col {
                                            let tmp_start = std::mem::replace(
                                                &mut self.line.should_start_newline,
                                                false,
                                            );
                                            let tmp_end = std::mem::replace(
                                                &mut self.line.should_end_newline,
                                                false,
                                            );
                                            self.event(ui, e, src_span, cache, options, max_width);
                                            self.line.should_start_newline = tmp_start;
                                            self.line.should_end_newline = tmp_end;
                                        }
                                    });
                                }

                                ui.end_row();
                            }
                        });
                    });
            });

            self.is_table = false;
            if events.peek().is_none() {
                self.line.should_end_newline_forced = false;
            }

            self.line.try_insert_end(ui);
            ui.label("\n");
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
            pulldown_cmark::Event::Start(tag) => self.start_tag(ui, tag, cache, options),
            pulldown_cmark::Event::End(tag) => self.end_tag(ui, tag, cache, options, max_width),
            pulldown_cmark::Event::Text(text) => {
                self.event_text(text, src_span, ui, cache);
            }
            pulldown_cmark::Event::Code(text) => {
                self.text_style.code = true;
                self.event_text(text, src_span, ui, cache);
                self.text_style.code = false;
            }
            pulldown_cmark::Event::InlineHtml(text) => {
                self.event_text(text, src_span, ui, cache);
            }

            pulldown_cmark::Event::Html(text) => {
                if options.html_fn.is_some() {
                    self.html_block.push_str(&text);
                } else {
                    self.event_text(text, src_span, ui, cache);
                }
            }
            pulldown_cmark::Event::FootnoteReference(footnote) => {
                footnote_start(ui, &footnote);
            }
            pulldown_cmark::Event::SoftBreak => {
                soft_break(ui);
            }
            pulldown_cmark::Event::HardBreak => newline(ui),
            pulldown_cmark::Event::Rule => {
                self.line.try_insert_start(ui);
                rule(ui, self.line.can_insert_end());
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
            pulldown_cmark::Event::InlineMath(tex) => {
                if let Some(math_fn) = options.math_fn {
                    math_fn(ui, &tex, true);
                }
            }
            pulldown_cmark::Event::DisplayMath(tex) => {
                if let Some(math_fn) = options.math_fn {
                    math_fn(ui, &tex, false);
                }
            }
        }
    }

    fn event_text(
        &mut self,
        text: CowStr,
        src_span: Range<usize>,
        ui: &mut Ui,
        cache: &mut CommonMarkCache,
    ) {
        if let Some(image) = &mut self.image {
            image.alt_text.push(self.text_style.to_richtext(ui, &text));
        } else if let Some(block) = &mut self.code_block {
            block.content.push_str(&text);
        } else if let Some(link) = &mut self.link {
            link.text.push(self.text_style.to_richtext(ui, &text));
        } else {
            self.render_body_text(ui, cache, &text, src_span);
        }
    }

    /// Render a body-text segment, splitting it at search-match boundaries and
    /// painting teal (match) or violet (active match) backgrounds on the hits.
    /// Falls back to a plain label when there are no overlapping search ranges.
    fn render_body_text(
        &self,
        ui: &mut Ui,
        cache: &CommonMarkCache,
        text: &str,
        src_span: Range<usize>,
    ) {
        // Highlight colours — theme-adaptive teal/lilac palette.
        // Different hues for the active match (lilac/violet) vs other matches (teal)
        // to make the focused hit immediately obvious.
        let (match_bg, active_bg) = if ui.visuals().dark_mode {
            (
                egui::Color32::from_rgb(30, 115, 105), // deep teal   – readable with light text
                egui::Color32::from_rgb(95, 75, 165),  // deep violet – readable with light text
            )
        } else {
            (
                egui::Color32::from_rgb(140, 220, 210), // soft mint-teal   – readable with dark text
                egui::Color32::from_rgb(185, 165, 240), // soft periwinkle  – readable with dark text
            )
        };

        // Collect intervals that overlap with this text span, converted to
        // byte offsets local to `text` (0-based from text start).
        let intervals: Vec<(usize, usize, bool)> = {
            let active = cache.active_search_range();
            cache
                .search_ranges()
                .iter()
                .filter(|r| r.start < src_span.end && r.end > src_span.start)
                .map(|r| {
                    let local_start = r.start.saturating_sub(src_span.start).min(text.len());
                    let local_end = r.end.saturating_sub(src_span.start).min(text.len());
                    let is_active =
                        active.map_or(false, |ar| ar.start == r.start && ar.end == r.end);
                    (local_start, local_end, is_active)
                })
                .filter(|(s, e, _)| s < e)
                .collect()
        };

        if intervals.is_empty() {
            ui.label(self.text_style.to_richtext(ui, text));
            return;
        }

        // Render prefix / highlight / suffix segments in sequence.
        // `item_spacing.x` is already 0 in the outer horizontal-wrap layout, so
        // consecutive labels flow without gaps between them.
        let mut pos = 0usize;
        for (start, end, is_active) in &intervals {
            if pos < *start {
                if let Some(slice) = text.get(pos..*start) {
                    ui.label(self.text_style.to_richtext(ui, slice));
                }
            }
            if let Some(slice) = text.get(*start..*end) {
                let bg = if *is_active { active_bg } else { match_bg };
                ui.label(self.text_style.to_richtext(ui, slice).background_color(bg));
            }
            pos = *end;
        }
        if pos < text.len() {
            if let Some(slice) = text.get(pos..) {
                ui.label(self.text_style.to_richtext(ui, slice));
            }
        }
    }

    fn start_tag(
        &mut self,
        ui: &mut Ui,
        tag: pulldown_cmark::Tag,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
    ) {
        match tag {
            pulldown_cmark::Tag::Paragraph => {
                self.line.try_insert_start(ui);
            }
            pulldown_cmark::Tag::Heading { level, id, .. } => {
                if let Some(scroll_target) = cache.scroll_to_id_target()
                    && let Some(id) = id
                    && id.into_string() == scroll_target
                {
                    ui.scroll_to_cursor(Some(egui::Align::TOP));
                    cache.scroll_to_id_target_mut().take();
                }

                // Headings should always insert a newline even if it is at the start.
                // Whether this is okay in all scenarios is a different question.
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

            // deliberately not using the built in alerts from pulldown-cmark as
            // the markdown itself cannot be localized :( e.g: [!TIP]
            pulldown_cmark::Tag::BlockQuote(_) => {
                self.is_blockquote = true;
            }
            pulldown_cmark::Tag::CodeBlock(c) => {
                match c {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                        self.code_block = Some(crate::CodeBlock {
                            lang: Some(lang.to_string()),
                            content: "".to_string(),
                        });
                    }
                    pulldown_cmark::CodeBlockKind::Indented => {
                        self.code_block = Some(crate::CodeBlock {
                            lang: None,
                            content: "".to_string(),
                        });
                    }
                }
                self.line.try_insert_start(ui);
            }

            pulldown_cmark::Tag::List(point) => {
                if !self.list.is_inside_a_list() && self.line.can_insert_start() {
                    newline(ui);
                }

                if let Some(number) = point {
                    self.list.start_level_with_number(number);
                } else {
                    self.list.start_level_without_number();
                }
                self.line.should_start_newline = false;
                self.line.should_end_newline = false;
            }

            pulldown_cmark::Tag::Item => {
                self.is_list_item = true;
                self.list.start_item(ui, options);
            }

            pulldown_cmark::Tag::FootnoteDefinition(note) => {
                self.line.try_insert_start(ui);

                self.line.should_start_newline = false;
                self.line.should_end_newline = false;
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
            pulldown_cmark::Tag::HtmlBlock => {
                self.line.try_insert_start(ui);
            }
            pulldown_cmark::Tag::MetadataBlock(_) => {}

            pulldown_cmark::Tag::DefinitionList => {
                self.line.try_insert_start(ui);
                self.def_list.is_first_item = true;
            }
            pulldown_cmark::Tag::DefinitionListTitle => {
                // we disable newline as the first title should not insert a newline
                // as we have already done that upon the DefinitionList Tag
                if !self.def_list.is_first_item {
                    self.line.try_insert_start(ui)
                } else {
                    self.def_list.is_first_item = false;
                }
            }
            pulldown_cmark::Tag::DefinitionListDefinition => {
                self.def_list.is_def_list_def = true;
            }
            // Not yet supported
            pulldown_cmark::Tag::Superscript | pulldown_cmark::Tag::Subscript => {}
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
                self.line.try_insert_end(ui);
            }
            pulldown_cmark::TagEnd::Heading { .. } => {
                self.line.try_insert_end(ui);
                self.text_style.heading = None;
            }
            pulldown_cmark::TagEnd::BlockQuote(_) => {}
            pulldown_cmark::TagEnd::CodeBlock => {
                self.end_code_block(ui, cache, options, max_width);
            }

            pulldown_cmark::TagEnd::List(_) => {
                if self.list.is_last_level() {
                    self.line.should_start_newline = true;
                    self.line.should_end_newline = true;
                }

                self.list.end_level(ui, self.line.can_insert_end());

                if !self.list.is_inside_a_list() {
                    // Reset all the state and make it ready for the next list that occurs
                    self.list = List::default();
                }
            }
            pulldown_cmark::TagEnd::Item => {}
            pulldown_cmark::TagEnd::FootnoteDefinition => {
                self.line.should_start_newline = true;
                self.line.should_end_newline = true;
                self.line.try_insert_end(ui);
            }
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
                    link.end(ui, cache, options, &mut self.deferred_scroll_to_heading);
                }
            }
            pulldown_cmark::TagEnd::Image => {
                if let Some(image) = self.image.take() {
                    image.end(ui, options);
                }
            }
            pulldown_cmark::TagEnd::HtmlBlock => {
                if let Some(html_fn) = options.html_fn {
                    html_fn(ui, &self.html_block);
                    self.html_block.clear();
                }
            }

            pulldown_cmark::TagEnd::MetadataBlock(_) => {}

            pulldown_cmark::TagEnd::DefinitionList => self.line.try_insert_end(ui),
            pulldown_cmark::TagEnd::DefinitionListTitle
            | pulldown_cmark::TagEnd::DefinitionListDefinition => {}
            pulldown_cmark::TagEnd::Superscript | pulldown_cmark::TagEnd::Subscript => {}
        }
    }

    fn end_code_block(
        &mut self,
        ui: &mut Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
    ) {
        if let Some(block) = self.code_block.take() {
            block.end(ui, cache, options, max_width);
            self.line.try_insert_end(ui);
        }
    }
}
