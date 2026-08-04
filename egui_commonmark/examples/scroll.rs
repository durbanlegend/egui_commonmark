//! Make sure to run this example from the repo directory and not the example
//! directory. Run with:
//! `cargo r --example scroll --features better_syntax_highlighting,svg [light|dark]`
//! Run with CACHE=false to disable viewport caching for comparison.

use std::env;

use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

struct App {
    cache: CommonMarkCache,
    content: String,
    viewport_cache: bool,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.set_min_height(512.0);

        egui::CentralPanel::default().show(ui, |ui| {
            // Style the scroll bar to make the thumb position clearly visible.
            {
                let scroll = &mut ui.style_mut().spacing.scroll;
                scroll.floating = true;
                scroll.floating_width = 7.0;
                scroll.content_margin = egui::Margin::same(10);
                scroll.bar_width = 10.0;
                scroll.dormant_handle_opacity = 0.40;
                scroll.interact_handle_opacity = 0.55;
                scroll.active_handle_opacity = 0.80;
            }

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
         This document demonstrates `show_scrollable` with `viewport_cache(true)`: \
         only the visible slice is rendered each frame, so scrolling stays fast \
         regardless of document length. Run with `CACHE=false` to compare against \
         the full-document render path.\n\
         \n\
         Keyboard shortcuts: Up/Down arrows scroll one line; Page Up/Down scroll \
         one page; Home/End (Fn+Left/Right or Cmd+Up/Down on macOS) jump to \
         document top or bottom.\n\
         \n",
    );

    let repeating = "\n\
        This section will be repeated.\n\
        \n\
        ```rs\n\
        let mut vec = Vec::new();\n\
        vec.push(5);\n\
        ```\n\
        \n\
        # Plans\n\
        * Make a sandwich\n\
        * Bake a cake\n\
        * Conquer the world\n\
        \n\
        ![Ferris the Rust mascot](egui_commonmark/examples/cuddlyferris.png)\n\
        \n";

    // Insert a named anchor at a predictable location for the TOC link demo.
    let anchor_section = "\n\
        This section will be repeated.\n\
        \n\
        ```rs\n\
        let mut vec = Vec::new();\n\
        vec.push(5);\n\
        ```\n\
        \n\
        # Plans {#section-500}\n\
        * Make a sandwich\n\
        * Bake a cake\n\
        * Conquer the world\n\
        \n\
        ![Ferris the Rust mascot](egui_commonmark/examples/cuddlyferris.png)\n\
        \n";

    for i in 1..=1024_usize {
        if i == 500 {
            text += anchor_section;
        } else {
            text += repeating;
        }
    }
    text
}
