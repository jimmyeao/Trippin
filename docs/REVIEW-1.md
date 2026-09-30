# UI redesign — review of `feat/ui-redesign` (round 1)

Reviewed from source (`panel.rs`, `ui_theme.rs`, `editor.rs`) against `DEVIN.md` and the mockups.
I haven't seen the running app yet. Attach screenshots of each page at 900×640 to the PR so the next round can be reviewed visually.

## What's good — keep it
- `ui_theme.rs`: the tokens are exact and `apply()` is wired in. `card()`, `pill()`, `key_badge()` and `segmented_wide()` are the right building blocks.
- Thumbnails: requested from the render thread, cached to `data_dir()/thumbs`, re-requested when missing. Dancer tiles use mid-frame PNGs.
- Palette swatches are drawn from the real LUT (`palettes::lut`), not approximations.
- Pads fire real `Action`s and show the user's actual key binding.
- Timeline editor: lane colours, a right-hand inspector, a library panel and a red playhead are all in place.

---

## Where it went wrong

### 1. It built 1a *and* 1b together instead of choosing
The brief said: **1b is the layout, top tabs, and Scenes merges into Perform. 1a is styling reference only.**
What was built instead:
- the 1a sidebar (148px);
- a separate 1a "Show" page, which is the default tab;
- 1b as a second page, "Perform".

The results:
- **Everything is duplicated.** Prev/Next, Mode and Bars appear on both Show and Perform.
- **Cramped layout.** On Perform, the 148px sidebar + 292px inspector + scene grid have to fit in 980px. The library ends up ~520px wide, about 3 tiles. The mockup has 4+.
- **The app opens on the wrong page.** Users land on Show, not the library.
- **Director settings are split up.** Breakdowns, drop-cut, random order and sync live on Show, not in the inspector's "Director & sync" section as specified.

### 2. Panels are declared in the wrong order, then patched with magic numbers
- The header is painted into `ui` *before* `Panel::left("nav")`.
- `Panel::right("inspector")` is declared *after* the segmented Library/Pads control.
- egui side panels must be laid out before any central content. Getting this wrong is what caused "the central panel underlaps the inspector by ~15px".
- The workarounds are symptoms of the ordering bug:
  - `available_width() - 16.0` and `- 24.0` fudges;
  - `horizontal_scroll_offset(0.0)` pinned everywhere;
  - the scrollbar hidden (`AlwaysHidden`) so a stray handle doesn't show in the gap.

### 3. Hidden scrollbars
The scene grid has `ScrollBarVisibility::AlwaysHidden`. With 130 scenes, users need to see that the list scrolls and how far down they are. The mockup shows no scrollbar only because it's a static image.

### 4. The grid doesn't fill its width
- Tiles are a fixed 150×108, laid out in `horizontal` rows, with the column count computed from a hard-coded 160.
- This leaves an uneven empty strip on the right, and tiles don't grow to fill the width.
- The mockup uses equal-width columns that fill the width.

### 5. Placeholder and painting bugs
- **Stripes spill outside their box.** The `preview_card` stripes are 10px-thick `line_segment`s running past the rect with no clip, so they bleed outside the card's rounded corners. Use `painter.with_clip_rect(rect)`.
- **Two different placeholders.** Tile placeholders show the first two letters of the name on a flat fill, while preview cards use stripes. Use one: the striped one, clipped.
- **Caption width is guessed.** It's computed as `caption.len() * 7.0`. Measure it with `painter.layout_no_wrap(...)` and use the galley's width.
- **Name truncation can panic.** `&name[..19]` slices bytes and will panic on a non-ASCII scene name. Use `chars().take(n)` or let egui elide the text (`Label::truncate()`).
- **The seasonal tag uses "❄".** egui's default fonts may not include that glyph. Use the text "season", as in the mockup.

### 6. The header drifted from the mockup
- It has two lines plus a groove meter, a percentage *printed inside a 4px progress bar*, and the device name.
- The 1b header is **one row that shares the tab bar**: tabs on the left; fps · BPM · pips · pill on the right.
- `show_percentage()` on a 4px bar is unreadable. Remove it.
- Pills are added in right-to-left order, so they display reversed compared with the mockup.

### 7. Perform view (1c) isn't glanceable
- **Small previews.** Now/Next are 96×54 thumbnails in small cards. The mockup has large previews (≈40% of the window height), with an accent border on Now and a dashed border on Next.
- **Small beat blocks.** They're 20px squares. They should be large blocks filling the header row, with the beat number inside.
- **Pads all look the same.** Every pad is an identical `CARD` button with the key underneath. The mockup uses colour by meaning: Blackout is `DANGER_BG`, Dancer look is purple, Next is `ACCENT_SEL`. It also puts the key badge in the top-right and a sub-state line under each label (e.g. "Off → Mirror X").
- **It still has the 148px sidebar.** Perform should be full-width — hide the sidebar/tabs down to a thin bottom bar, as in the mockup.

### 8. The inspector is a form, not a summary
- The mockup has read-only summary rows that you click to jump to another page: "Dancer: stock_duet · neon" goes to Dancer & FX.
- The build puts ComboBoxes for Look, Effect and Palette in the inspector, which turns it into a second settings page.
- Palette should be a clickable strip that opens a popover of swatches, not a dropdown.

### 9. Missing features from §7 of the brief
- ❌ Queue a specific scene to play next (right-click or shift-click a tile).
- ❌ Favourites plus a ★ filter chip.
- ❌ "Rotation rules" popover. The seasonal/3D/2D controls went to the Show page instead.
- ⚠ Up next is often empty. In random mode `next_scene` is `None`, so the Next card reads "nothing queued". The director must pick the next scene **ahead of time** (at the start of a scene) so it can be shown and changed.

---

## Rules for the next pass

1. **One layout.**
   - Delete `Tab::Show` and the sidebar.
   - Tabs become: `Perform · Dancer & FX · Stream · Timeline · Keys`, in a top bar that also holds the status items.
   - Perform opens by default.
2. **Declare panels in this order:**
   1. `Panel::top` (tabs + status);
   2. `Panel::bottom` if any;
   3. `Panel::right` (inspector);
   4. central content last.
   
   No `- N.0` width fudges. If something overlaps, the order is wrong.
3. **Never hide scrollbars.** Use `ScrollBarVisibility::VisibleWhenNeeded`, and don't pin scroll offsets.
4. **Grids fill their width.**
   - `cols = floor((avail + gap) / min_tile_w)`;
   - `tile_w = (avail - gap*(cols-1)) / cols`;
   - thumbnail height is `tile_w * 9/16`.
5. **Clip all custom painting.** Use `ui.painter_at(rect)` or `painter.with_clip_rect(rect)`.
6. **Measure text, don't estimate it.** Use galley widths, and elide with egui (`truncate()`), never by slicing bytes.
7. **Use only glyphs from the bundled fonts.** No emoji or symbols outside egui's defaults. Add an icon font explicitly if one is needed.
8. **One component per concept.** One `scene_thumb(ui, rect, name, state)` for tiles, previews and the editor library, with the same placeholder everywhere.
9. **The inspector summarises, pages edit.** No ComboBoxes in the inspector except Mode and Bars (segmented). Everything else is a label row that switches tabs when clicked.
10. **The Perform view must be readable from 2m away.**
    - Previews ≥ 35% of the window height.
    - Pads ≥ 72px tall, coloured by meaning, key badge in the top-right, sub-state line.
    - No sidebar.
11. **Settings stay in one place.**
    - Every setting from the old tabs exists exactly once. Tick off the full list in the PR description.
    - Director & sync go in a collapsible section at the bottom of the inspector.
    - Rotation rules go in a popover beside the filter chips.
12. **Director owns "up next".** Pick the next scene when the current scene starts. Expose it in `Status` in all modes except Manual. Queuing a scene overwrites the pick. Follow AGENTS.md: one lock guard per statement, and never hold `timeline` while locking `settings`.
13. **Check it visually before saying it's done** (AGENTS.md §2). Screenshot every page at 900×640 and 720×520 (minimum size) and compare side by side with `1b-library-inspector.png` / `1c-perform.png`. Attach the pairs to the PR.

## Suggested commit sequence
1. `panel: drop Show tab + sidebar, top tab bar with inline status` (rules 1–2)
2. `panel: move director/sync/rotation rules into inspector + popover`
3. `panel: fluid scene grid, visible scrollbar, shared scene_thumb()` (rules 3–8)
4. `panel: inspector summary rows link to pages` (rule 9)
5. `director: pick next scene ahead; queue from tile right-click` (rule 12)
6. `panel: favourites + ★ chip`
7. `panel: perform view full-width, large previews, semantic pads` (rule 10)
8. PR with screenshot pairs + a settings checklist (rules 11, 13)
