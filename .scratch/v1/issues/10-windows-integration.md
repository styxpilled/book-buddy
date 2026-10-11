# 10: Windows integration

Status: ready-for-agent
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
