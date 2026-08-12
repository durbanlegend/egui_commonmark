//! Demonstrates search-match highlighting with default colors.
//!
//! Typing in the search bar highlights every match. Prev/Next step through
//! matches and scroll the document to centre each one in the viewport.
//!
//! Run with:
//! `cargo r --example search [light|dark]`

use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

const MARKDOWN: &str = r#"# Search Highlighting

Type text in the search bar above to highlight every occurrence in this document.
Use **Prev** and **Next** to step through matches.

Suggestion: try searching for "MIT" to see scrolling to a match near the bottom.

---

# A commonmark viewer for [egui](https://github.com/emilk/egui)

While this crate's main focus is commonmark, it also supports a subset of
Github's markdown syntax: tables, strikethrough, tasklists and footnotes.

## Features

* `macros`: macros for compile time parsing of markdown
* `better_syntax_highlighting`: Syntax highlighting inside code blocks with
  [`syntect`](https://crates.io/crates/syntect)
* `svg`: Support for viewing svg images
* `fetch`: Images with urls will be downloaded and displayed
* `embedded_image`: Load base64 image data urls from within markdown files


## Examples

For an easy intro check out the `hello_world` example. To see all the different
features egui_commonmark has to offer check out the `book` example.

## FAQ

### URL is not displayed when hovering over a link

By default egui does not show urls when you hover hyperlinks. To enable it,
you can do the following before calling any ui related functions:

```rust
ui.style_mut().url_in_tooltip = true;
```

## MSRV Policy

This crate uses the same MSRV as the latest released egui version.

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
"#;

struct App {
    cache: CommonMarkCache,
    search_query: String,
    search_matches: Vec<std::ops::Range<usize>>,
    active_match: Option<usize>,
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
    /// forces a full document re-render), so this shoud stay responsive.
    fn update_search_matches(&mut self) {
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
            let parser = pulldown_cmark::Parser::new_ext(MARKDOWN, options).into_offset_iter();
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
                // dbg!(&self.search_matches);
            }
        }
        self.cache.set_search_ranges(self.search_matches.clone());

        if self.search_matches.is_empty() {
            self.active_match = None;
            self.sync_active_match();
            return;
        }

        let cursor = self
            .cache
            .viewport_start_byte_offset("search_example")
            .unwrap_or(0);
        dbg!(&cursor);
        let nearest = self
            .search_matches
            .iter()
            .position(|r| r.start >= cursor)
            .unwrap_or(0);
        dbg!(&nearest);
        self.active_match = Some(nearest);
        self.sync_active_match();
        self.cache.scroll_to_active_search_match();
    }

    fn sync_active_match(&mut self) {
        self.cache.set_active_search_range(
            self.active_match
                .and_then(|i| {
                    dbg!(i);
                    self.search_matches.get(i)
                })
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

            let (
                scroll_line_up,
                scroll_line_down,
                scroll_page_up,
                scroll_page_down,
                scroll_doc_top,
                scroll_doc_bottom,
            ) = ui.ctx().input(|i| {
                use egui::Key;
                (
                    !i.modifiers.command && i.key_pressed(Key::ArrowUp),
                    !i.modifiers.command && i.key_pressed(Key::ArrowDown),
                    i.key_pressed(Key::PageUp),
                    i.key_pressed(Key::PageDown),
                    i.key_pressed(Key::Home)
                        || (i.modifiers.command && i.key_pressed(Key::ArrowUp)),
                    i.key_pressed(Key::End)
                        || (i.modifiers.command && i.key_pressed(Key::ArrowDown)),
                )
            });

            // Only act on scroll keys when no text field has focus.
            if !ui.ctx().egui_wants_keyboard_input() {
                let line_h = ui.text_style_height(&egui::TextStyle::Body);
                let page_h = ui.available_height();
                if scroll_line_up {
                    self.cache.set_scroll_delta(egui::vec2(0.0, line_h));
                } else if scroll_line_down {
                    self.cache.set_scroll_delta(egui::vec2(0.0, -line_h));
                } else if scroll_page_up {
                    self.cache.set_scroll_delta(egui::vec2(0.0, page_h));
                } else if scroll_page_down {
                    self.cache.set_scroll_delta(egui::vec2(0.0, -page_h));
                } else if scroll_doc_top {
                    self.cache.set_scroll_delta(egui::vec2(0.0, f32::MAX / 2.0));
                } else if scroll_doc_bottom {
                    self.cache
                        .set_scroll_delta(egui::vec2(0.0, -f32::MAX / 2.0));
                }
            }

            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                CommonMarkViewer::new().show(ui, &mut self.cache, MARKDOWN);
            });
        });
    }
}

fn main() -> eframe::Result {
    let mut args = std::env::args();
    args.next();

    eframe::run_native(
        "Markdown search example",
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
                search_query: String::new(),
                search_matches: Vec::new(),
                active_match: None,
            }))
        }),
    )
}
