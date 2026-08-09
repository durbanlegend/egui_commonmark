//! Make sure to run this example from the repo directory and not the example
//! directory. Run with:
//! `cargo r --example scroll --features better_syntax_highlighting,svg`
//! Add `light` or `dark` to set the theme.

use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

struct App {
    cache: CommonMarkCache,
    content: String,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            CommonMarkViewer::new()
                .max_image_width(Some(512))
                .enable_scroll_to_heading(true)
                .show_scrollable("scroll_example", ui, &mut self.cache, &self.content);
        });
    }
}

fn main() -> eframe::Result {
    let mut args = std::env::args();
    args.next();

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
         to demonstrate that heading navigation works across the full document.\n\
         \n",
    );

    for i in 1..=1024_usize {
        let id = if i == 500 { " {#section-500}" } else { "" };
        text += &format!(
            "\n## Section {i}{id}\n\
             \n\
             This is section {i}. Each section contains a short code block and an image.\n\
             \n\
             ```rs\n\
             let mut vec = Vec::new();\n\
             vec.push({i});\n\
             ```\n\
             \n\
             * Make a sandwich\n\
             * Bake a cake\n\
             * Conquer the world\n\
             \n\
             ![Ferris the Rust mascot](egui_commonmark/examples/cuddlyferris.png)\n\
             \n"
        );
    }
    text
}
