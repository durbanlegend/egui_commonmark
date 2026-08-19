//! Make sure to run this example from the repo directory and not the example
//! directory. Run with:
//! `cargo r --example scroll --features better_syntax_highlighting,svg [light|dark]`
//! Run with `CACHE=false` to disable viewport caching for comparison.
//!
//! Keyboard shortcuts (when no text field has focus):
//! Up/Down arrows scroll one line; Page Up/Down scroll one page;
//! Home/End (or Cmd+Up/Down on macOS) jump to the top or bottom.
use std::env;

use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

const EGUI_SOURCE_ID: &'static str = "scroll_example";

struct App {
    cache: CommonMarkCache,
    content: String,
    viewport_cache: bool,
    search_query: String,
    search_matches: Vec<std::ops::Range<usize>>,
    active_match: Option<usize>,
    /// Counts down after search-initiated scrolls (Next / Previous / query
    /// change) to block viewport-driven active_match updates while the
    /// scroll-to-match animation is still in progress.  User scroll input
    /// immediately clears this to zero so the user always wins.
    search_scroll_protection: u32,
    /// Byte offset reported by the viewer at the end of the last frame;
    /// used to detect viewport movement without relying on input events.
    last_viewport_offset: usize,
}

impl App {
    /// Recomputes `search_matches` from the *rendered* text only (via
    /// pulldown-cmark's `Text`/`Code` events), so link destinations,
    /// heading `{#id}` attribute syntax, and other non-visible markdown
    /// syntax are never matched (a naive substring search over the raw
    /// source would, for example, double-count "500" in
    /// `[Section 500](#section-500)`: once in the visible text, once in the
    /// URL).
    ///
    /// Recomputes `search_matches` on every keystroke and immediately
    /// advances to the nearest match at or after wherever the user is
    /// currently scrolled to (wrapping to the first match if there is none
    /// after that point), mirroring how a normal "find in page" behaves.
    /// Recomputation and the resulting scroll are both cheap (see
    /// `CommonMarkCache::scroll_to_active_search_match`'s docs: this never
    /// forces a full document re-render), so this stays responsive even for
    /// this fairly large (~275 KB) document.
    fn update_search_matches(&mut self) {
        // Anchor to the byte position of the currently active match so that
        // adding/removing characters from the query stays on the same spot.
        // Fall back to the viewport position for a fresh (no active match)
        // search.  Using viewport_start here on a query change would jump
        // backwards whenever the viewport centre is a couple of sections
        // before the active match (i.e. the match is centred on screen).
        let anchor = self
            .active_match
            .and_then(|i| self.search_matches.get(i))
            .map(|r| r.start)
            .or_else(|| self.cache.viewport_start_byte_offset(EGUI_SOURCE_ID))
            .unwrap_or(0);

        self.search_matches.clear();
        if !self.search_query.is_empty() {
            let query = self.search_query.to_lowercase();
            // Mirror the options CommonMarkViewer itself parses with,
            // including heading attributes since `enable_scroll_to_heading`
            // is set below (otherwise `{#section-500}` would remain in the
            // heading's Text event and get matched too).
            let options = pulldown_cmark::Options::ENABLE_STRIKETHROUGH
                | pulldown_cmark::Options::ENABLE_TASKLISTS
                | pulldown_cmark::Options::ENABLE_TABLES
                | pulldown_cmark::Options::ENABLE_FOOTNOTES
                | pulldown_cmark::Options::ENABLE_HEADING_ATTRIBUTES;
            let parser = pulldown_cmark::Parser::new_ext(&self.content, options).into_offset_iter();
            for (event, range) in parser {
                let text = match event {
                    pulldown_cmark::Event::Text(text) | pulldown_cmark::Event::Code(text) => text,
                    _ => continue,
                };
                let haystack = text.to_lowercase();
                let mut start = 0;
                while let Some(pos) = haystack[start..].find(&query) {
                    let match_start = range.start + start + pos;
                    let match_end = match_start + query.len();
                    self.search_matches.push(match_start..match_end);
                    start += pos + query.len();
                }
            }
        }
        self.cache.set_search_ranges(self.search_matches.clone());

        if self.search_matches.is_empty() {
            self.active_match = None;
            self.sync_active_match();
            return;
        }

        let nearest = self
            .search_matches
            .iter()
            .position(|r| r.start >= anchor)
            .unwrap_or(0);
        self.active_match = Some(nearest);
        self.sync_active_match();
        self.cache.scroll_to_active_search_match();
        // Suppress viewport-sync for ~30 frames so the animation toward the
        // new match is not immediately overridden by the centering drift
        // (viewport start lands before the active match when centred on screen).
        self.search_scroll_protection = 30;
    }

    fn sync_active_match(&mut self) {
        self.cache.set_active_search_range(
            self.active_match
                .and_then(|i| self.search_matches.get(i))
                .cloned(),
        );
    }

    fn go_to_match(&mut self, delta: isize) {
        if self.search_matches.is_empty() {
            return;
        }
        let len = self.search_matches.len() as isize;
        let next = match self.active_match {
            Some(i) => (i as isize + delta).rem_euclid(len),
            // First navigation after a fresh search: start at the first
            // match for Next, the last one for Previous.
            None if delta >= 0 => 0,
            None => len - 1,
        };
        self.active_match = Some(next as usize);
        self.sync_active_match();
        self.cache.scroll_to_active_search_match();
        self.search_scroll_protection = 30;
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.set_min_height(512.0);

        egui::Panel::top("search_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Search:");
                let response = ui.text_edit_singleline(&mut self.search_query);
                if response.changed() {
                    self.update_search_matches();
                }
                // Checked unconditionally (not gated on the text edit still
                // having focus): a single-line TextEdit surrenders focus the
                // moment Enter is pressed, so `response.has_focus()` would
                // already be false here. We re-request focus below so that
                // repeated Enter presses keep working without having to
                // click back into the box each time.
                let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                if enter_pressed {
                    response.request_focus();
                }

                let match_count = self.search_matches.len();
                ui.label(match self.active_match {
                    Some(i) if match_count > 0 => format!("{}/{match_count}", i + 1),
                    _ => format!("0/{match_count}"),
                });

                if ui.button("Previous").clicked()
                    || (enter_pressed && ui.input(|i| i.modifiers.shift))
                {
                    self.go_to_match(-1);
                }
                if ui.button("Next").clicked()
                    || (enter_pressed && !ui.input(|i| i.modifiers.shift))
                {
                    self.go_to_match(1);
                }
            });
        });

        egui::CentralPanel::default().show(ui, |ui| {
            ui.style_mut().spacing.scroll = egui::style::ScrollStyle::thin();

            let _user_scrolled = self.cache.handle_keyboard_scrolling(ui);

            CommonMarkViewer::new()
                .max_image_width(Some(512))
                .enable_scroll_to_heading(true)
                .viewport_cache(self.viewport_cache)
                .show_scrollable(EGUI_SOURCE_ID, ui, &mut self.cache, &self.content);

            // After show_scrollable the viewer has applied any pending scroll
            // delta and updated the byte-offset tracker.  Sync active_match
            // whenever the viewport byte offset actually changed AND no search
            // scroll is still animating.  This fires on every animation frame
            // (not just the key-press frame), so even large PageDown jumps
            // settle to the correct match once the animation completes.
            let current_offset = self
                .cache
                .viewport_start_byte_offset(EGUI_SOURCE_ID)
                .unwrap_or(0);

            if !self.search_matches.is_empty()
                && self.search_scroll_protection == 0
                && current_offset != self.last_viewport_offset
            {
                let len = self.search_matches.len();
                let idx = self
                    .search_matches
                    .partition_point(|r| r.start < current_offset);
                let nearest = if idx > 0 {
                    idx - 1
                } else {
                    len.saturating_sub(1)
                };
                if self.active_match != Some(nearest) {
                    self.active_match = Some(nearest);
                    self.sync_active_match();
                    // Do NOT call scroll_to_active_search_match here: the
                    // viewport is already where the user put it.
                }
            }

            self.last_viewport_offset = current_offset;
            if self.search_scroll_protection > 0 {
                self.search_scroll_protection -= 1;
            }
        });
    }
}

fn main() -> eframe::Result {
    let mut args = env::args();
    args.next();

    let viewport_cache = env::var("CACHE")
        .map(|v| v.to_lowercase() != "false" && v != "0")
        .unwrap_or(true);

    let content = build_document();
    eprintln!("Document size: {} bytes", content.len());

    eframe::run_native(
        "Markdown scroll example",
        eframe::NativeOptions::default(),
        Box::new(move |cc| {
            if let Some(theme) = args.next() {
                if theme == "light" {
                    cc.egui_ctx.set_theme(egui::Theme::Light);
                } else if theme == "dark" {
                    cc.egui_ctx.set_theme(egui::Theme::Dark);
                }
            }
            Ok(Box::new(App {
                cache: CommonMarkCache::default(),
                content,
                viewport_cache,
                search_query: String::new(),
                search_matches: Vec::new(),
                active_match: None,
                search_scroll_protection: 0,
                last_viewport_offset: 0,
            }))
        }),
    )
}

fn build_document() -> String {
    let mut text = String::from(
        "# Markdown Scroll Example\n\
         \n\
         Jump to: [Section 500](#section-500)\n\
         \n\
         This document demonstrates `show_scrollable`: only the visible slice is \
         rendered each frame, so scrolling stays fast regardless of document length. \
         The TOC link above jumps to section 500, far outside the initial viewport, \
         to demonstrate that heading navigation works across the full document.\
         \n\
         Keyboard shortcuts: Up/Down arrows scroll one line; Page Up/Down scroll \
         one page; Home/End (or Cmd+Up/Down on macOS) jump to the top or bottom.\
         \n",
    );

    for i in 1..=1024_usize {
        let id = if i == 500 { " {#section-500}" } else { "" };
        text += &format!(
            r#"
## Section {i}{id}

This is section {i}. Each section contains a short code block and an image.

```rs
let mut vec = Vec::new();
vec.push({i});
```

* Make a sandwich
* Bake a cake
* Conquer the world

![Ferris the Rust mascot](egui_commonmark/examples/cuddlyferris.png)

"#
        );
    }
    text
}
