# Scope 5: shell and UI parts

Files: `src/shell.rs`, `src/main.rs`, `src/menu.rs`, `src/style.rs`, `src/title_bar.rs`, `src/settings.rs`.

The code in this scope is in good condition. The proposals are small. All are size S.

### P1: Remove the `enabled` flag from `choice` options
- Files: `src/settings.rs:296-315`, call sites `src/settings.rs:484-575`
- Problem: `choice` takes `options: &[(T, &'static str, bool)]` (`settings.rs:300`). All six call sites pass `true` for every option (`settings.rs:489, 535-536, 548, 559, 570`). `git log -p` shows that no option was ever `false` since the panel was added (956ed18). So the flag and the `enabled_button` call in `choice` (`settings.rs:312`) are dead branches.
- Change: Change the type to `&[(T, &'static str)]`. In the closure, destructure `&(value, label)` and replace `enabled_button(button, enabled, move |cx| change(cx, |s| set(s, value)))` with `button.on_click(move |_, _, cx| change(cx, |s| set(s, value)))`. Remove the `, true` from each tuple at the call sites. Keep `enabled_button`. The font size buttons (`settings.rs:378, 388`) and RE-DITHER (`settings.rs:520`) still use it.
- Parity: With `enabled == true`, `enabled_button` does exactly `button.on_click(move |_, _, cx| on_click(cx))` (`settings.rs:323-324`). The element tree, the ids `(id, ix)`, the `ml(px(-1.))` overlap and the style calls do not change. No persisted data is involved.
- Performance: No effect. A bool less per option in a static slice.
- Verify: `cargo build`, `cargo build --features sim`. Sim: open settings, click each choice button (THEME, MODE, BARS, IMAGES, SHOW HIDDEN). Compare screenshots before and after.
- Size: S
- Depends on: none
- Value: medium. Risk: low.

### P2: Keep the settings panel and its subscription in one `Option`
- Files: `src/shell.rs:26-27, 46-47, 110-134, 197`
- Problem: `Shell` keeps `settings: Option<Entity<SettingsPanel>>` and `_settings_subscription: Option<Subscription>` as two fields (`shell.rs:26-27`). They must be `Some` and `None` together. `open_settings` (`shell.rs:111-119`) and `close_settings` (`shell.rs:127-128`) set them in pairs by hand. The two other overlays already use one `Option<(Entity<_>, Subscription)>` (`shell.rs:24-25`).
- Change: Replace both fields with `settings: Option<(Entity<SettingsPanel>, Subscription)>`. In `open_settings`, in the `is_none()` branch, make the panel and the subscription, then `self.settings = Some((panel, subscription))`. The focus line becomes `if let Some((panel, _)) = &self.settings`. In `close_settings`, remove the `_settings_subscription = None` line. In `render`, replace `.children(self.settings.clone())` with `.children(self.settings.as_ref().map(|(panel, _)| panel.clone()))`, as the `details` line above it does. Remove the field from `Shell::new`.
- Parity: The panel entity drops before the subscription, as now (tuple fields drop in order, and now `settings = None` runs before `_settings_subscription = None`). The child order in `render` does not change. Focus calls and `cx.notify()` do not change.
- Performance: No effect.
- Verify: `cargo build`, `cargo build --features sim`. Sim: ctrl-, opens settings, Escape closes it and the focus goes back (library, then once more with a book open: a key such as `right` must turn the page after close).
- Size: S
- Depends on: none
- Value: medium. Risk: low.

### P3: Remove the theme install from `style::init`
- Files: `src/style.rs:18-30`
- Problem: `style::init` installs the monochrome theme with `Tint::DRAWN` in the global mode (`style.rs:28-29`). `main.rs:57-60` then runs `settings::init`, which calls `apply_theme` (`settings.rs:34`). `AppliedTheme` starts as `None` (`settings.rs:71`), so `apply_theme` always calls `style::set_theme` again with the saved mode and tint. The first install is overwritten at once. Each install parses and deserializes the bundled JSON and runs `Theme::change`. Before 2eded73 this was not redundant, because the old `apply_theme` only called `Theme::change` and did not install the theme set.
- Change: In `style::init`, remove the two lines `let mode = Theme::global(cx).mode;` and `set_theme(mode, Tint::DRAWN, cx);`. The function then only adds the fonts. Remove imports that become unused, if any (`Theme` is still used in `set_theme`).
- Parity: Checked in gpui-component 0.6.1. `Theme::apply_config` (`theme/schema.rs:1064`) sets all colors from the config plus defaults, and sets `font_family`, `font_size`, `mono_font_family`, `radius`, `radius_lg` and `shadow` only if the config has them. The light and the dark config in `assets/themes/monochrome.json` have the same keys (`font.family`, `font.size`, `mono_font.family`, `radius`, `radius.lg`, `shadow`, 40 colors each) and no `highlight`. So nothing from the first install survives the second one. Nothing between `style::init` and `settings::init` reads the theme (`reader::init` only binds keys and adds fonts, `cx.set_global(store)` does not read it). No window is open yet. `apply_theme` also calls `redither` at startup in both cases, because `applied` is `None`.
- Performance: Better. Startup does one JSON parse, one `ThemeSet` deserialize and one `Theme::change` less.
- Verify: `cargo build`, `cargo test`, `cargo build --features sim`. Sim: start with `theme: Light`, then with `theme: Dark` and a non-default hue and saturation in a test `library.json`. Screenshots of the library and the settings panel must be equal before and after.
- Size: S
- Depends on: none
- Value: low. Risk: low.

### P4: One helper to save a tint
- Files: `src/settings.rs:172-175, 180-183`
- Problem: Two copies write a `Tint` into the settings field by field: `slid` (`settings.rs:172-175`) and `drawn_tint` (`settings.rs:180-183`). The inverse, `tint(&Settings)`, is already one function (`settings.rs:82-87`).
- Change: Add next to `tint`:
  ```rust
  fn save_tint(tint: Tint, cx: &mut App) {
      change(cx, |s| {
          s.hue = tint.hue;
          s.saturation = tint.saturation;
      });
  }
  ```
  Use `save_tint(shown, cx)` in `slid` and `save_tint(Tint::DRAWN, cx)` in `drawn_tint`.
- Parity: The same two field writes in the same order, inside the same `change` call. The slider updates in `drawn_tint` still run after the save.
- Performance: No effect.
- Verify: `cargo build`. Sim: move the HUE slider and release, then click DEFAULT. Compare screenshots and the saved `library.json`.
- Size: S
- Depends on: none
- Value: low. Risk: low.

### P5: One constant for the opacity of a disabled control
- Files: `src/menu.rs:158`, `src/settings.rs:326`, `src/style.rs`
- Problem: A disabled menu item (`menu.rs:158`) and a disabled settings button (`settings.rs:326`) both use the literal `0.45`. They show the same state and must stay equal.
- Change: Add `pub const DISABLED: f32 = 0.45;` to `style.rs` with a one-line doc comment ("The opacity of a control that does nothing."). Use `style::DISABLED` at both sites. Do not touch `library_view.rs:493` (`0.42`, a different meaning: missing or hidden cards).
- Parity: The same value, so the same pixels.
- Performance: No effect.
- Verify: `cargo build`.
- Size: S
- Depends on: none
- Value: low. Risk: low.

## Suspected bugs

1. A book that a later start opens does not close the overlays. `open_from_later_starts` (`main.rs:152-177`) calls `Shell::open` (`shell.rs:62`), which closes only the reader. If settings or the details dialog is open, it stays on top of the reader, but the focus moves to the reader. Escape then closes the book (`reader.rs:792`), not the overlay. If cover adjust is open, it stays in `self.cover_adjust`, hidden behind the reader (`shell.rs:168-171`). When the book closes, cover adjust shows again, but `close` focuses the library (`shell.rs:145`), so Escape does not close cover adjust until the user clicks in it.
2. Uncertain. A slider preview can stay installed without a save. `slid` installs the preview tint on `SliderEvent::Change` and saves only on `Release` (`settings.rs:161-177`). If the panel closes during a drag (Escape pressed while the mouse button is down), the slider is no longer painted and `Release` may not arrive. The preview tint then shows until the next settings change, which reinstalls the saved tint through `apply_theme`.

## Out of scope notes

- Window state: `save_window_state` is in `shell.rs:203-221` and `restore_window` is in `main.rs:123-150`. They are the two halves of one concept. Scope 6 may want to put them together when it plans the module layout.
- `settings.rs` holds two concepts: the theme application (`AppliedTheme`, `apply_theme`, `preview_tint`, `tint`, `settings.rs:58-97`) and the panel view. Scope 6 may move the theme part next to `style::set_theme` in a `ui/` module. I do not propose this, because it is a move between modules.

## Kept as is

- `open_cover_adjust` and `open_details` (`shell.rs:77-108`) look alike, but the constructors, event types and fields differ. A generic helper would be longer than the two copies.
- `apply_theme` and `preview_tint` both call `set_theme` and then set `AppliedTheme`. It is two lines, and the steps after them differ (redither on a mode change against `refresh_windows`).
- `title_bar::height` uses `cfg!` at run time (`title_bar.rs:10`), so `reader.rs` can call it on all targets.
- `let _ = shell;` and `let _ = window;` under `cfg` (`main.rs:104-119`) silence unused values on builds without Windows or sim.
- `back_button` is a `canvas` with a window-level mouse listener (`shell.rs:223-244`). The comment explains it: overlays that occlude must not block the mouse back button.
- `cx.observe(&reader, ...)` (`shell.rs:70`) is needed to swap the library for the reader when loading ends (`docs/notes.md`, "Opening a book").
- `style::set_theme` parses the bundled JSON on each call, also on each slider `Change`. A cached `serde_json::Value` still needs a full clone per call, and `refresh_windows` costs more. Not worth a change.
- `SettingsPanel::render` clones `settings` and `folders` (`settings.rs:370-371`). The borrow of `cx` needs it, and each remove button needs its own `PathBuf`. The panel is not a hot path.
- `slider.set_value` in `drawn_tint` does not emit `SliderEvent` (checked in gpui-base 0.6.1 `slider.rs:261`), so DEFAULT does not start a preview.
- The remove-folder button (`settings.rs:429-434`) saves without a rescan. Books from the folder stay in the library. This looks deliberate (`library.rs:655`, `remove_hides_books_in_watched_folders`).
- `menu.rs` is coherent and small. `shield` clones the handler for three buttons, which is the cheapest form with gpui's per-button `on_mouse_down`.
