use crate::alerts::AlertBundle;
use egui::{RichText, TextBuffer, TextStyle, Ui, text::LayoutJob};
use std::collections::HashMap;
use std::ops::Range;

use crate::pulldown::ScrollableCache;

#[cfg(feature = "better_syntax_highlighting")]
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::{SyntaxDefinition, SyntaxSet},
    util::LinesWithEndings,
};

#[cfg(feature = "better_syntax_highlighting")]
const DEFAULT_THEME_LIGHT: &str = "base16-ocean.light";
#[cfg(feature = "better_syntax_highlighting")]
const DEFAULT_THEME_DARK: &str = "base16-ocean.dark";

pub struct CommonMarkOptions<'f> {
    pub indentation_spaces: usize,
    pub max_image_width: Option<usize>,
    pub show_alt_text_on_hover: bool,
    pub default_width: Option<usize>,
    #[cfg(feature = "better_syntax_highlighting")]
    pub theme_light: String,
    #[cfg(feature = "better_syntax_highlighting")]
    pub theme_dark: String,
    pub use_explicit_uri_scheme: bool,
    pub default_implicit_uri_scheme: String,
    pub alerts: AlertBundle,
    /// Whether to present a mutable ui for things like checkboxes
    pub mutable: bool,
    pub math_fn: Option<&'f crate::RenderMathFn>,
    pub html_fn: Option<&'f crate::RenderHtmlFn>,
    /// Whether to enable scrolling to headings by their ID.
    /// To give a heading an ID, use the syntax `# Heading {#myheadingid}`. Then links to `#myheadingid` e.g. `[click me!](#myheadingid)` will scroll to that heading.
    pub enable_scroll_to_heading: bool,
    /// When `true`, `show_scrollable` only renders the visible slice of the
    /// document each frame. When `false` (the default) the full document is
    /// rendered every frame and egui clips what is off-screen.
    pub use_viewport_cache: bool,
    /// Background colour for passive search matches. When `None`, a
    /// theme-derived default is used (see [`crate::search::default_match_bg`]).
    pub search_match_bg: Option<egui::Color32>,
    /// Background colour for the active (focused) search match. When
    /// `None`, a theme-derived default is used (see
    /// [`crate::search::default_active_match_bg`]).
    pub search_active_match_bg: Option<egui::Color32>,
}

impl std::fmt::Debug for CommonMarkOptions<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("CommonMarkOptions");

        s.field("indentation_spaces", &self.indentation_spaces)
            .field("max_image_width", &self.max_image_width)
            .field("show_alt_text_on_hover", &self.show_alt_text_on_hover)
            .field("default_width", &self.default_width);

        #[cfg(feature = "better_syntax_highlighting")]
        s.field("theme_light", &self.theme_light)
            .field("theme_dark", &self.theme_dark);

        s.field("use_explicit_uri_scheme", &self.use_explicit_uri_scheme)
            .field(
                "default_implicit_uri_scheme",
                &self.default_implicit_uri_scheme,
            )
            .field("alerts", &self.alerts)
            .field("mutable", &self.mutable)
            .field("search_match_bg", &self.search_match_bg)
            .field("search_active_match_bg", &self.search_active_match_bg)
            .finish()
    }
}

impl Default for CommonMarkOptions<'_> {
    fn default() -> Self {
        Self {
            indentation_spaces: 4,
            max_image_width: None,
            show_alt_text_on_hover: true,
            default_width: None,
            #[cfg(feature = "better_syntax_highlighting")]
            theme_light: DEFAULT_THEME_LIGHT.to_owned(),
            #[cfg(feature = "better_syntax_highlighting")]
            theme_dark: DEFAULT_THEME_DARK.to_owned(),
            use_explicit_uri_scheme: false,
            default_implicit_uri_scheme: "file://".to_owned(),
            alerts: AlertBundle::gfm(),
            mutable: false,
            math_fn: None,
            html_fn: None,
            enable_scroll_to_heading: false,
            use_viewport_cache: false,
            search_match_bg: None,
            search_active_match_bg: None,
        }
    }
}

impl CommonMarkOptions<'_> {
    #[cfg(feature = "better_syntax_highlighting")]
    pub fn curr_theme(&self, ui: &Ui) -> &str {
        if ui.style().visuals.dark_mode {
            &self.theme_dark
        } else {
            &self.theme_light
        }
    }

    pub fn max_width(&self, ui: &Ui) -> f32 {
        let max_image_width = self.max_image_width.unwrap_or(0) as f32;
        let available_width = ui.available_width();

        let max_width = max_image_width.max(available_width);
        if let Some(default_width) = self.default_width {
            if default_width as f32 > max_width {
                default_width as f32
            } else {
                max_width
            }
        } else {
            max_width
        }
    }

    /// The background colour to use for passive search matches: the
    /// explicit override if one was set, otherwise a theme-derived default.
    pub fn search_match_bg(&self, ui: &Ui) -> egui::Color32 {
        self.search_match_bg
            .unwrap_or_else(|| crate::search::default_match_bg(ui.visuals()))
    }

    /// The background colour to use for the active search match: the
    /// explicit override if one was set, otherwise a theme-derived default.
    pub fn search_active_match_bg(&self, ui: &Ui) -> egui::Color32 {
        self.search_active_match_bg
            .unwrap_or_else(|| crate::search::default_active_match_bg(ui.visuals()))
    }
}

#[derive(Default, Clone)]
pub struct Style {
    pub heading: Option<u8>,
    pub strong: bool,
    pub emphasis: bool,
    pub strikethrough: bool,
    pub quote: bool,
    pub code: bool,
}

impl Style {
    pub fn to_richtext(&self, ui: &Ui, text: &str) -> RichText {
        let mut text = RichText::new(text);

        if let Some(level) = self.heading {
            let max_height = ui
                .style()
                .text_styles
                .get(&TextStyle::Heading)
                .map_or(32.0, |d| d.size);
            let min_height = ui
                .style()
                .text_styles
                .get(&TextStyle::Body)
                .map_or(14.0, |d| d.size);
            let diff = max_height - min_height;

            match level {
                0 => {
                    text = text.strong().heading();
                }
                1 => {
                    let size = min_height + diff * 0.835;
                    text = text.strong().size(size);
                }
                2 => {
                    let size = min_height + diff * 0.668;
                    text = text.strong().size(size);
                }
                3 => {
                    let size = min_height + diff * 0.501;
                    text = text.strong().size(size);
                }
                4 => {
                    let size = min_height + diff * 0.334;
                    text = text.size(size);
                }
                // We only support 6 levels
                5.. => {
                    let size = min_height + diff * 0.167;
                    text = text.size(size);
                }
            }
        }

        if self.quote {
            text = text.weak();
        }

        if self.strong {
            text = text.strong();
        }

        if self.emphasis {
            // FIXME: Might want to add some space between the next text
            text = text.italics();
        }

        if self.strikethrough {
            text = text.strikethrough();
        }

        if self.code {
            text = text.code();
        }

        text
    }
}

#[derive(Default)]
pub struct Link {
    pub destination: String,
    pub text: Vec<RichText>,
    /// For each accumulated `text` piece, its local byte range within the
    /// final rendered job's text paired with its byte range in the original
    /// source. Used to translate global search match ranges into positions
    /// local to this link's rendered text.
    pub chunks: Vec<(Range<usize>, Range<usize>)>,
}

impl Link {
    /// Append a piece of link text, recording its source span so search
    /// matches can later be mapped back onto it.
    pub fn push_text(&mut self, text: RichText, src_span: Range<usize>) {
        let local_start: usize = self.text.iter().map(|t| t.text().len()).sum();
        let local_end = local_start + text.text().len();
        self.chunks.push((local_start..local_end, src_span));
        self.text.push(text);
    }

    /// Renders the link. If `want_scroll_to_active_match` is true and the
    /// currently active search match falls inside this link's text, the view
    /// is scrolled (centering the link) and `true` is returned so the caller
    /// knows the request has been fulfilled.
    pub fn end(
        self,
        ui: &mut Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        scroll_to_heading: &mut Option<String>,
        want_scroll_to_active_match: bool,
    ) -> bool {
        let Self {
            destination,
            text,
            chunks,
        } = self;

        // When a link wraps an image (`[![alt](img)](url)`), all text events are captured
        // by the image widget and link.text is never populated. Rendering an empty Label in
        // a wrapping layout resets cursor.min.x to 0, superimposing subsequent elements on
        // the image that was just drawn. Nothing to render, so return early.
        if text.is_empty() {
            return false;
        }

        let intervals = crate::search::chunked_search_intervals(
            &chunks,
            cache.search_ranges(),
            cache.active_search_range(),
        );
        let has_active_match = intervals.iter().any(|(_, is_active)| *is_active);

        let mut layout_job = LayoutJob::default();
        for t in text {
            t.append_to(
                &mut layout_job,
                ui.style(),
                egui::FontSelection::Default,
                egui::Align::LEFT,
            );
        }
        if !intervals.is_empty() {
            crate::search::apply_search_highlights(
                &mut layout_job,
                &intervals,
                options.search_match_bg(ui),
                options.search_active_match_bg(ui),
            );
        }

        let response = if cache.link_hooks().contains_key(&destination) {
            let ui_link = ui.link(layout_job);
            if ui_link.clicked() || ui_link.middle_clicked() {
                cache.link_hooks_mut().insert(destination, true);
            }
            ui_link
        } else if options.enable_scroll_to_heading
            && let Some(stripped) = destination.strip_prefix("#")
        {
            let response = ui.link(layout_job);
            if response.clicked() {
                scroll_to_heading.replace(stripped.to_string());
            };
            response
        } else {
            ui.hyperlink_to(layout_job, destination)
        };

        if want_scroll_to_active_match && has_active_match {
            ui.scroll_to_rect(response.rect, Some(egui::Align::Center));
            true
        } else {
            false
        }
    }
}

pub struct Image {
    pub uri: String,
    pub alt_text: Vec<RichText>,
}

impl Image {
    // FIXME: string conversion
    pub fn new(uri: &str, options: &CommonMarkOptions) -> Self {
        let has_scheme = uri.contains("://") || uri.starts_with("data:");
        let uri = if options.use_explicit_uri_scheme || has_scheme {
            uri.to_string()
        } else {
            // Assume file scheme
            format!("{}{uri}", options.default_implicit_uri_scheme)
        };

        Self {
            uri,
            alt_text: Vec::new(),
        }
    }

    /// Returns the rendered height in points, or `0.0` if the texture is still
    /// loading. The caller uses this to detect whether split-point heights can
    /// be trusted.
    pub fn end(self, ui: &mut Ui, options: &CommonMarkOptions) -> f32 {
        let Self { uri, alt_text } = self;
        let response = ui.add(
            egui::Image::from_uri(&uri)
                .fit_to_original_size(1.0)
                .max_width(options.max_width(ui)),
        );
        let height = response.rect.height();
        if !alt_text.is_empty() && options.show_alt_text_on_hover {
            response.on_hover_ui_at_pointer(|ui| {
                for alt in alt_text {
                    ui.label(alt);
                }
            });
        }
        // egui's 24×24 placeholder means height ≥ 1.0 even while Pending, so
        // query the load state directly rather than relying on height alone.
        let is_pending = matches!(
            ui.ctx().try_load_texture(
                &uri,
                egui::TextureOptions::default(),
                egui::load::SizeHint::default(),
            ),
            Ok(egui::load::TexturePoll::Pending { .. })
        );
        if is_pending { 0.0 } else { height }
    }
}

#[derive(Default)]
pub struct CodeBlock {
    pub lang: Option<String>,
    pub content: String,
    /// For each chunk of text appended to `content` (one per markdown text
    /// event), the local byte range within `content` paired with its byte
    /// range in the original source text. Used to translate global search
    /// match ranges into positions local to this code block.
    pub chunks: Vec<(Range<usize>, Range<usize>)>,
}

impl CodeBlock {
    /// Append a chunk of text to the code block's content, recording its
    /// source span so search matches can later be mapped back onto it.
    pub fn push_text(&mut self, text: &str, src_span: Range<usize>) {
        let start = self.content.len();
        self.content.push_str(text);
        self.chunks.push((start..self.content.len(), src_span));
    }

    /// Renders the code block. If `want_scroll_to_active_match` is true and
    /// the currently active search match falls inside this block, the view
    /// is scrolled (centering the match) and `true` is returned so the
    /// caller knows the request has been fulfilled.
    pub fn end(
        &self,
        ui: &mut Ui,
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        max_width: f32,
        want_scroll_to_active_match: bool,
    ) -> bool {
        let intervals = crate::search::chunked_search_intervals(
            &self.chunks,
            cache.search_ranges(),
            cache.active_search_range(),
        );
        let scroll_to_active_match = want_scroll_to_active_match
            .then(|| intervals.iter().find(|(_, is_active)| *is_active))
            .flatten()
            .map(|(range, _)| range.clone());
        let did_scroll = scroll_to_active_match.is_some();

        ui.scope(|ui| {
            Self::pre_syntax_highlighting(cache, options, ui);

            let mut layout = |ui: &Ui, string: &dyn TextBuffer, wrap_width: f32| {
                let mut job = if let Some(lang) = &self.lang {
                    self.syntax_highlighting(cache, options, lang, ui, string.as_str())
                } else {
                    plain_highlighting(ui, string.as_str())
                };

                if !intervals.is_empty() {
                    crate::search::apply_search_highlights(
                        &mut job,
                        &intervals,
                        options.search_match_bg(ui),
                        options.search_active_match_bg(ui),
                    );
                }

                job.wrap.max_width = wrap_width;
                ui.fonts_mut(|f| f.layout_job(job))
            };

            crate::elements::code_block(
                ui,
                max_width,
                &self.content,
                &mut layout,
                scroll_to_active_match,
            );
        });

        did_scroll
    }
}

#[cfg(not(feature = "better_syntax_highlighting"))]
impl CodeBlock {
    fn pre_syntax_highlighting(
        _cache: &mut CommonMarkCache,
        _options: &CommonMarkOptions,
        ui: &mut Ui,
    ) {
        ui.style_mut().visuals.extreme_bg_color = ui.visuals().extreme_bg_color;
    }

    fn syntax_highlighting(
        &self,
        _cache: &mut CommonMarkCache,
        _options: &CommonMarkOptions,
        extension: &str,
        ui: &Ui,
        text: &str,
    ) -> egui::text::LayoutJob {
        simple_highlighting(ui, text, extension)
    }
}

#[cfg(feature = "better_syntax_highlighting")]
impl CodeBlock {
    fn pre_syntax_highlighting(
        cache: &mut CommonMarkCache,
        options: &CommonMarkOptions,
        ui: &mut Ui,
    ) {
        let curr_theme = cache.curr_theme(ui, options);
        let style = ui.style_mut();

        style.visuals.extreme_bg_color = curr_theme
            .settings
            .background
            .map(syntect_color_to_egui)
            .unwrap_or(style.visuals.extreme_bg_color);

        if let Some(color) = curr_theme.settings.selection_foreground {
            style.visuals.selection.bg_fill = syntect_color_to_egui(color);
        }
    }

    fn syntax_highlighting(
        &self,
        cache: &CommonMarkCache,
        options: &CommonMarkOptions,
        extension: &str,
        ui: &Ui,
        text: &str,
    ) -> egui::text::LayoutJob {
        if let Some(syntax) = cache.ps.find_syntax_by_extension(extension) {
            let mut job = egui::text::LayoutJob::default();
            let mut h = HighlightLines::new(syntax, cache.curr_theme(ui, options));

            for line in LinesWithEndings::from(text) {
                let ranges = h.highlight_line(line, &cache.ps).unwrap();
                for v in ranges {
                    let front = v.0.foreground;
                    job.append(
                        v.1,
                        0.0,
                        egui::TextFormat::simple(
                            TextStyle::Monospace.resolve(ui.style()),
                            syntect_color_to_egui(front),
                        ),
                    );
                }
            }

            job
        } else {
            simple_highlighting(ui, text, extension)
        }
    }
}

fn simple_highlighting(ui: &Ui, text: &str, extension: &str) -> egui::text::LayoutJob {
    egui_extras::syntax_highlighting::highlight(
        ui.ctx(),
        ui.style(),
        &egui_extras::syntax_highlighting::CodeTheme::from_style(ui.style()),
        text,
        extension,
    )
}

fn plain_highlighting(ui: &Ui, text: &str) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        text,
        0.0,
        egui::TextFormat::simple(
            TextStyle::Monospace.resolve(ui.style()),
            ui.style().visuals.text_color(),
        ),
    );
    job
}

#[cfg(feature = "better_syntax_highlighting")]
fn syntect_color_to_egui(color: syntect::highlighting::Color) -> egui::Color32 {
    egui::Color32::from_rgb(color.r, color.g, color.b)
}

#[cfg(feature = "better_syntax_highlighting")]
fn default_theme(ui: &Ui) -> &str {
    if ui.style().visuals.dark_mode {
        DEFAULT_THEME_DARK
    } else {
        DEFAULT_THEME_LIGHT
    }
}

/// A cache used for storing content such as images.
#[derive(Debug)]
pub struct CommonMarkCache {
    // Everything stored in `CommonMarkCache` must take into account that
    // the cache is for multiple `CommonMarkviewer`s with different source_ids.
    #[cfg(feature = "better_syntax_highlighting")]
    ps: SyntaxSet,

    #[cfg(feature = "better_syntax_highlighting")]
    ts: ThemeSet,

    /// The ID of the heading to scroll to. This is set when a link whose destination is a fragment (e.g. `#my-heading`) has been clicked.
    scroll_to_id_target: Option<String>,
    link_hooks: HashMap<String, bool>,

    scroll: HashMap<egui::Id, ScrollableCache>,
    pub(self) has_installed_loaders: bool,
    /// Keyboard / programmatic scroll delta applied inside the next
    /// `show_scrollable` call and then cleared.
    pub pending_scroll_delta: egui::Vec2,

    /// The search query text
    pub search_query: String,
    /// Byte ranges (into the source text passed to the viewer) that should
    /// be highlighted as search matches.
    search_ranges: Vec<Range<usize>>,
    /// The currently active (focused) search match, highlighted more
    /// prominently than the others.
    active_search_range: Option<Range<usize>>,
    /// The ordinal number of the active search match
    active_match: Option<usize>,
    /// Set by [`CommonMarkCache::scroll_to_active_search_match`] and cleared
    /// once the render pass that finds and scrolls to the active match runs.
    pending_scroll_to_active_match: bool,
    /// Number of consecutive internal retries (viewport blind-scroll not
    /// yet having brought the match into a rendered slice). Bounds an
    /// otherwise-unlikely non-convergent loop; reset whenever the user
    /// requests a fresh scroll via [`CommonMarkCache::scroll_to_active_search_match`].
    pending_scroll_to_active_match_retries: u8,
}

#[allow(clippy::derivable_impls)]
impl Default for CommonMarkCache {
    fn default() -> Self {
        Self {
            #[cfg(feature = "better_syntax_highlighting")]
            ps: SyntaxSet::load_defaults_newlines(),
            #[cfg(feature = "better_syntax_highlighting")]
            ts: ThemeSet::load_defaults(),
            link_hooks: HashMap::new(),
            scroll: Default::default(),
            scroll_to_id_target: None,
            has_installed_loaders: false,
            pending_scroll_delta: egui::Vec2::ZERO,
            search_query: String::new(),
            search_ranges: Vec::new(),
            active_search_range: None,
            active_match: None,
            pending_scroll_to_active_match: false,
            pending_scroll_to_active_match_retries: 0,
        }
    }
}

impl CommonMarkCache {
    #[cfg(feature = "better_syntax_highlighting")]
    pub fn add_syntax_from_folder(&mut self, path: &str) {
        let mut builder = self.ps.clone().into_builder();
        let _ = builder.add_from_folder(path, true);
        self.ps = builder.build();
    }

    #[cfg(feature = "better_syntax_highlighting")]
    pub fn add_syntax_from_str(
        &mut self,
        s: &str,
        fallback_name: Option<&str>,
    ) -> Result<(), syntect::parsing::ParseSyntaxError> {
        let mut builder = self.ps.clone().into_builder();
        SyntaxDefinition::load_from_str(s, true, fallback_name).map(|d| builder.add(d))?;
        self.ps = builder.build();
        Ok(())
    }

    #[cfg(feature = "better_syntax_highlighting")]
    /// Add more color themes for code blocks(.tmTheme files). Set the color theme with
    /// [`syntax_theme_dark`](CommonMarkViewer::syntax_theme_dark) and
    /// [`syntax_theme_light`](CommonMarkViewer::syntax_theme_light)
    pub fn add_syntax_themes_from_folder(
        &mut self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<(), syntect::LoadingError> {
        self.ts.add_from_folder(path)
    }

    #[cfg(feature = "better_syntax_highlighting")]
    /// Add color theme for code blocks(.tmTheme files). Set the color theme with
    /// [`syntax_theme_dark`](CommonMarkViewer::syntax_theme_dark) and
    /// [`syntax_theme_light`](CommonMarkViewer::syntax_theme_light)
    pub fn add_syntax_theme_from_bytes(
        &mut self,
        name: impl Into<String>,
        bytes: &[u8],
    ) -> Result<(), syntect::LoadingError> {
        let mut cursor = std::io::Cursor::new(bytes);
        self.ts
            .themes
            .insert(name.into(), ThemeSet::load_from_reader(&mut cursor)?);
        Ok(())
    }

    /// Get the desired fragment. This is the id which will be scrolled to if it is found in the markdown.
    pub fn scroll_to_id_target(&self) -> Option<&str> {
        self.scroll_to_id_target.as_deref()
    }

    /// Get mutable access to the desired fragment. Setting this will cause the viewer to scroll to the heading with this id if it exists. Setting it to None will prevent scrolling.
    pub fn scroll_to_id_target_mut(&mut self) -> &mut Option<String> {
        &mut self.scroll_to_id_target
    }

    /// Accumulate a scroll delta to be applied inside the next [`show_scrollable`] or
    /// [`apply_pending_scroll_delta`] call and then cleared.
    /// Positive y scrolls toward the top; negative toward the bottom.
    /// Multiple calls before the next frame are summed.
    ///
    /// This is the preferred way to drive keyboard or programmatic scrolling when using
    /// [`show_scrollable`], because the scroll area is owned internally and cannot be
    /// reached directly by the caller.
    ///
    /// [`show_scrollable`]: crate::CommonMarkViewer::show_scrollable
    pub fn set_scroll_delta(&mut self, delta: egui::Vec2) {
        self.pending_scroll_delta += delta;
    }

    /// To apply scrolling without `show_scrollable`, call this function immediately before
    /// or after `show`.
    pub fn apply_pending_scroll_delta(&mut self, ui: &Ui) {
        let delta = std::mem::replace(&mut self.pending_scroll_delta, egui::Vec2::ZERO);
        if delta != egui::Vec2::ZERO {
            ui.scroll_with_delta(delta);
        }
    }

    /// Set the byte ranges (into the source text passed to the viewer) that
    /// should be highlighted as search matches. Ranges must fall on valid
    /// UTF-8 character boundaries.
    ///
    /// This is cheap to call every frame (e.g. while the user is typing into
    /// a search box): highlighting never changes the number of widgets that
    /// get rendered, so it cannot desync egui's widget IDs.
    pub fn set_search_ranges(&mut self, ranges: Vec<Range<usize>>) {
        self.search_ranges = ranges;
    }

    /// The search match ranges currently set for highlighting.
    pub fn search_ranges(&self) -> &[Range<usize>] {
        &self.search_ranges
    }

    /// Approximate the source byte offset of whatever is currently at the
    /// top of the viewport for the given [`show_scrollable`](crate::CommonMarkViewer::show_scrollable)
    /// instance. Useful for implementing "search from the current position"
    /// (like a typical find-in-page: jump to the nearest match at or after
    /// what's currently on screen, instead of always restarting from the top
    /// of the document).
    ///
    /// Returns `None` if nothing has been rendered for `source_id` yet, or
    /// if it was rendered with [`viewport_cache`](crate::CommonMarkViewer::viewport_cache)
    /// disabled (in which case the whole document is visible-ish anyway).
    pub fn viewport_start_byte_offset(&self, source_id: impl egui::AsId) -> Option<usize> {
        let sc = self.scroll.get(&egui::Id::new(source_id))?;
        sc.byte_offset_for_virtual_y(sc.last_viewport_top_y)
    }

    /// Set the active (focused) search match, which is highlighted more
    /// prominently than the other matches. Pass `None` to clear it.
    ///
    /// This does not by itself scroll the view; call
    /// [`scroll_to_active_search_match`](Self::scroll_to_active_search_match)
    /// as well if you want that (typically when the user moves to the next/
    /// previous match).
    pub fn set_active_search_range(&mut self, range: Option<Range<usize>>) {
        self.active_search_range = range;
    }

    /// The currently active (focused) search match, if any.
    pub fn active_search_range(&self) -> Option<&Range<usize>> {
        self.active_search_range.as_ref()
    }

    /// The ordinal number of the currently active (focused) search match, if any.
    pub fn active_match(&self) -> Option<usize> {
        self.active_match
    }

    /// Request that the view scroll so that the active search match (set via
    /// [`set_active_search_range`](Self::set_active_search_range)) becomes
    /// visible, centered in the viewport where possible. The request is
    /// consumed by the next render.
    ///
    /// This never forces a full re-render of the document, even in
    /// [`show_scrollable`](crate::CommonMarkViewer::show_scrollable)'s
    /// viewport-cached mode: if the match isn't already in the currently
    /// rendered slice, the view is scrolled toward its approximate position
    /// (using data already collected by the last full render) and refined
    /// precisely over the following frame or two as the real slice comes
    /// into view.
    pub fn scroll_to_active_search_match(&mut self) {
        self.pending_scroll_to_active_match = true;
        self.pending_scroll_to_active_match_retries = 0;
    }

    /// Takes (and clears) the pending scroll-to-active-match request. Used
    /// internally by the renderer.
    pub fn take_pending_scroll_to_active_match(&mut self) -> bool {
        std::mem::take(&mut self.pending_scroll_to_active_match)
    }

    /// Re-arms the pending scroll-to-active-match request for another
    /// attempt (the match wasn't in the slice rendered this frame), unless
    /// the retry budget has been exhausted, in which case the request is
    /// dropped and `false` is returned. Used internally by the renderer to
    /// bound an otherwise-unlikely non-convergent blind-scroll loop.
    pub fn retry_scroll_to_active_match(&mut self) -> bool {
        const MAX_RETRIES: u8 = 8;
        if self.pending_scroll_to_active_match_retries >= MAX_RETRIES {
            self.pending_scroll_to_active_match = false;
            return false;
        }
        self.pending_scroll_to_active_match_retries += 1;
        self.pending_scroll_to_active_match = true;
        true
    }

    /// Recomputes `search_ranges` from the *rendered* text only (via
    /// pulldown-cmark's `Text`/`Code` events), so link destinations,
    /// heading `{#id}` attribute syntax, and other non-visible markdown
    /// syntax are never matched (a naive substring search over the raw
    /// source would, for example, double-count "500" in
    /// `[Section 500](#section-500)`: once in the visible text, once in the
    /// URL).
    ///
    /// Recomputes `search_ranges` on every keystroke and immediately
    /// advances to the nearest match at or after wherever the user is
    /// currently scrolled to (wrapping to the first match if there is none
    /// after that point), mirroring how a normal "find in page" behaves.
    /// Recomputation and the resulting scroll are both cheap (see
    /// `CommonMarkCache::scroll_to_active_search_match`'s docs: this never
    /// forces a full document re-render), so this shoud stay responsive.
    pub fn update_search_matches(&mut self, egui_source_id: &str, content: &str) {
        self.search_ranges.clear();
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
            let parser = pulldown_cmark::Parser::new_ext(content, options).into_offset_iter();
            for (event, range) in parser {
                let (pulldown_cmark::Event::Text(text) | pulldown_cmark::Event::Code(text)) = event
                else {
                    continue;
                };
                let haystack = text.to_lowercase();
                let mut start = 0;
                while let Some(pos) = haystack[start..].find(&query) {
                    let match_start = range.start + start + pos;
                    let match_end = match_start + query.len();
                    self.search_ranges.push(match_start..match_end);
                    start += pos + query.len();
                }
            }
        }

        if self.search_ranges.is_empty() {
            self.active_match = None;
            self.sync_active_match();
            return;
        }

        let cursor = self.viewport_start_byte_offset(egui_source_id).unwrap_or(0);
        dbg!(cursor);
        let nearest = self
            .search_ranges
            .iter()
            .position(|r| r.start >= cursor)
            .unwrap_or(0);
        self.active_match = Some(nearest);
        self.sync_active_match();
        self.scroll_to_active_search_match();
    }

    // Update the active search range to the desired ordinal value
    fn sync_active_match(&mut self) {
        self.set_active_search_range(
            self.active_match
                .and_then(|i| self.search_ranges.get(i))
                .cloned(),
        );
    }

    /// Scroll back or forward `delta` matches (according to the sign of `delta`) if applicable
    pub fn go_to_match(&mut self, delta: isize) {
        if self.search_ranges.is_empty() {
            return;
        }
        let len = self.search_ranges.len() as isize;
        let next = match self.active_match {
            Some(i) => (i as isize + delta).rem_euclid(len),
            // First navigation after a fresh search: start at the first
            // match for Next, the last one for Previous.
            None if delta >= 0 => 0,
            None => len - 1,
        };
        self.active_match = Some(next as usize);
        self.sync_active_match();
        self.scroll_to_active_search_match();
    }

    /// Clear the cache for all scrollable elements
    pub fn clear_scrollable(&mut self) {
        self.scroll.clear();
    }

    /// Clear the cache for a specific scrollable viewer. Returns false if the
    /// id was not in the cache.
    pub fn clear_scrollable_with_id(&mut self, source_id: impl egui::AsId) -> bool {
        self.scroll.remove(&egui::Id::new(source_id)).is_some()
    }

    /// If the user clicks on a link in the markdown render that has `name` as a link. The hook
    /// specified with this method will be set to true. It's status can be acquired
    /// with [`get_link_hook`](Self::get_link_hook). Be aware that all hook state is reset once
    /// [`CommonMarkViewer::show`] gets called
    ///
    /// # Why use link hooks
    ///
    /// egui provides a method for checking links afterwards so why use this instead?
    ///
    /// ```rust
    /// # use egui::__run_test_ctx;
    /// # __run_test_ctx(|ctx| {
    /// ctx.output_mut(|o| for command in &o.commands {
    ///     matches!(command, egui::OutputCommand::OpenUrl(_));
    /// });
    /// # });
    /// ```
    ///
    /// The main difference is that link hooks allows egui_commonmark to check for link hooks
    /// while rendering. Normally when hovering over a link, egui_commonmark will display the full
    /// url. With link hooks this feature is disabled, but to do that all hooks must be known.
    // Works when displayed through egui_commonmark
    #[allow(rustdoc::broken_intra_doc_links)]
    pub fn add_link_hook<S: Into<String>>(&mut self, name: S) {
        self.link_hooks.insert(name.into(), false);
    }

    /// Returns None if the link hook could not be found. Returns the last known status of the
    /// hook otherwise.
    pub fn remove_link_hook(&mut self, name: &str) -> Option<bool> {
        self.link_hooks.remove(name)
    }

    /// Get status of link. Returns true if it was clicked
    pub fn get_link_hook(&self, name: &str) -> Option<bool> {
        self.link_hooks.get(name).copied()
    }

    /// Remove all link hooks
    pub fn link_hooks_clear(&mut self) {
        self.link_hooks.clear();
    }

    /// All link hooks
    pub fn link_hooks(&self) -> &HashMap<String, bool> {
        &self.link_hooks
    }

    /// Raw access to link hooks
    pub fn link_hooks_mut(&mut self) -> &mut HashMap<String, bool> {
        &mut self.link_hooks
    }

    /// Set all link hooks to false
    fn deactivate_link_hooks(&mut self) {
        for v in self.link_hooks.values_mut() {
            *v = false;
        }
    }

    #[cfg(feature = "better_syntax_highlighting")]
    fn curr_theme(&self, ui: &Ui, options: &CommonMarkOptions) -> &Theme {
        self.ts
            .themes
            .get(options.curr_theme(ui))
            // Since we have called load_defaults, the default theme *should* always be available..
            .unwrap_or_else(|| &self.ts.themes[default_theme(ui)])
    }
}

pub fn scroll_cache<'a>(cache: &'a mut CommonMarkCache, id: &egui::Id) -> &'a mut ScrollableCache {
    if !cache.scroll.contains_key(id) {
        cache.scroll.insert(*id, Default::default());
    }
    cache.scroll.get_mut(id).unwrap()
}

/// Should be called before any rendering
pub fn prepare_show(cache: &mut CommonMarkCache, ctx: &egui::Context) {
    if !cache.has_installed_loaders {
        // Even though the install function can be called multiple times, its not the cheapest
        // so we ensure that we only call it once.
        // This could be done at the creation of the cache, however it is better to keep the
        // cache free from egui's Ui and Context types as this allows it to be created before
        // any egui instances. It also keeps the API similar to before the introduction of the
        // image loaders.
        #[cfg(feature = "embedded_image")]
        crate::data_url_loader::install_loader(ctx);

        egui_extras::install_image_loaders(ctx);
        cache.has_installed_loaders = true;
    }

    cache.deactivate_link_hooks();
}
