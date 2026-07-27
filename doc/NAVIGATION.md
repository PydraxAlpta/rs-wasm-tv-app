# Navigation & the leanback focus model

This is the subtle part of the codebase. Read it alongside `src/metrics.rs`,
`src/anim.rs`, `src/ui/components/carousel.rs`, `src/ui/components/rail_list.rs`,
`src/ui/components/banner.rs`, `src/ui/pages/catalog.rs`, and `src/ui/pages/shell.rs`.

## Fixed focus anchor, moving content

Leanback UIs don't move a cursor over static content — they pin the focus to one screen
position and slide the content behind it. The focus ring is always drawn at the content's
left edge (`metrics.focus_x`) at a fixed row Y; everything else is positioned relative to
the currently-focused indices.

In `RailList::render`:

```
card_x = focus_x + (col  - anim_col ) * card_step   // card_step = card_w + card_gap
row_y  = focus_y + (rail - anim_rail) * rail_step
```

`anim_col` and `anim_rail` are **fractional** animated indices. When they equal the
integer focused index, the focused card sits exactly on the anchor. While a tween is
mid-flight the whole grid is offset by the fractional part, which is what produces the
smooth slide.

## Tween: frame-rate-independent smoothing (`src/anim.rs`)

Every animated quantity is a `Tween` — a scalar easing toward a target with time-constant
`tau`. Per step it covers `1 - e^(-dt/tau)` of the remaining distance, so behaviour is
identical at 30/60/120 fps. `set_target` aims it, `step(dt)` advances it, `snap` jumps
instantly, `is_settled` reports "close enough". Smaller `tau` = snappier.

Key `tau` values (`carousel.rs` unless noted): `NAV_TAU = 0.11` (horizontal cards, nav
underline, banner), `RAIL_TAU = 0.2` (vertical rails — a longer ease), plus per-widget
constants like the shell `SLIDE_TAU = 0.18`, overlay `SLIDE_TAU = 0.16`, player
`FADE_TAU = 0.2`.

## The two-index rail model (`RailList`)

`RailList` owns:

- `focus_rail: usize` — the integer focused rail.
- `focus_col: Vec<usize>` — a **remembered column per rail**. Moving up/down restores the
  column you last had on that rail, Netflix-style.
- `rail_carousel: HCarousel` — the horizontal position within the focused rail.
- `anim_rail: Tween` — the fractional vertical position.

When you switch rails (`move_to_rail`), the current column is saved into `focus_col`, and
the new rail's remembered column is **snapped** (not eased) into the horizontal carousel —
so changing rows never produces a sideways slide, only a vertical one.

Rails are revealed lazily in batches of `RAIL_BATCH = 5` (`maybe_load_more`): as focus
comes within two rows of the loaded edge, the next batch becomes navigable/visible. This
keeps the initial working set small.

## `HCarousel`: clamped vs wrapping (`carousel.rs`)

`HCarousel` is the reusable horizontal engine. It holds an integer `index` and an animated
`Tween`, and comes in two flavours:

- **Clamped** (`wrap = false`) — card rails. `index` and target are clamped to `[0, len-1]`.
- **Wrapping** (`wrap = true`) — the hero banner. Targets are kept **unbounded** so rapid
  wraps never reverse mid-flight (going 4 → 0 animates forward to logical index 5, then
  `normalize` folds the settled value back into range). This is why `fast_wrap_keeps_direction`
  is a test.

## Hold-to-scroll (the app owns key repeat)

OS key auto-repeat is deliberately dropped for arrows in `install_keydown`
(`event.repeat()` → early return). All continuous scrolling is driven by the app from the
rAF loop, so it stays smooth and frame-rate-independent. The mechanism has three parts,
shared by cards, rails, and the banner:

1. **Tap vs hold.** On keydown the widget takes a single step and records `held` + resets
   `held_secs`. Each `update` accrues `held_secs`. If the key is released before
   `HOLD_SCROLL_DELAY = 0.2 s`, it stays a single-step tap.

2. **Hold advance (`hold_advance` / `hold_advance_rail`).** Once past the delay, every
   frame tops up the tween target so at least `HOLD_AHEAD = 1.0` index-units of runway sit
   ahead of the current animated value. The exponential tween therefore never settles
   between cards — motion is continuous while the key is down.

3. **Release ease (`release_ease` / `release_ease_rail`, from `handle_key_up`).** On
   release the target is committed **forward** in the travel direction (`ceil` when going
   right/down, `floor` left/up) — never backward. If the chosen stop is closer than
   `RELEASE_MIN_RUN = 0.4`, it coasts one more step so the ease-out has room to feel
   smooth. Clamped carousels clamp the result; wrapping ones fold it.

The invariants above are pinned by tests: `hold_advance_keeps_runway_ahead`,
`release_ease_drops_far_target`, `release_ease_coasts_when_too_close_to_stop`,
`release_ease_never_goes_backward` (and their `rail_list` vertical equivalents).

## Zones and cross-zone chaining (`CatalogPage`)

A catalog page has two focus zones — `Banner` (top) and `Rails` (below) — routed by a
`FocusScope`. Within the rails, `RailList` handles Left/Right (cards) and Up/Down (rails).
The interesting behaviour is at the **edges**, where a held direction should *chain* from
one zone into the next instead of stopping:

- **Up** from rail 0 → the banner; a continued hold from the banner then leaves the page
  upward (`pending_move_out = Some(Up)`), which the shell reads to move focus to the nav.
- **Down** from the banner → the rails; a continued hold keeps descending into deeper rails.

`CatalogPage::hold_traverse` runs each frame while a vertical key is held. It waits out a
`BOUNDARY_DWELL` (= `NAV_TAU`) at each crossing so a fast hold pauses momentarily on each
zone rather than flying straight through, and it only crosses once the current tween is
`vertical_near_settle()` (within `CHAIN_THRESHOLD = 0.4`). When it hands off into the
rails mid-hold it calls `RailList::arm_continuous_hold` so the rail skips its own tap
delay and continues seamlessly. If a page has no banner, Up from rail 0 goes straight to
the nav.

`handle_key_up` on the page clears the held state and forwards the release into the focused
child so its `release_ease` runs.

## The shell: tabs and page sliding (`MainShell`)

`MainShell` sits above the catalog pages. It holds:

- `nav: NavBar` and a `focus: ShellFocus { Nav, Content }`.
- `pages: [Option<CatalogPage>; TAB_COUNT]` — Home eager, others lazily built on first visit.
- `slide: Tween` — the fractional tab index. Each page is laid out at `x = (i - slide) * width`,
  so changing tab animates the whole strip sideways. Pages more than ~1.2 tabs off-strip
  skip their per-frame update work, and each visible strip is `set_clip`-ped to its rect so
  banners/rails never bleed into the neighbour or off-screen mid-slide.

Focus flow: Down from the nav enters content (and begins a hold-traverse so a held Down
continues into the rails); Up from the top content zone (banner, or rail 0 with no banner)
returns to the nav. Left/Right on the nav switches tab, lazily creating the target page and
retargeting `slide`. An open `MetadataOverlay` intercepts keys first (so Back closes it
without popping the shell); otherwise Back on the root pops the stack → app exit.

## Key mapping (`src/wasm/mod.rs`)

The logical `Key` enum (`Up/Down/Left/Right/Enter/Back`) is decoupled from physical keys.
`map_key` translates browser `KeyboardEvent`s:

| Physical | Logical |
| --- | --- |
| Arrow keys | Up / Down / Left / Right |
| Enter, Space | Enter |
| Backspace, Escape, GoBack, BrowserBack | Back |
| Tizen remote Back (keyCode 10009 / 461) | Back |

Both keydown and keyup are wired; keyup is what drives the release-ease. On the root
screen, Back → `Pop` → empty stack → `exit_app()` (Tizen app exit via `js_sys::Reflect`
walk, falling back to `window.close()`).
