use std::ops::Range;

/// Byte ranges of the matches of `query` in `text`, case-insensitive. Matches
/// do not overlap.
pub fn find(text: &str, query: &str) -> Vec<Range<usize>> {
    let needle: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    if needle.is_empty() {
        return Vec::new();
    }
    // Each lower case char, with the byte offset of the char it comes from.
    let haystack: Vec<(usize, char)> = text
        .char_indices()
        .flat_map(|(i, c)| c.to_lowercase().map(move |l| (i, l)))
        .collect();
    let mut found = Vec::new();
    let mut i = 0;
    while i + needle.len() <= haystack.len() {
        let window = &haystack[i..i + needle.len()];
        if window.iter().map(|(_, c)| c).eq(needle.iter()) {
            let end = haystack
                .get(i + needle.len())
                .map_or(text.len(), |(start, _)| *start);
            found.push(window[0].0..end);
            i += needle.len();
        } else {
            i += 1;
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::{BlockKind, parse_xhtml};

    #[test]
    fn case_insensitive() {
        assert_eq!(find("Ab ab AB", "aB"), [0..2, 3..5, 6..8]);
        assert_eq!(find("Ünïcode café", "CAFÉ"), vec![10..15]);
        assert_eq!(find("aaaa", "aa"), [0..2, 2..4]);
        assert!(find("abc", "").is_empty());
        assert!(find("abc", "abcd").is_empty());
    }

    #[test]
    fn match_spans_runs_with_different_styles() {
        let chapter = parse_xhtml(
            "<html><body><p>A <b>bold</b> <i>word</i> here.</p></body></html>",
            "/c.xhtml",
        );
        let block = &chapter.blocks[0];
        let BlockKind::Paragraph { runs } = &block.kind else {
            panic!("not a paragraph");
        };
        assert!(runs.len() > 1);
        let text = block.text();
        let found = find(&text, "a BOLD word");
        assert_eq!(found.len(), 1);
        assert_eq!(&text[found[0].clone()], "A bold word");
    }
}
