# 06: Settings panel

Status: done
Spec: [../spec.md](../spec.md) (Settings)
Depends on: 04

## Scope

- Settings panel that opens from the library and from the reader.
- Font: serif or sans. Font size, also with Ctrl+= and Ctrl+-.
- Theme: light or dark. It applies to the reader and the library.
- Add and remove watched folders.
- Mode setting (scroll or paginated). Paginated mode stays disabled until issue 09.
- Store the settings in `library.json`.

## Acceptance criteria

- A change to a setting applies at once and keeps the reading position.
- Settings are the same after a restart.

## Comments

### 2026-10-12: implementation notes

- Code: `Settings` in `src/library.rs` (`settings` in `library.json`, with defaults for a missing key), `src/settings.rs` (panel, actions, `change`), and the font fields of `Reader`.
- The panel is an overlay in `Shell`. Ctrl+, opens it, and there is a "Settings" button in the library toolbar and in the reader bottom bar. Esc, "Close" or a click outside the panel closes it. Focus then goes back to the reader or the library.
- Controls: theme (light or dark), font (serif or sans), font size (- and +), mode (scroll; paginated is disabled until issue 09), watched folders (add with a folder dialog, remove).
- Ctrl+= and Ctrl+- are global. The font size is 12 to 36. Each change is saved at once.
- Theme: the gpui-kit light or dark theme. It is applied on startup and after each change.
- Position: a font change calls `ListState::remeasure`, which keeps the scrolled part of the top block.
- A new watched folder starts a scan. A removed folder keeps its books in the library.
- Unit test: settings load with defaults for missing keys, and save and load again.
- Verified with sim on `basic-v3plus2.epub`: open from the reader (button and Ctrl+,) and from the library, dark theme, sans, size 20, Ctrl+= and Ctrl+- three times back to the same pixel, restart with the same settings and position, remove a folder, click outside and Esc close the panel and the reader keys work after.
- Not verified: the "Add folder" dialog (sim cannot drive a native dialog).
- Known: at the end of the book, where the list cannot put the position at the top, a font change can move the text a little. This is the same limit as in issue 05.
