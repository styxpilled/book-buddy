# Refactor proposal rules

You review part of the book-buddy codebase and propose refactors. You do not
apply them. Read `CLAUDE.md` and `docs/notes.md` before you start.

## Goal

Improve codebase health. Keep exact feature, visual and behavior parity.
Performance must not get worse. Improvements to performance are welcome.

## Work mode

- Read only. Do not edit files, except your report file. Do not run the app.
  Do not run builds. The user may be running the app.
- Use `git log -p` and `git blame` on code that looks odd. Code that looks odd
  is probably deliberate. If a comment, a commit message or `docs/notes.md`
  explains it, keep it.
- Stay in your assigned files. If you find a problem outside them, put it under
  "Out of scope notes". Do not write a proposal for it.

## What makes a proposal valid

A proposal must have a concrete benefit. Examples:
- It removes duplicated logic. Two or more copies, not one copy that may repeat later.
- It splits a file or a type that has more than one clear responsibility.
- It moves code to the module that owns the concept.
- It removes dead code, unused parameters, or state that is never read.
- It replaces a manual pattern with an existing helper in the repo or in a
  dependency that is already in the build.
- It removes clones, allocations or repeated work, mainly in hot paths.
- It makes an invariant hold by its types, and removes a runtime check or a comment.

Do not propose:
- Renames, reordering or formatting changes for taste only.
- An abstraction, trait or generic that has only one user.
- Splitting a file that is coherent only because it is long.
- New dependencies, or version changes in `Cargo.toml`.
- Changes to `build.rs`, `assets/`, the dev-profile `opt-level` overrides, or
  the dependency pins and the comments that explain them.
- Changes to "fix" behavior. If you think something is a bug, report it under
  "Suspected bugs". A refactor must not fix it silently.
- Using gpui-component menus or popovers. `docs/notes.md` forbids them.

If the code in your scope is reasonable, say so and propose nothing.
Zero proposals is a valid result.

## Parity: what must not change

- Pixels. Layout, spacing, colors, fonts, shadows, dither output, cover
  rendering. Also the order of gpui elements and style calls when the
  order has an effect.
- Float math order in dither, color, cover-adjust and layout code. A
  different order can change a pixel.
- Input. Key bindings, action names, mouse behavior, cursors, focus, scroll,
  selection.
- Timing and sequence. The order of async tasks, which executor runs the
  work (background or foreground), debounce and wait values (for example
  `IMAGE_WAIT`), and when the view changes. Never add a loading screen. Delay
  the view change instead.
- Persisted data. Serde field names, `rename`, `default`, `skip` and enum
  representation in `library.json`, settings and themes. Existing user files
  must load the same way. Keep the data directory location.
- Sort orders, search results, pagination and page breaks.
- Window state, the single-instance behavior and file association.
- Error behavior that the user can see. Log text can change.

## Performance

- Hot paths: render functions, page layout, search, selection, image decode,
  dither, library grid, scan. Do not add allocations, clones, locks or extra
  passes in them.
- Do not move work from a background executor to the main thread.
- Keep `fast_image_resize` for resizing. See `docs/notes.md`.
- If a proposal changes performance, say how. Name the path and the cost
  that goes away or that is added.

## Modules

Grouping the Rust source into modules is welcome. Rules:
- Group by feature, not by layer. Use for example `reader/`, `library/`,
  `platform/windows/`, `ui/` (popups, style, title bar).
- Keep the lib/bin boundary. `lib.rs` holds logic that has no UI and has
  tests (`book`, `dither`, `library`, `search`). Code moves into the lib only
  if it has no gpui dependency.
- Windows-only code stays behind `cfg(target_os = "windows")`. The `sim`
  module stays behind its current gate.
- A module move is a pure move: no edits in the same step. Give the move as a
  separate proposal before the edits that depend on it, so that the diff is
  easy to review.
- Keep visibility as narrow as it is now. Use `pub(crate)` or `pub(super)`,
  not `pub`, unless the lib boundary needs `pub`.
- Tests move with the code they test.
- Only the agent for scope 6 proposes moves between modules. The other agents
  propose splits inside their own files only.

## Code style

- Write as the surrounding code does: the same naming, idioms and comment density.
- Comments: minimal, ASD-STE100 Simplified Technical English, no em-dashes.
- Do not add new clippy warnings. Do not add `unsafe`. Do not make an
  existing `unsafe` block larger.

## Tests

- Existing tests must keep their meaning. A test can move. Its assertions
  must not get weaker.
- If a refactor touches logic with no tests, and the logic is pure (no UI),
  propose a characterization test first. It records the current behavior.
- For a change that can affect pixels or input, give a check with
  `scripts/sim.ps1`: steps and screenshots before and after that must be equal.

## Output format

For each proposal:

### P<n>: <short title>
- Files: paths and line ranges
- Problem: what is wrong now, with code references (`file:line`)
- Change: what to do, specific enough for a different agent to do it
- Parity: why the behavior, the pixels and the persisted data stay the same.
  Name the risks you checked.
- Performance: no effect / better (how) / risk (where)
- Verify: `cargo test`, `cargo build --features sim`, sim steps if needed
- Size: S (under 50 lines changed) / M / L
- Depends on: other proposals, or none
- Value: low / medium / high. Risk: low / medium / high.

Put the proposals in order of value divided by risk. After them, add:
- Suspected bugs: behavior that looks wrong. No fixes.
- Out of scope notes.
- Kept as is: things that look odd but are correct, with the reason in one line.
  This prevents the next reviewer from proposing them again.

## Scopes

1. `src/reader.rs`
2. `src/reader/pages.rs`, `src/reader/search.rs`, `src/reader/selection.rs`, `src/search.rs`
3. `src/book/`, `src/dither.rs`, `src/image_cache.rs`, `src/cover_adjust.rs`, `src/backdrop.rs`
4. `src/library.rs`, `src/library_view.rs`, `src/scan.rs`, `src/details.rs`, `src/store.rs`
5. `src/shell.rs`, `src/main.rs`, `src/menu.rs`, `src/style.rs`, `src/title_bar.rs`, `src/settings.rs`
6. `src/association.rs`, `src/instance.rs`, `src/sim.rs`, `build.rs`, and the module layout of the crate

Reports go to `.scratch/refactor/proposals/<NN>-<scope-slug>.md`.
