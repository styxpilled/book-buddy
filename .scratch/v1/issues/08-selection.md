# 08: Text selection and copy

Status: ready-for-agent
Spec: [../spec.md](../spec.md) (Reader: Selection)
Depends on: 02

## Scope

- Click and drag to select text inside one block. Double-click selects a word.
- Ctrl+C copies the selected text to the clipboard.
- A drag that leaves the block keeps the selection inside the start block.
- A click on a link does not start a selection.

## Acceptance criteria

- Select part of a paragraph that has bold and italic runs, copy it, and paste it: the text is correct.
- Selection does not cause visible lag while scrolling.
