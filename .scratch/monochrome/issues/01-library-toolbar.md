# 01: Library toolbar

Status: needs-triage
Spec: [../spec.md](../spec.md)
Depends on: none

## Scope

- Remove the "SORT =" text. The sort button shows only the current order, for example "LAST OPENED".
- Move the "SHOW HIDDEN" checkbox from the library toolbar to the settings panel. Store it in the settings, so it persists.
- Make one header from the two rows at the top of the library (the toolbar row and the "LIBRARY" row with the rule and the book count).

## Acceptance criteria

- The library has one header row. It holds the filter, sort, add books, settings and the book count.
- "Show hidden" in settings shows and hides the hidden books at once, and keeps its value after a restart.
- The sort menu works as before.
