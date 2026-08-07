//! Demonstrates search-match highlighting with default colors.
//!
//! Typing in the search bar highlights every match. Prev/Next step through
//! matches and scroll the document to the heading section containing each one.
//!
//! Run with:
//! `cargo r --example search [light|dark]`

use std::collections::HashMap;

use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use pulldown_cmark::{Event, Options, Parser, Tag};

const MARKDOWN: &str = r#"# Search Highlighting

Type text in the search bar above to highlight every occurrence in this document.
Use **Prev** and **Next** to step through matches.

Suggestion: try a search string like "MIT" to demonstrate scrolling to the matches.

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

fn pc_opts() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_HEADING_ATTRIBUTES
}

/// Converts heading text to a URL-safe slug: lowercased, non-alphanumeric runs
/// replaced by `-`.
fn slugify(text: &str) -> String {
    let mut slug = String::with_capacity(text.len());
    let mut prev_sep = true;
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            prev_sep = false;
        } else if !prev_sep {
            slug.push('-');
            prev_sep = true;
        }
    }
    if slug.ends_with('-') {
        slug.pop();
    }
    slug
}

/// Parses an ATX heading line and returns `(level, plain_text)`, stripping any
/// trailing `{…}` attribute block. Returns `None` for non-heading lines.
#[allow(clippy::cast_possible_truncation)]
fn parse_heading_line(line: &str) -> Option<(u8, &str)> {
    let hashes = line.bytes().take_while(|&b| b == b'#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = line[hashes..].strip_prefix(' ')?;
    let text = rest.trim_end();
    if text.is_empty() {
        return None;
    }
    let plain = text.rfind('{').map_or(text, |brace| {
        let attr = text[brace..].trim_end();
        if attr.ends_with('}') {
            text[..brace].trim_end()
        } else {
            text
        }
    });
    Some((hashes as u8, plain))
}

/// Returns the explicit `{#id}` from a heading line, if present.
fn extract_heading_id(line: &str) -> Option<&str> {
    let brace = line.rfind('{')?;
    let attr = line[brace..].trim_end();
    if attr.starts_with("{#") && attr.ends_with('}') {
        Some(&attr[2..attr.len() - 1])
    } else {
        None
    }
}

/// Scans `raw` markdown, injects `{#slug}` attributes into every heading that
/// lacks one, and returns `(processed_content, slugs_in_order)`.
///
/// Uses pulldown-cmark as the heading oracle so that the injected IDs are
/// consistent with what the renderer sees.
fn inject_heading_ids(raw: &str) -> (String, Vec<String>) {
    // Pass 1 — let pulldown-cmark locate the real heading starts.
    let heading_starts: Vec<usize> = Parser::new_ext(raw, pc_opts())
        .into_offset_iter()
        .filter_map(|(event, span)| {
            matches!(event, Event::Start(Tag::Heading { .. })).then_some(span.start)
        })
        .collect();

    // Pass 2 — rebuild content, injecting `{#slug}` where needed.
    let mut out = String::with_capacity(raw.len() + heading_starts.len() * 24);
    let mut slugs: Vec<String> = Vec::new();
    let mut slug_counts: HashMap<String, usize> = HashMap::new();
    let mut pos = 0usize;

    for line_start in heading_starts {
        out.push_str(&raw[pos..line_start]);

        let newline = raw[line_start..]
            .find('\n')
            .map_or(raw.len(), |p| line_start + p);
        let line = &raw[line_start..newline];

        if let Some((_level, plain_text)) = parse_heading_line(line) {
            let (slug, injected) = extract_heading_id(line).map_or_else(
                || {
                    let base = slugify(plain_text);
                    let n = slug_counts.entry(base.clone()).or_insert(0);
                    let slug = if *n == 0 { base } else { format!("{base}-{n}") };
                    *n += 1;
                    let with_id = format!("{} {{#{slug}}}", line.trim_end());
                    (slug, with_id)
                },
                |id| (id.to_string(), line.to_string()),
            );
            slugs.push(slug);
            out.push_str(&injected);
        } else {
            // Setext or unrecognised heading — pass through unchanged.
            out.push_str(line);
        }

        pos = if newline < raw.len() {
            out.push('\n');
            newline + 1
        } else {
            newline
        };
    }

    out.push_str(&raw[pos..]);
    (out, slugs)
}

/// Builds a `(byte_pos, slug)` list from the processed content and its slugs.
/// The byte positions are content-relative (i.e. into the injected string).
fn build_heading_positions(content: &str, slugs: &[String]) -> Vec<(usize, String)> {
    let mut result = Vec::new();
    let mut idx = 0;
    for (event, span) in Parser::new_ext(content, pc_opts()).into_offset_iter() {
        if matches!(event, Event::Start(Tag::Heading { .. })) {
            if let Some(slug) = slugs.get(idx) {
                result.push((span.start, slug.clone()));
            }
            idx += 1;
        }
    }
    result
}

/// Appends the byte-start positions of all case-insensitive occurrences of
/// `query` found within `text` to `out`. `span_start` is the offset of `text`
/// within the full document.
fn collect_matches(text: &str, span_start: usize, query: &str, qlen: usize, out: &mut Vec<usize>) {
    let lower = text.to_lowercase();
    let mut pos = 0;
    while pos < lower.len() {
        match lower[pos..].find(query) {
            Some(rel) => {
                out.push(span_start + pos + rel);
                pos += rel + qlen;
            }
            None => break,
        }
    }
}

struct App {
    /// Processed markdown with `{#slug}` IDs injected into every heading.
    content: String,
    cache: CommonMarkCache,
    query: String,
    /// Byte start positions of matches within `content`, in document order.
    matches: Vec<usize>,
    /// Index of the active (focused) match.
    active: usize,
    /// `(byte_pos, slug)` for each heading, used to scroll to the section
    /// containing the active match.
    heading_positions: Vec<(usize, String)>,
}

impl App {
    fn new() -> Self {
        let (content, slugs) = inject_heading_ids(MARKDOWN);
        let heading_positions = build_heading_positions(&content, &slugs);
        Self {
            content,
            cache: CommonMarkCache::default(),
            query: String::new(),
            matches: Vec::new(),
            active: 0,
            heading_positions,
        }
    }

    /// Rebuild `matches` by searching inside every text-bearing pulldown-cmark
    /// event. This excludes the injected `{#slug}` attributes and link
    /// destinations, which are not visible in the rendered output.
    fn update_matches(&mut self) {
        self.matches.clear();
        self.active = 0;
        if self.query.is_empty() {
            return;
        }
        let query = self.query.to_lowercase();
        let qlen = query.len().max(1);
        for (event, span) in Parser::new_ext(&self.content, pc_opts()).into_offset_iter() {
            if let Event::Text(ref text) | Event::Code(ref text) = event {
                collect_matches(text, span.start, &query, qlen, &mut self.matches);
            }
        }
    }

    /// Push the current match ranges and active range into the cache.
    fn apply_to_cache(&mut self) {
        let qlen = self.query.len();
        let ranges: Vec<std::ops::Range<usize>> =
            self.matches.iter().map(|&s| s..s + qlen).collect();
        self.cache.set_search_ranges(ranges);
        self.cache
            .set_active_search_range(self.matches.get(self.active).map(|&s| s..s + qlen));
    }

    /// Scroll to the heading section that contains the active match.
    fn scroll_to_active_match(&mut self) {
        let Some(&byte_pos) = self.matches.get(self.active) else {
            return;
        };
        if let Some((_, slug)) = self
            .heading_positions
            .iter()
            .rev()
            .find(|(pos, _)| *pos <= byte_pos)
        {
            *self.cache.scroll_to_id_target_mut() = Some(slug.clone());
        }
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
                    self.scroll_to_active_match();
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
                        self.scroll_to_active_match();
                    }
                    if ui.button("Next").clicked() {
                        self.active = (self.active + 1) % count;
                        self.scroll_to_active_match();
                    }
                }

                if !self.query.is_empty() && ui.button("\u{2716}").clicked() {
                    self.query.clear();
                    self.update_matches();
                }
            });

            ui.separator();

            self.apply_to_cache();

            egui::ScrollArea::vertical().show(ui, |ui| {
                CommonMarkViewer::new().enable_scroll_to_heading(true).show(
                    ui,
                    &mut self.cache,
                    &self.content,
                );
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
