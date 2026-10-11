# 05: TOC, links, seek bar, keyboard

Status: ready-for-agent
Spec: [../spec.md](../spec.md) (Reader: Chrome, Keyboard; Content pipeline)
Depends on: 03

## Scope

- TOC side panel with nested entries. Highlight the current chapter. Click an entry to jump to it. T toggles the panel.
- Internal links in runs jump to their anchor. Alt+Left goes back to the position before the jump.
- Top bar with the book title and chapter title. It hides while you read and shows when the mouse moves to the top edge.
- Bottom bar with the percent read and a seek bar. Click or drag on it to jump to that percent.
- The keyboard map from the spec, except search and font size (other issues).

## Acceptance criteria

- Every TOC entry in the fixtures jumps to the correct heading.
- A footnote link jumps to the note, and Alt+Left returns to the same position.
- All keys in the spec table work in scroll mode.
