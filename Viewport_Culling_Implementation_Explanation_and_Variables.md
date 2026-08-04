## How viewport culling works in `egui_commonmark`

---

### 1. What egui provides at the baseline

egui is an **immediate-mode** GUI. That means every frame you call layout/draw functions in code — there are no retained widget trees. Two egui primitives are essential here:

**`ScrollArea::show`** — renders all content unconditionally. egui clips pixels that fall outside the scroll window, but it still *lays out* every widget. For a large document this is wasteful: hundreds of paragraphs are measured and processed even though only ~10 are visible.

**`ScrollArea::show_viewport`** — a lower-level variant that gives your closure a `viewport: egui::Rect`. This rectangle is in *virtual space*: `viewport.min.y` equals the current scroll offset, and `viewport.max.y` equals scroll offset + visible height. The catch is that **you must tell egui the total content height yourself** via `ui.set_height(h)`, because egui no longer measures it for you. In return, you can skip rendering anything that's off-screen.

Other egui APIs used:
- `ui.next_widget_position()` — screen-space position where the next widget will land.
- `ui.cursor().min.y` — the y-coordinate of the *top* of the current row (important: in a bottom-aligned layout, `next_widget_position().y` is the *bottom* of the row).
- `ui.allocate_space(vec2(w, h))` — reserves space without drawing anything; used to jump over off-screen content.
- `ui.scroll_to_rect(rect, align)` and `ui.scroll_with_delta(delta)` — request a programmatic scroll.
- `ctx.try_load_texture(uri, ...)` returning `TexturePoll::Pending` — tells you an image is still loading.

---

### 2. What egui_commonmark provides at Release 0.24.0

**`CommonMarkCache`** is a long-lived struct (the caller keeps it alive across frames). It holds per-document scrolling state in a `HashMap<egui::Id, ScrollableCache>`.

**`ScrollableCache`** at Release 0.24.0 had three fields:
```
available_size    – window/panel size, to detect resizes
page_size         – Option<Vec2>: total document size once measured, or None
split_points      – Vec<(event_index, start_pos, end_pos)>: where the renderer cut the stream
```

**`CommonMarkViewerInternal`** is the state machine that walks the pulldown-cmark event stream. It holds flags for the current context (inside a list? inside a code block? inside a link? etc.) and emits egui widgets for each event.

**`show_scrollable`** had the rough shape of the current implementation — a two-pass design: first a *full render* to measure everything and record split points, then a *viewport render* that only processes the visible slice. However at Release 0.24.0:
- Split points were recorded at *every* `End(...)` event, including inside lists — which broke the state machine when re-entering mid-list.
- Coordinates were stored in raw **screen space**, which tied their values to the scroll offset at the moment they were recorded (specifically, it forced a `scroll_offset(ZERO)` to guarantee correctness).
- `page_size` was stored as `ui.next_widget_position().to_vec2()` — a raw screen position, not a document-relative height.
- `Link::end` already handled `#fragment` links with `scroll_to_id_target`, and `start_tag` already called `ui.scroll_to_cursor()` when it found a matching heading.

---

### 3. The coordinate problem — and how `viewport_culling` solves it

The root issue is that **screen space and virtual space are different things**:

- **Screen space** — absolute pixel coordinates on screen. `ui.next_widget_position()` returns screen-space values.
- **Virtual space** (content-relative) — position inside the document, where `0` = top of document. `viewport.min.y` and `viewport.max.y` from `show_viewport` are in virtual space.

The relationship is: `virtual_y = screen_y − content_origin_y`

`content_origin_y` is captured once at the very start of the full render:
```rust
let content_origin_y = ui.next_widget_position().y;
```
This is the screen-space y of where the document begins. Every split point's `vstart` and `vend` are then stored as `screen_y − content_origin_y`, giving a value where `0` means "top of document". This means you can record split points at *any* scroll position, and they'll still be directly comparable to `viewport.min.y` / `viewport.max.y` on the next frame.

---

### 4. The two-pass design in full detail

#### Pass 1 — Full render (`page_size == None`)

`show_scrollable` detects `page_size.is_none()` and calls `ScrollArea::vertical().show(...)`, passing `Some(source_id)` as the `split_points_id` argument to `show()`. This enables split-point recording.

Inside `show()`, for every event in the pulldown-cmark stream:

1. **`block_start_position`** — when a `Start(Paragraph | Heading | CodeBlock)` event is seen *at the top level* (not inside a list), the current cursor position is saved. This captures the visual *top* of the block before any of its content is rendered. It's critical for image paragraphs: by the time `End(Paragraph)` fires, the image has already been drawn and the cursor is at the image's *bottom* — so `start_position` there would give the wrong top-of-block coordinate.

2. **`heading_y_positions`** — when a `Start(Heading { id: Some(id) })` is seen, `ui.cursor().min.y − content_origin_y` is stored in the map under that heading's slug. The `cursor().min.y` is used (not `next_widget_position().y`) because this is a bottom-aligned layout, so `next_widget_position` gives the row bottom.

3. After each event is processed, **`is_safe_block_end`** is checked. A safe block end is an `End(Paragraph | Heading | CodeBlock)` at the top level (not inside a list). These are the only positions where the renderer's state machine is guaranteed to be in a clean, restartable state — lists, block quotes, and tables have complex nested state that can't be resumed mid-way. When `is_safe_block_end` is true, a split point `(event_index, vstart, vend)` is recorded:
   - `vstart` = `block_start_position` (the top of the block, in virtual coords)
   - `vend` = current cursor position after `End(Block)`, in virtual coords

4. **`any_image_loading`** — if `Image::end()` returns `< 1.0` (meaning the texture is still `TexturePoll::Pending`), the flag is set. At the end of the render, if any image was loading, the split points and heading positions are *discarded* and `page_size` is left `None`. The image loader will trigger another repaint automatically, and the next frame repeats the full render — until all images have stable heights and split points can be trusted.

5. When all images are stable, `page_size` is committed: `vec2(max_width, final_y − content_origin_y)`.

#### Pass 2 — Viewport culling (`page_size == Some`)

`show_scrollable` calls `ScrollArea::vertical().show_viewport(ui, |ui, viewport| { ... })`.

Inside the closure:

- `ui.set_height(page_size.y)` — tells egui the total document height so the scroll bar is correct.
- The *rendering window* is computed:
  ```
  viewport_height  = viewport.max.y − viewport.min.y
  render_below     = viewport.max.y + viewport_height   (one extra viewport below visible)
  ```
  The extra viewport of headroom means you can scroll fast without seeing blank space.

- **`preceding_split`** — the last split point whose `vend.y < viewport.min.y`. This is the block that ends just above the visible area. Its `vend.y` tells us the virtual position of the "top edge of what we need to render".
  ```rust
  let preceding_split = split_points
      .rfind(|(_, _, vend)| vend.y < viewport.min.y);
  ```

- **`skip_height`** = `preceding_split.vend.y` — the vertical space to allocate *without rendering*, jumping the layout cursor to just before the visible area.

- **`last_event_index`** — the first split point whose `vstart.y > render_below`. Events up to this index are rendered; everything after is skipped.

- **`(skip_count, take_count)`** — event stream slice:
  - When a preceding split was found: start from `preceding_split.event_index + 1` (the split point's `End(Block)` event is already accounted for in `skip_height`, so replaying it would add a duplicate newline), take `last_event_index − preceding_split.event_index` events.
  - When at the top of the document (no preceding split): start from event 0.

The event stream is then `.skip(skip_count).take(take_count)` and processed through the same `process_event` path as in a full render.

---

### 5. Detecting and handling window resize

At the end of `show_scrollable`, `available_size` (the current panel/window dimensions) is compared against the value stored in the cache. If they differ, the cache is invalidated (`page_size = None`, split points cleared) so the next frame does a full re-measure at the new width. Crucially, because `scroll_offset(ZERO)` was removed from the full-render path, this re-render happens at the *current* scroll position — no flash back to the top.

---

### 6. TOC navigation / `scroll_to_id_target`

TOC links use the syntax `[click me!](#my-heading-id)`. How a click gets turned into a scroll depends on which render path is active.

**How the click is captured — link hooks and `#` links:**

In `Link::end()` (called when `End(Link)` is processed):
```
if link_hooks contains this destination  →  mark the hook as clicked (for app-level interception)
else if enable_scroll_to_heading and destination starts with '#'  →  record the slug in deferred_scroll_to_heading
else  →  ui.hyperlink_to(...)  (open in browser)
```

The `deferred_scroll_to_heading` is important: it's set *during* the render loop but only written to `cache.scroll_to_id_target` *after* the loop finishes:
```rust
*cache.scroll_to_id_target_mut() = self.deferred_scroll_to_heading.take();
```
This prevents double-processing on the same frame.

**Full-render / no-cache path:**

In `start_tag()`, whenever a `Start(Heading { id: Some(id) })` is encountered during rendering, the code checks:
```rust
if cache.scroll_to_id_target() == Some(id) {
    ui.scroll_to_cursor(Some(egui::Align::TOP));
    cache.scroll_to_id_target_mut().take();  // consumed
}
```
Because the heading is being *rendered on screen*, egui can scroll so that the cursor is visible.

**Viewport-culling path:**

The problem is that the target heading may be nowhere near the current viewport, so it won't be rendered at all. Instead, `heading_y_positions` is consulted:

```rust
let pending_scroll_y = if let Some(slug) = cache.scroll_to_id_target() {
    sc.heading_y_positions.get(slug).copied()  // virtual y, already known
};
```

Then inside `show_viewport`:
```rust
if let Some(y) = pending_scroll_y {
    let r = Rect::from_min_size(
        pos2(0.0, ui.next_widget_position().y + y),
        Vec2::ZERO,
    );
    ui.scroll_to_rect(r, Some(Align::TOP));
}
```

Why `ui.next_widget_position().y + y`? Inside `show_viewport`, the layout origin is at `screen_top − current_scroll`. So `ui.next_widget_position().y + y` = `(screen_top − current_scroll) + y`, and `scroll_to_rect` with `Align::TOP` will set the scroll so that position appears at `screen_top`, meaning `new_scroll = y`. That's exactly the virtual y of the heading. ✓

---

### 7. Link hooks

Link hooks are a separate mechanism that lets the **application** intercept specific link clicks — for example, a TOC panel that renders `[Section 1](#sec1)` and wants to know when the user clicks that link in order to do something custom (like highlighting the entry in a sidebar, or navigating a separate panel).

Usage pattern:
```rust
cache.add_link_hook("my-custom-action");   // register before rendering
// ... show ...
if cache.get_link_hook("my-custom-action") == Some(true) { /* it was clicked */ }
```

When the renderer encounters a link whose destination is a registered hook, it renders a clickable `ui.link(...)` and sets the hook value to `true` on click — instead of calling `ui.hyperlink_to(...)` which would open a browser URL. All hooks are reset to `false` at the start of each frame by `prepare_show → deactivate_link_hooks`.

The key difference from egui's own `ctx.output_mut(|o| ...)` mechanism is that link hooks are checked *during* rendering, so the renderer can suppress the hover-URL tooltip for hooked links.

---

### 8. Search highlighting

The application provides byte ranges in the source markdown string via:
```rust
cache.set_search_ranges(ranges);           // all matches
cache.set_active_search_range(Some(range)); // the focused match
```

Every `Event::Text(text)` comes with a `src_span: Range<usize>` — the byte offset of that text in the source string, provided by pulldown-cmark's `.into_offset_iter()`.

In `event_text()`, if the text is body text (not inside an image, code block, or link), it calls `render_body_text()` instead of a plain `ui.label()`.

`render_body_text()` works like this:
1. Filter `search_ranges` to those overlapping with `src_span`.
2. Convert their absolute byte offsets to *local* offsets within this text chunk.
3. Walk through the text left-to-right, emitting:
   - A plain label for any prefix before a match.
   - A label with a teal background for a regular match.
   - A label with a violet background for the active/focused match.
   - A plain label for any suffix.

Because `item_spacing.x = 0` is already set on the outer layout, the segments flow together without gaps, appearing as one continuous highlighted run.

---

### Summary of what `viewport_culling` added on top of Release 0.24.0

| Area | Release 0.24.0 | `viewport_culling` |
|---|---|---|
| Split-point trigger | Every `End(...)` event, even inside lists | Only `End(Paragraph\|Heading\|CodeBlock)` at top level |
| Coordinate space | Raw screen space; required `scroll_offset(ZERO)` during full render | Virtual (content-relative), subtracted via `content_origin_y` |
| Block top capture | Used cursor at `End(Block)` (= block bottom) | `block_start_position` captured at `Start(Block)` |
| TOC in viewport mode | Fell back to full render | `heading_y_positions` map enables direct jump |
| Image loading | No detection | `any_image_loading` + `Image::end()` returns height |
| Window resize | Cleared cache | Same, but no jump to top (removed `scroll_offset(ZERO)`) |
| Search | Not present | `search_ranges`, `active_search_range`, `render_body_text()` |
| Programmatic scroll | Not present | `pending_scroll_delta` applied inside scroll area |
| Cache toggle | Always on | `use_viewport_cache` flag, with clean fallback path |
| Table overflow | Not handled | Wrapped in horizontal `ScrollArea` |
