//! Demonstrates search-match highlighting with default colors.
//!
//! Run with:
//! `cargo r --example search [light|dark]`

use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

const MARKDOWN: &str = "\
# Search Highlighting

Type text in the search bar above to highlight every occurrence in this document.
Use **Prev** and **Next** to step through matches.

## Rust

Rust is a systems programming language focused on safety, speed, and concurrency.
Rust achieves memory safety without a garbage collector.

## Why Rust?

Many developers choose Rust for its performance characteristics. The Rust compiler
catches entire classes of bugs at compile time. Rust's ownership model ensures that
data races are impossible.

## Getting Started with Rust

Install Rust via `rustup`. The Rust toolchain includes `cargo`, the build tool and
package manager. Most Rust projects start with `cargo new`.
";

struct App {
    cache: CommonMarkCache,
    query: String,
    /// Byte-range matches into MARKDOWN, in order.
    matches: Vec<std::ops::Range<usize>>,
    /// Index into `matches` for the active (focused) hit.
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

    /// Recompute all match ranges for the current query.
    fn update_matches(&mut self) {
        self.matches.clear();
        if self.query.is_empty() {
            return;
        }
        // Case-insensitive search: scan the lowercased source.
        let haystack = MARKDOWN.to_lowercase();
        let needle = self.query.to_lowercase();
        let mut start = 0;
        while let Some(pos) = haystack[start..].find(&needle) {
            let lo = start + pos;
            let hi = lo + self.query.len();
            self.matches.push(lo..hi);
            start = lo + 1;
        }
        // Clamp active index in case the new query has fewer results.
        if !self.matches.is_empty() {
            self.active = self.active.min(self.matches.len() - 1);
        } else {
            self.active = 0;
        }
    }

    /// Push the current matches into the cache so the viewer picks them up.
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
                    }
                    if ui.button("Next").clicked() {
                        self.active = (self.active + 1) % count;
                    }
                }

                if !self.query.is_empty() && ui.button("✕").clicked() {
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
