# Scope 6: platform modules and crate layout

Files read: all of `src/`, `build.rs`, `Cargo.toml`, `docs/notes.md`, `scripts/sim.ps1`.

## Current layout and dependencies (bin)

- Root of the bin has 21 modules in one flat list (`src/main.rs:6-24`), next to the 4 lib modules.
- `style` is used by 10 modules. `menu` by `details`, `library_view`, `settings`. `title_bar` by `reader`, `shell`.
- `backdrop` is used only by `library_view` and `cover_adjust`. `image_cache` only by `library_view`.
  `cover_adjust` and `details` are opened only by `shell` from `LibraryEvent::AdjustCover` and `LibraryEvent::EditDetails` (`src/shell.rs:38-39`).
- `scan` is used by `main`, `shell`, `settings`, `reader`, `library_view`. `store` by almost all.
- `association`, `instance`, `sim` are Windows-only. They have no gpui dependency, except `sim`.

Proposed tree (P1, P3, P4 together):

```
src/
  lib.rs book/ dither.rs library.rs search.rs     lib, no change
  main.rs shell.rs store.rs scan.rs settings.rs   app wiring and cross-feature state
  reader.rs reader/{pages,search,selection}.rs    no change
  library_view.rs library_view/{backdrop,cover_adjust,details,image_cache}.rs
  ui.rs ui/{menu,style,title_bar}.rs
  platform.rs platform/{association,instance,sim}.rs   cfg(target_os = "windows")
```

### P1: Move the Windows-only modules into `platform/`
- Files: `src/association.rs`, `src/instance.rs`, `src/sim.rs` (whole files), `src/main.rs:6-7,12-13,20-21,47,114,155`, `src/settings.rs:155,225,228`
- Problem: Three Windows-only modules sit in the root list, each with its own `cfg` line in `src/main.rs:6,12,20`. The gate for the Windows integration is in three places.
- Change: Pure move, path edits only.
  - `git mv src/association.rs src/platform/association.rs`, same for `instance.rs` and `sim.rs`.
  - New `src/platform.rs`:
    ```rust
    pub(crate) mod association;
    pub(crate) mod instance;
    #[cfg(all(feature = "sim", target_os = "windows"))]
    pub(crate) mod sim;
    ```
    Keep the full `sim` gate as it is now, so the gate stays the same text in one place.
  - `src/main.rs`: replace the three `mod` lines with `#[cfg(target_os = "windows")] mod platform;`. Change `instance::claim` (line 47) and `instance::Opened` (line 155) to `platform::instance::...`, and `sim::serve` (line 114) to `platform::sim::serve`. The `cfg` attributes on the call sites stay.
  - `src/settings.rs:155,225,228`: `crate::association::` to `crate::platform::association::`.
  - Do not add a `platform/windows/` level. A module named `windows` would shadow the `windows` crate in paths of its parent module, and there is one platform only.
- Parity: No code changes. `pub(crate)` on a child of a private root module has the same reach as the current private `mod` at the crate root. The `sim` gate is the same expression. Pipe names, mutex name, registry keys do not change.
- Performance: no effect.
- Verify: `cargo test`, `cargo build`, `cargo build --features sim`, `cargo clippy`. Run `scripts/sim.ps1` with one `shot:` step to see that the sim pipe answers.
- Size: S
- Depends on: none
- Value: medium. Risk: low.

### P2: One function to make and connect a named pipe
- Files: `src/instance.rs:88-113`, `src/sim.rs:85-107` (after P1: `src/platform/`)
- Problem: `instance::listen` and `sim::listen` have the same 20 lines: `CreateNamedPipeW` with the same mode flags and instance count, the invalid-handle check with the same `bail!`, `ConnectNamedPipe(..).ok()`, and `File::from_raw_handle`. These are three `unsafe` blocks, two times. The only differences are the access (`PIPE_ACCESS_INBOUND` / `PIPE_ACCESS_DUPLEX`) and the out buffer size (0 / 4096).
- Change: In `src/platform.rs`, add
  ```rust
  /// Makes one instance of the pipe and waits for a client.
  pub(super) fn accept(name: &HSTRING, access: FILE_FLAGS_AND_ATTRIBUTES, out_buffer: u32) -> Result<File>
  ```
  with the body of `src/instance.rs:91-110` (the `CreateNamedPipeW` call with `out_buffer` in place of `0` and `access` in place of `PIPE_ACCESS_INBOUND`, the check, the connect, the `from_raw_handle`), and the SAFETY comments as they are. Then:
  - `instance::listen`: `let file = super::accept(&name, PIPE_ACCESS_INBOUND, 0)?;`
  - `sim::listen`: before the loop `let name = HSTRING::from(r"\\.\pipe\book-buddy-dev-sim");`, in the loop `let file = super::accept(&name, PIPE_ACCESS_DUPLEX, 4096)?;`. Remove the `w!` import.
  - Remove the now unused `Pipes`, `FromRawHandle` and `bail` imports where they become unused.
  Check the exact type of the `dwopenmode` parameter of `CreateNamedPipeW` in `windows` 0.62.2 (it is `FILE_FLAGS_AND_ATTRIBUTES` in recent versions).
- Parity: Same Win32 calls with the same arguments, in the same order, on the same threads. The error text of a failed `CreateNamedPipeW` is the same. Pipe names do not change, so `sim.ps1` and a later start still connect. The `unsafe` blocks are not larger: the same three blocks move.
- Performance: no effect. Sim makes one `HSTRING` per thread in place of a static string. Not a hot path.
- Verify: `cargo build`, `cargo build --features sim`, `cargo clippy`. Single instance: start the dev build, then start it again with an `.epub` path; the first window must come to the front and open the book, and the second process must exit. Sim: run `scripts/sim.ps1` with `keys:{ESC}` and one `shot:`.
- Size: S
- Depends on: P1
- Value: medium. Risk: low.

### P3: Move `menu`, `style`, `title_bar` into `ui/`
- Files: `src/menu.rs`, `src/style.rs`, `src/title_bar.rs` (whole files); imports in `src/main.rs`, `src/backdrop.rs:13`, `src/cover_adjust.rs:18`, `src/details.rs:13,15`, `src/library_view.rs:22,26`, `src/menu.rs:14`, `src/reader.rs:35-36`, `src/reader/search.rs:17`, `src/scan.rs:12`, `src/settings.rs:12,15`, `src/shell.rs:174`, `docs/notes.md:6`
- Problem: The shared UI pieces (popup menu and shield, theme and style helpers, title bar) are in the flat root next to feature modules. `rules.md` names this group.
- Change: Pure move, path edits only.
  - `git mv` the three files to `src/ui/`. New `src/ui.rs`: `pub(crate) mod menu; pub(crate) mod style; pub(crate) mod title_bar;`.
  - `src/main.rs`: replace `mod menu; mod style; mod title_bar;` with `mod ui;`. `style::init(cx)` becomes `ui::style::init(cx)`.
  - Every `crate::style` becomes `crate::ui::style`, every `crate::menu` becomes `crate::ui::menu`, every `crate::title_bar` becomes `crate::ui::title_bar`. In `src/menu.rs:14`, `crate::style` becomes `super::style`.
  - `docs/notes.md:6`: `src/menu.rs` to `src/ui/menu.rs`.
  - The `style` tests move with the file.
- Parity: No code changes. Same visibility reach as now. Element and style call order cannot change because no function body changes.
- Performance: no effect.
- Verify: `cargo test` (the `style` tests run from the new path), `cargo build`, `cargo build --features sim`, `cargo clippy`.
- Size: M (about 20 import lines in 12 files)
- Depends on: none (do it after P1 so the two diffs stay small)
- Value: medium. Risk: low.

### P4: Group the library screen modules under `library_view/`
- Files: `src/backdrop.rs`, `src/cover_adjust.rs`, `src/details.rs`, `src/image_cache.rs` (whole files); `src/main.rs:8-11`, `src/library_view.rs:20-21`, `src/cover_adjust.rs:16`, `src/shell.rs:11-12`
- Problem: Four modules that only the library screen uses sit in the root. `backdrop` is used only by `library_view` and `cover_adjust` (`src/library_view.rs:20`, `src/cover_adjust.rs:16`). `image_cache` only by `library_view` (`src/library_view.rs:21`). `cover_adjust` and `details` are dialogs for one library entry, opened from `LibraryEvent` (`src/shell.rs:38-39`).
- Change: Pure move, path edits only. Use the reader pattern (`reader.rs` with `reader/`).
  - `git mv` the four files to `src/library_view/`.
  - At the top of `src/library_view.rs`: `mod backdrop; pub(crate) mod cover_adjust; pub(crate) mod details; mod image_cache;`.
    `backdrop` and `image_cache` become private to `library_view`; this is narrower than now. `cover_adjust` reaches `backdrop` as a child of the same parent.
  - `src/library_view.rs:20-21`: `use backdrop::backdrop;` and `use image_cache::{BoundedImageCache, Filter};` (same style as `use pages::Pages;` in `src/reader.rs:37`).
  - `src/cover_adjust.rs:16`: `use super::backdrop::backdrop;`.
  - `src/shell.rs:11-12`: `crate::library_view::cover_adjust::...`, `crate::library_view::details::...`.
  - `src/main.rs`: remove `mod backdrop; mod cover_adjust; mod details; mod image_cache;`.
  - The `backdrop` tests move with the file.
  - The group cannot be named `library`: `mod library;` in `main.rs` resolves to `src/library.rs`, which is the lib module. The same is true for `search`. Keep `library_view` as the name, so no module is renamed.
  - `scan` stays in the root: `main`, `shell`, `settings` and `reader` use it too.
- Parity: No code changes. Visibility is the same or narrower.
- Performance: no effect.
- Verify: `cargo test`, `cargo build`, `cargo build --features sim`, `cargo clippy`.
- Size: S
- Depends on: P3 if done after it (the `style` and `menu` import paths in the moved files change in P3). Either order works; do one at a time.
- Value: medium. Risk: low.

### P5: `association`: remove the unused `name` parameter of `get`, share the open command
- Files: `src/association.rs:21-26,46-53,78-116`
- Problem:
  - `get` has a `name: Option<&str>` parameter, and both callers give `None` (`src/association.rs:32,51`). The parameter, the `HSTRING` for it and the `map_or` (lines 80-83) are dead.
  - `register` and `registered` both build the same command string `format!("\"{}\" \"%1\"", exe.display())` from `current_exe` (lines 22-23 and 46-50) and the same key `format!(r"{CLASSES}\{PROG_ID}\shell\open\command")` (lines 25, 51). Two copies. If one changes alone, `registered` is false after every `register`.
- Change:
  - `fn get(key: &str) -> Option<String>`; pass `PCWSTR::null()` as the value name in both `RegGetValueW` calls. Update the two callers.
  - Add `fn command_key() -> String` and `fn command() -> std::io::Result<String>` (the `current_exe` and the `format!`). `register` uses `command()?`. `registered` uses `let Ok(command) = command() else { return false; };`.
  - `set` keeps its `name` parameter: one caller gives `Some(PROG_ID)`.
- Parity: The same registry calls with the same values. A `None` name gave `PCWSTR::null()` before too. `register` still returns the `current_exe` error through `anyhow`.
- Performance: no effect.
- Verify: `cargo build`, `cargo clippy`. In the settings panel the `EPUB FILES` row shows `REGISTERED` after a click on `REGISTER`, and shows it again after a restart.
- Size: S
- Depends on: P1 if done after it (file path only).
- Value: low. Risk: low.

### P6: `sim`: send all events through the dispatch helpers
- Files: `src/sim.rs:272-290,320-339,341-372,411-413,417-421`
- Problem:
  - `Sim::key` ends with a copy of `dispatch` (lines 411-413 against 417-421).
  - `move_to`, `button` and the `scroll` step each have the same 8 lines: `handle.update`, `to_logical(window, at)?`, `window.dispatch_event(..)`, `Ok(())`, `})?` (lines 277-288, 328-338, 351-371). Three copies.
- Change:
  - In `Sim::key`, replace the last `handle.update` with `dispatch(handle, cx, event)`.
  - Add `fn dispatch_at(handle: AnyWindowHandle, cx: &mut AsyncApp, at: (i32, i32), event: impl FnOnce(Point<Pixels>) -> PlatformInput) -> Result<()>`. It does the `handle.update`, calls `to_logical` and then `event(position)` inside it, and dispatches. Use it in `move_to`, `button` and `scroll`.
- Parity: Each event is still one `handle.update` with one `dispatch_event`, in the same order. `to_logical` still runs inside the update, before the event is built. The answers to `sim.ps1` are the same. Sim-only code.
- Performance: no effect.
- Verify: `cargo test --features sim` (`send_keys_spelling`), `cargo build --features sim`, `cargo clippy --features sim`. Run `scripts/sim.ps1` with `click:`, `move:`, `scroll:`, `kdown:`/`kup:` and `shot:` steps before and after; the shots must be equal.
- Size: S
- Depends on: P1 if done after it (file path only).
- Value: low. Risk: low.

## Suspected bugs

1. `src/instance.rs:37-41`: when the mutex exists but `send` fails (the running app does not take the pipe in 3 s), the second start continues as a first start. It opens a second window, and its `listen` fails because the pipe has a maximum of 1 instance (logged only). Then two processes hold the same `Store` and both write `library.json`; the last save wins.
2. `src/instance.rs:78`: the path is sent with `to_string_lossy`. A Windows path that is not valid UTF-16 (an unpaired surrogate) arrives changed, and the running app cannot open it. The first start opens the same path correctly through `args_os`.
3. `src/instance.rs:32,62`: the mutex is `Local\` (one per session), but the pipe name is one per machine. When the same user is logged on in two sessions (console and remote desktop), a start in the second session finds no mutex, opens a window, and its pipe fails. A later start in the second session then sends its book to the app in the first session.

## Out of scope notes

- `src/main.rs:153-177` (`open_from_later_starts`) is single-instance glue. It stays in `main.rs` because it needs `Shell` and `Root`, and `instance.rs` has no gpui dependency now.

## Kept as is

- `build.rs`: correct, and `rules.md` forbids changes.
- No bin module moves into the lib: `association` and `instance` are Windows-only, have no tests, and would need `pub`. All other bin modules use gpui (`store` for `Global`, `sim::send_keys` for `Keystroke`).
- No `platform/windows/` level: a `windows` module shadows the `windows` crate name, and there is one platform.
- `scan`, `store`, `settings`, `shell` stay in the root: more than one feature uses them.
- The `CreateMutexW` handle is never closed: the mutex must live until the process exits (comment at `src/instance.rs:33`).
- The sim pipe name is fixed and has no user name: `scripts/sim.ps1:21` uses it, and the feature is for the dev build.
- `kdown` returns before `SETTLE` (`src/sim.rs:179-186`): a full settle makes a tap into a hold (comment).
- The `keys:` value is not trimmed (`src/sim.rs:72`): `keys: ` types a space.
- `registered` compares with `eq_ignore_ascii_case`: Windows paths are not case-sensitive.
- `register` changes the `.epub` default only when it is empty or already ours (`src/association.rs:33`): it must not take the default from another app.
- `name()` has the user name and a dev suffix (`src/instance.rs:51-60`): pipe names are per machine, and the dev build must not send books to the release build.
