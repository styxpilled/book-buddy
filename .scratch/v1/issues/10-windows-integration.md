# 10: Windows integration

Status: done
Spec: [../spec.md](../spec.md) (Windows integration)
Depends on: 03

## Scope

- Single instance: use a named mutex. A second instance sends its path to the first instance through a named pipe, and then exits. The first instance brings its window to the front and opens the book.
- Settings button that writes the `.epub` file association under `HKCU\Software\Classes`, with the current exe path. No admin rights.
- Save the window position, size, maximized and fullscreen state on close. Restore them on startup. Do not restore a position that is off-screen.
- F11 toggles fullscreen.

## Acceptance criteria

- Double-click an `.epub` in Explorer: it opens in the running window, or starts the app if it is not running.
- The window opens where it was last closed. If that monitor is disconnected, it opens on the main monitor.

## Comments

### 2026-10-13: implementation notes

- All Windows-only code is behind `#[cfg(target_os = "windows")]`. The `windows` crate is a Windows-only dependency, and the `sim` feature is Windows only. Also gated: Show in Explorer, the case-insensitive path match (and its unit test), the Consolas font of `pre` blocks (IBM Plex Mono elsewhere) and `windows_subsystem`. The non-Windows build is not checked: no other target is installed.
- Single instance: `src/instance.rs`. A mutex `Local\book-buddy-<user>` (`book-buddy-dev-<user>` in a debug build, so the dev build never sends to the release build). A later start opens the pipe `\\.\pipe\<same name>`, calls `AllowSetForegroundWindow`, writes its path (or an empty line) and exits. It tries for at most 3 s. If it cannot send, it starts as a normal instance. The running app activates its window and opens the path. The pipe name has the user name, because pipe names are the same for all sessions.
- File association: `src/association.rs`, button `EPUB FILES = REGISTER` in the settings (Windows only). It writes `HKCU\Software\Classes\BookBuddy.epub\shell\open\command` = `"<exe>" "%1"` and the value `BookBuddy.epub` under `.epub\OpenWithProgids`. It sets the default value of `.epub` only when it is empty or absent. Nothing is deleted. The button shows `REGISTERED` (filled) when the command points to this exe. Windows keeps the default app in `FileExts\.epub\UserChoice`, which an app cannot set, so pick the app once in "Open with".
- Window state: `Library::window` in `library.json`: the windowed bounds in logical pixels, `maximized` and `fullscreen`. Saved when the window closes. On startup, the window opens on the display that has the center of the saved bounds. If no display has it, the window opens in the center of the main display with the saved size.
- F11 toggles fullscreen (`ToggleFullscreen`, global binding, handled in `Shell`).
- Custom title bar: `src/title_bar.rs`. On Windows, `appears_transparent` hides the system title bar, and the app draws `[ BOOK-BUDDY ]` and the minimize, maximize and close buttons. They use `window_control_area`, so Windows does the hit test: drag, double-click to maximize, snap layouts and the buttons are native. A fullscreen window has no title bar. The reader subtracts the title bar height in its page geometry, image size and hidden-bar trigger.
- `drive.ps1` and the sim have a new step `bclick:x,y` for the mouse back button.
- Verified on the dev build: a second start with `poetry.epub` exited after 90 ms with code 0, and the running window came to the front and opened the book. A drag on the title bar moved the window. The maximize and close buttons worked with real mouse input. After a close and a restart, the window rect was the same (251,33,1267,841). A saved x of 10000 opened in the center of the main display. A saved position on the second display opened there. F11 hid the title bar and filled the screen, and F11 again restored it. The reader pages fit under the title bar.
- Not verified, as asked: the register button and a double-click in Explorer. The register button was not pressed, so no registry key was written.
