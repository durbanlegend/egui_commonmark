//! Make sure to run this example from the repo directory and not the example
//! directory. To see all the features in full effect, run this example with
//! `cargo r --example scroll --features better_syntax_highlighting,svg,fetch
//! Add `light` or `dark` to the end of the command to specify theme. Default
//! is light. `cargo r --example scroll --all-features dark`

use eframe::egui;
use egui::Ui;
use egui_commonmark::*;

struct App {
    cache: CommonMarkCache,
    content: String,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        ui.set_min_height(512.0);

        egui::CentralPanel::default().show(ui, |ui| {
            // ── Style the scroll bar that show_scrollable will create internally ─
            // These settings propagate into the inner ScrollArea because
            // show_scrollable inherits ui.style() from this outer ui.
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
                    // Scroll keys — only plain (non-Cmd) arrow keys for line scroll.
                    !i.modifiers.command && i.key_pressed(Key::ArrowUp),
                    !i.modifiers.command && i.key_pressed(Key::ArrowDown),
                    i.key_pressed(Key::PageUp),
                    i.key_pressed(Key::PageDown),
                    // Home / End: physical key OR Cmd+Arrow (standard macOS navigation).
                    i.key_pressed(Key::Home)
                        || (i.modifiers.command && i.key_pressed(Key::ArrowUp)),
                    i.key_pressed(Key::End)
                        || (i.modifiers.command && i.key_pressed(Key::ArrowDown)),
                )
            });

            // Act on shortcuts only when text field does not have focus.
            let wants_text = ui.ctx().egui_wants_keyboard_input();

            // ── Keyboard scrolling ───────────────────────────────────────────
            // Deltas are threaded through CommonMarkCache so show_scrollable
            // can apply them inside its own internal ScrollArea.
            if !wants_text {
                let line_h = ui.text_style_height(&egui::TextStyle::Body);
                let page_h = ui.available_height();
                // eprintln!("line_h={line_h}, page_h={page_h}");
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
                .viewport_cache(true)
                .show_scrollable("Generated content", ui, &mut self.cache, &self.content);
        });
    }
}

fn main() {
    let mut args = std::env::args();
    args.next();
    let use_dark_theme = if let Some(theme) = args.next() {
        if theme == "light" {
            false
        } else {
            theme == "dark"
        }
    } else {
        false
    };

    let text = build_document();

    eprintln!("Document size is {} bytes", text.len());

    eframe::run_native(
        "Markdown viewer",
        eframe::NativeOptions::default(),
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(if use_dark_theme {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });

            Ok(Box::new(App {
                cache: CommonMarkCache::default(),
                content: text,
            }))
        }),
    )
    .unwrap();
}

fn build_document() -> String {
    let mut text = r#"# Commonmark Viewer Example
    This is a fairly large markdown file showcasing scroll.

    After the first rendering pass it should be responsive.
    But it will need to re-render each time the app is resized
    or if the content gets modified for any reason.

    To experience uncached performance for comparison, set
    `.viewport_cache(false)` on the `CommonmarkViewer`.

    The scrollbar has deliberately been made conspicuous
    for the demonstration.

    Try using the scrolling shortcuts:
        Home:           Fn-left arrow
        End:            Fn-right arrow
        Up   1 line:    Up-arrow
        Down 1 line:    Down-arrow
        Up   1 page:    Fn-up arrow
        Down 1 page:    Fn-up arrow
                "#
    .to_string();

    let repeating = r#"
This section will be repeated

```rs
let mut vec = Vec::new();
vec.push(5);
```

# Plans
* Make a sandwich
* Bake a cake
* Conquer the world
* Take a picture

[![Take a picture](https://picsum.photos/300/200/?random)](https://picsum.photos/300/200/?random)
    "#;
    text += &repeating.repeat(1024);
    text
}
