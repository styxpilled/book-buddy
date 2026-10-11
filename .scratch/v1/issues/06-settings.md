# 06: Settings panel

Status: ready-for-agent
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
