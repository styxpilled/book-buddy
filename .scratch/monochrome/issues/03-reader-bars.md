# 03: Reader margins and bars

Status: needs-triage
Spec: [../spec.md](../spec.md)
Depends on: none

## Scope

- Increase the space above the text in the reader.
- Decrease the space below the text in the reader.
- The top bar (title and chapter) is always visible by default. The text starts below it.
- Add a setting to show the top and bottom bars only when the pointer is near them. Default: off.
- With the setting on, the area that shows a bar is larger than the bar. Now the top bar shows only when the pointer is within 12 px of the top (`TOP_EDGE` in `src/reader.rs`).
- Apply this to scroll mode and paginated mode.

## Acceptance criteria

- With the setting off, both bars are always visible and no text is under them.
- With the setting on, each bar shows when the pointer comes near its edge, and hides when the pointer moves away. The trigger area is taller than the bar.
- A change of the setting keeps the reading position.
- The new margins look correct at font sizes 12 and 36.
