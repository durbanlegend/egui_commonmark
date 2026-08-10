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

struct App {
    cache: CommonMarkCache,
    content: String,
    viewport_cache: bool,
    search_query: String,
    search_matches: Vec<std::ops::Range<usize>>,
    active_match: Option<usize>,
}

impl App {
    fn update_search_matches(&mut self) {
        self.search_matches.clear();
        if !self.search_query.is_empty() {
            let query = self.search_query.to_lowercase();
            let haystack = self.content.to_lowercase();
            let mut start = 0;
            while let Some(pos) = haystack[start..].find(&query) {
                let match_start = start + pos;
                let match_end = match_start + query.len();
                self.search_matches.push(match_start..match_end);
                start = match_end;
            }
        }
        self.active_match = if self.search_matches.is_empty() {
            None
        } else {
            Some(0)
        };
        self.cache.set_search_ranges(self.search_matches.clone());
        self.sync_active_match();
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
        let current = self.active_match.map(|i| i as isize).unwrap_or(0);
        let next = (current + delta).rem_euclid(len);
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
                let enter_pressed =
                    response.lost_focus() && ui.ctx().input(|i| i.key_pressed(egui::Key::Enter));

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

            CommonMarkViewer::new()
                .max_image_width(Some(512))
                .enable_scroll_to_heading(true)
                .viewport_cache(self.viewport_cache)
                .show_scrollable("scroll_example", ui, &mut self.cache, &self.content);
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
