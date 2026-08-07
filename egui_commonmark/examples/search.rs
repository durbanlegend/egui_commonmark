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
    query: String,
    /// Byte-range matches into MARKDOWN, in document order.
    matches: Vec<std::ops::Range<usize>>,
    /// Index of the active (focused) match.
    active: usize,
}

impl App {
    fn new() -> Self {
        Self {
            cache: CommonMarkCache::default(),
            query: String::new(),
            matches: Vec::new(),
            active: 0,
        }
    }

    /// Rebuild match ranges from a case-insensitive search over the raw source.
    fn update_matches(&mut self) {
        self.matches.clear();
        self.active = 0;
        if self.query.is_empty() {
            return;
        }
        let haystack = MARKDOWN.to_lowercase();
        let needle = self.query.to_lowercase();
        let mut start = 0;
        while let Some(pos) = haystack[start..].find(&needle) {
            let lo = start + pos;
            self.matches.push(lo..lo + self.query.len());
            start = lo + 1;
        }
    }

    /// Push the current match ranges and active range into the cache.
    fn apply_to_cache(&mut self) {
        self.cache.set_search_ranges(self.matches.clone());
        self.cache
            .set_active_search_range(self.matches.get(self.active).cloned());
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Search:");
                let response = ui.text_edit_singleline(&mut self.query);
                if response.changed() {
                    self.update_matches();
                    self.cache.set_scroll_to_active_match(true);
                }

                let count = self.matches.len();
                if count == 0 {
                    if !self.query.is_empty() {
                        ui.label("No matches");
                    }
                } else {
                    ui.label(format!("{} / {}", self.active + 1, count));
                    if ui.button("Prev").clicked() {
                        self.active = (self.active + count - 1) % count;
                        self.cache.set_scroll_to_active_match(true);
                    }
                    if ui.button("Next").clicked() {
                        self.active = (self.active + 1) % count;
                        self.cache.set_scroll_to_active_match(true);
                    }
                }

                if !self.query.is_empty() && ui.button("×").clicked() {
                    self.query.clear();
                    self.update_matches();
                }
            });

            ui.separator();

            self.apply_to_cache();

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
            Ok(Box::new(App::new()))
        }),
    )
}
