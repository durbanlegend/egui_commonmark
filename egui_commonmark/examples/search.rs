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
}

impl App {}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.set_min_height(512.0);

        egui::Panel::top("search_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Search:");
                let response = ui.text_edit_singleline(&mut self.cache.search_query);
                if response.changed() {
                    self.cache.update_search_matches("search_example", MARKDOWN);
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

                let match_count = self.cache.search_ranges().len();
                ui.label(match self.cache.active_match() {
                    Some(i) if match_count > 0 => format!("{}/{match_count}", i + 1),
                    _ => format!("0/{match_count}"),
                });

                if ui.button("Previous").clicked()
                    || (enter_pressed && ui.input(|i| i.modifiers.shift))
                {
                    self.cache.go_to_match(-1);
                }
                if ui.button("Next").clicked()
                    || (enter_pressed && !ui.input(|i| i.modifiers.shift))
                {
                    self.cache.go_to_match(1);
                }
            });
        });

        egui::CentralPanel::default().show(ui, |ui| {
            ui.style_mut().spacing.scroll = egui::style::ScrollStyle::thin();

            let user_scrolled = self.cache.handle_keyboard_scrolling(ui);

            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                self.cache.apply_pending_scroll_delta(ui);
                CommonMarkViewer::new().show(ui, &mut self.cache, MARKDOWN);
            });

            // After show() the cache holds fresh per-match virtual-y positions
            // and the current viewport top. Sync the active match so that
            // Next/Previous advance from wherever the user has scrolled to.
            self.cache.sync_active_match_to_viewport(user_scrolled);
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
            }))
        }),
    )
}
