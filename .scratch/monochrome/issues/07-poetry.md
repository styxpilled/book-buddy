# 07: Poetry layout

Status: done
Spec: [../spec.md](../spec.md)
Depends on: none

## Scope

Poems in `books/poetry.epub` ("Glass Armonica") do not look like poems. `screenshots/` shows another reader on the left and book-buddy on the right.

Problems:

1. Each line of a poem has the full paragraph gap below it. Lines of one stanza must be close, with a gap only between stanzas (screenshots 1, 2 and 4).
2. Stanza and section gaps are lost. The book marks them with empty elements: `<div class="pseudo_margin_1"/>` and `<div class="pseudo_margin_2"/>`.
3. Indented lines lose the indent. The book wraps them in a `div`, for example `<div class="inset_3"><p>pill-bottle brown.</p></div>`, and the subtitle "after the triptych by Hieronymous Bosch" (screenshot 4).
4. The part title page "GLASS ARMONICA" (`Part001.xhtml`) is a centered `h1` with empty spacers above it. book-buddy shows it at the top left (screenshot 3).

The markup of one poem (`Chapter003.xhtml`):

```html
<h2 class="poem_title">A FRIGHTFUL RELEASE</h2>
<div class="pseudo_margin_2"/>
<p><i>A piece of coffee</i></p>
<div class="pseudo_margin_2"/>
<p class="negative_indent_1">Coffee: of the madder family. And what</p>
<p class="negative_indent_1">of my own? We're mad as any beans that click</p>
<div class="inset_3">
<p>pill-bottle brown.</p>
</div>
```

The spec says the reader ignores book CSS. A fix must stay inside that rule, or this issue must change the rule on purpose. Options to decide in triage:

- Read a few layout facts from the markup only: empty block elements give vertical space, a wrapping `div` with a class gives an indent, a run of one-line paragraphs with no gap is verse.
- Read a small, fixed part of the book CSS (margins, `text-indent`, `text-align: center`) for the classes in use.

## Acceptance criteria

- In `poetry.epub`, the lines of a stanza are close together, with a clear gap between stanzas and before the italic section titles.
- The indented last lines ("pill-bottle brown.", "Return to sender.") are indented.
- Prose books in `books/` look the same as before.
- Paginated mode keeps stanzas and indents.

## Comments

Done with the markup-only option. The reader still ignores book CSS.

- An empty `div` is a gap of 1 em. Empty `p` elements do not count: Gutenberg books use them in contents pages.
- A `div` with a class that holds only one paragraph is an inset of 2 em.
- A chapter is verse if it has 3 or more paragraphs, 80% of them have 60 chars or fewer, and a paragraph has a gap before it. In a verse chapter, paragraphs have no space below them.
- A chapter with only headings is a title page. The headings are centered and keep their gaps.
- In other chapters, gaps and insets are ignored.
- Of the books in `books/`, only `pg2554` changes: its "CRIME AND PUNISHMENT / PART I" page is now a centered title page.
- Not done: the hanging indent of wrapped verse lines (`negative_indent_1`). GPUI text has no `text-indent`.
