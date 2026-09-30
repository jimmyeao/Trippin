# UI redesign — review round 2 (from screenshots)

Big improvement: one layout, real thumbnails, the tabs and header are right, and the timeline editor is close.
What still feels "off" comes down to a few systemic bugs plus proportions. Fix them in this order.

---

## A. Systemic bugs (fix these first; they affect every page)

### A1. Content overflows the right edge
Dancer & FX, Stream and the Routines grid are all clipped on the right:
- the FX card;
- "60 fps";
- the last tile column.

Meanwhile the left edge has a 4px sliver with no margin.

- **Cause:** a classic egui bug. `card().show(ui, |ui| ui.set_width(ui.available_width()))` makes the card's content full width, then the card adds `inner_margin` + stroke on top, so every card is ~26px too wide. `ui.columns` inside a ScrollArea makes it worse.
- **Fix:**
  - Give the central panel one outer margin: `Frame::central_panel(..).inner_margin(12)`.
  - Inside a card, use `ui.set_width(ui.available_width())` only *after* the frame's margin is accounted for. Or avoid it: use `Frame::show` + `ui.set_min_width(w - 2*margin - 2*stroke)`.
  - Build two-column pages with `ui.columns(2, …)` **outside** the ScrollArea, or with `egui_extras::StripBuilder`.
- **Rule:** no page may be wider than the window. Test at 720px wide; nothing may clip.

### A2. Tofu glyphs (□)
The ☐ shows up in three places:
- the Canon pad key badge (Space);
- every routine checkbox;
- the timeline editor's record button.

These are glyphs missing from the font.
- Space → the text `Space`.
- Checkboxes → paint them: a filled rounded rect plus a two-segment tick path (`painter.line(...)`). Don't use a font glyph.
- Record → a filled red circle (`circle_filled`, `#F06A62`).
- **Rule:** grep the diff for non-ASCII characters in UI strings. Each one must be confirmed to render in the bundled fonts, or be painted.

### A3. The accent colour drifts
Routines use **pink** for the selected outline and the checkboxes. Pad sub-lines are teal, purple, pink and orange.
- There is one accent: `ACCENT #3FB0D8`. Lane colours are only for the timeline editor.
- Selected / in rotation → ACCENT everywhere.

### A4. Alignment uses ad-hoc horizontals, not grids
- **Keys:** action labels are **centred**, and key badges and Rebind buttons wander, because key widths differ.
- **Stream:** label columns differ card to card.
- **Fix:** every label/control form uses `egui::Grid::new(id).num_columns(2).min_col_width(96.0)` with left-aligned labels.
- Keys page: 3 fixed columns, `[action ─ flex | key badge 90px | rebind 70px]`.

---

## B. Perform (Pads)

The main problems are proportions and duplication.

1. **Pads are ~260px tall with the label stuck at the bottom.** That's too much dead space, and the eye has to travel.
   - Fixed pad height **88px**.
   - Label + sub-line vertically centred, left-aligned. Key badge top-right.
   - Leftover height goes to the previews, not the pads.
2. **The previews are too small and awkwardly framed.**
   - "NOW" / "UP NEXT" sit in a left gutter inside the card. The UP NEXT thumbnail doesn't fill its card.
   - Make the thumbnail fill the card at 16:9.
   - Overlay the label top-left and the name bottom-left, as in the mockup.
   - Target: previews ≈ 40% of the content height.
3. **BPM appears twice.** The header shows `BPM 140.0 ●●●●` and the pads view shows `140 BPM ■■■■`.
   - Keep the big one in Pads mode and hide the header's BPM/pips while Pads is active. Or the reverse. Not both.
4. **Beat blocks stop at 4×36px, leaving a void.** Stretch them across the row width, as in 1c. Put the breakdown / no-signal pills and "cut in N bars" at the right end of that row.
5. **Too many pad colours.** Only three pads get colour:
   - Next = `ACCENT_SEL`;
   - Blackout = `DANGER_BG`;
   - Hold, *when active*, = accent outline.
   
   Everything else is `CARD`. "Mark the one" should not be brown.
6. **The palette column is 18 bordered cards.** That's visual noise and it pushes the list off the screen.
   - Make it a plain list: name (11px, muted) above a 14px strip, no card borders, 6px gap.
   - Selected = name in TEXT + 2px accent outline on the strip.
   - Width 150px.
7. **The Library | Pads toggle looks like two more tabs.**
   - Move it to the far right of the tab bar, left of fps, as a small segmented control (`INSET` track, 11px).
   - Or make it an icon toggle.

## C. Dancer & FX

1. **Unequal cards.** FX is half the Dancer card's height, and its options wrap ("Kaleido x8" drops to a new line).
   - Make both cards the same height, or stack FX under Dancer in a left column with Routines on the right.
   - FX options: a 3×2 segmented grid (`Grid` of equal cells).
2. **The routine grid shows 44 tiles; half of them are `_mir` duplicates.**
   - Show 22 tiles, one per routine.
   - Each tile gets a small "mirror" badge that toggles whether the `_mir` twin is in rotation (both ticked by default).
3. **Tiles:**
   - Thumbnails are black boxes on a CARD background. Draw the silhouette **white on `INSET`**, with no separate black box.
   - Name: elide from the middle so the `_mir` suffix isn't what gets lost.
4. Help text sits in three different places. One muted help line under each card title, max.

## D. Stream

1. **"OBS: add a Text source reading C:\…"** is rendered as *justified* text with huge letter-spacing. Use a normal left-aligned label, and a monospace path with a **Copy** button.
2. **Per-source status is a wall of text lines.**
   - Make it a compact 2-column grid: source | status.
   - Each row has a status dot: green = has a track, grey = idle / not found.
   - Collapse it under "Sources ▸" by default. The current track stays visible at the top.
3. Branding/Ticker/Video output: fine once A1 and A4 are fixed. The "Corner" arrow glyphs need confirming against A2.
4. Put 12px between cards, both axes. Currently it's uneven.

## E. Timeline tab

It's 80% empty, and "open editor" sits far from everything else.
- One card:
  - a primary **Open timeline editor** button (ACCENT_SEL, full width, 36px);
  - under it, the transport row (play/stop, timecode, status);
  - then a **list of saved timelines** (name, song, length, last edited) — click to load, double-click to open in the editor.
- Drop the separate "FILE" card.

## F. Keys

- Grid per A4: left-aligned labels, fixed key-badge and rebind columns.
- Style the filter field like the library search (`INSET`, border, 6px radius, placeholder "Filter actions…") and align it with the card's left edge.
- **Group actions** under small section labels: Scenes / Mode / Dancer / Sync / Output / Timeline / Recording. This reads far better than 25 flat rows.
- "Reset all keys to defaults": in the page footer, right-aligned, danger text style.

## G. Timeline editor (close; polish only)

1. **Record button** → red dot (A2). While armed, fill it `DANGER_BG` and pulse the dot.
2. **Toolbar is two rows with mismatched sizes.** Open / Add song / Save are bigger than the transport.
   - Use one 40px row: transport + timecode │ snap, follow │ flex │ name field, Save, AI show │ zoom.
   - Move Open… / Add song… into a "File ▾" menu button.
3. **Bar numbers (4, 5, 6) collide with the song title** in the audio lane. Draw bar numbers on the ruler only.
4. **Inspector empty state:** keep the hints, but drop the box and use muted 11px text. "+ marker at cursor" belongs in the toolbar.
5. Lane gutter: the labels sit ~100px left of the lanes with dead space between. Use a 110px gutter with the label and dot right next to the lane edge, as in the mockup.

---

## Rules to add to AGENTS.md (UI section)

1. **One outer margin (12px) on every page.** Nothing touches or overflows the window edge. Verify at 720×520.
2. **Card widths account for margin + stroke.** Never `set_width(available_width())` inside a padded frame.
3. **Forms are `egui::Grid`s.** Labels left-aligned in a fixed-width column.
4. **One accent (#3FB0D8)** for selection, live and in-rotation. Semantic colours only for danger, warn and breakdown.
5. **No font glyphs for UI icons.** Paint ticks, dots and arrows, or use plain words.
6. **Fixed control heights:** buttons 28px, pads 88px, inputs 26px. Leftover space goes to content (previews, grids), never to controls.
7. **No duplicated readouts** on one screen (BPM, beat, scene name).
8. **Help text:** at most one muted line per card, 11px, left-aligned, never justified.
9. **Screenshot-pair check before done:** every page at 900×640 and 720×520 next to its mockup PNG. Attach the pairs to the PR.
