# 07: Search in book

Status: ready-for-agent
Spec: [../spec.md](../spec.md) (Reader: Chrome)
Depends on: 05

## Scope

- Ctrl+F opens a search box. Search the full text of the book, case-insensitive.
- The results list shows the chapter title and some text around each match.
- Click a result to jump to it and highlight the match.
- F3 and Shift+F3 go to the next and previous match. Esc closes the search.
- Run the search off the UI thread.

## Acceptance criteria

- Search finds matches in chapters that are not on screen.
- A match that spans two runs with different styles (for example a bold word) is found.
