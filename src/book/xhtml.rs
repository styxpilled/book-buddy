use std::collections::HashMap;
use std::mem;

use anyhow::{Result, bail};
use quick_xml::escape::resolve_html5_entity;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use scraper::{Html, Node};

/// Offsets count chars of block text, plus one separator after each block.
#[derive(Debug, Default)]
pub struct Chapter {
    pub blocks: Vec<Block>,
    pub anchors: HashMap<String, usize>,
    pub len: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub offset: usize,
    pub quote: u8,
    pub kind: BlockKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlockKind {
    Heading {
        level: u8,
        runs: Vec<Run>,
    },
    Paragraph {
        runs: Vec<Run>,
    },
    /// `marker` is `None` for the second and later blocks of one list item.
    ListItem {
        depth: u8,
        marker: Option<Marker>,
        runs: Vec<Run>,
    },
    Pre {
        text: String,
    },
    Image {
        href: String,
        alt: String,
    },
    Rule,
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Marker {
    Bullet,
    Number(u32),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Run {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub link: Option<String>,
}

impl Chapter {
    pub fn error(message: String) -> Self {
        Self {
            blocks: vec![Block {
                offset: 0,
                quote: 0,
                kind: BlockKind::Error { message },
            }],
            anchors: HashMap::new(),
            len: 1,
        }
    }
}

/// `base` is the absolute href of the chapter. Links and images are resolved against it.
pub fn parse(source: &str, base: &str) -> Chapter {
    let mut builder = Builder::new(base);
    if parse_xml(source, &mut builder).is_err() {
        builder = Builder::new(base);
        parse_html(source, &mut builder);
    }
    builder.finish()
}

fn parse_xml(source: &str, b: &mut Builder) -> Result<()> {
    let mut reader = Reader::from_str(source);
    loop {
        match reader.read_event()? {
            Event::Start(e) => b.start(&local_name(&e), &xml_attrs(&e)?),
            Event::Empty(e) => {
                let name = local_name(&e);
                b.start(&name, &xml_attrs(&e)?);
                b.end(&name);
            }
            Event::End(e) => b.end(e.local_name().as_ref()),
            Event::Text(e) => b.text(&e.xml_content(XmlVersion::Implicit1_0)),
            Event::CData(e) => b.text(&e.xml_content(XmlVersion::Implicit1_0)),
            Event::GeneralRef(e) => match e.resolve_char_ref()? {
                Some(c) => b.text(c.encode_utf8(&mut [0; 4])),
                None => match resolve_html5_entity(&e.xml_content(XmlVersion::Implicit1_0)) {
                    Some(s) => b.text(s),
                    None => bail!("unknown entity"),
                },
            },
            Event::Eof => return Ok(()),
            _ => {}
        }
    }
}

fn local_name(e: &BytesStart) -> String {
    e.local_name().as_ref().to_string()
}

fn xml_attrs(e: &BytesStart) -> Result<Vec<(String, String)>> {
    e.attributes()
        .map(|a| {
            let a = a?;
            let key = a.key.local_name().as_ref().to_string();
            let value =
                a.normalized_value_with(XmlVersion::Implicit1_0, 128, resolve_html5_entity)?;
            Ok((key, value.into_owned()))
        })
        .collect()
}

fn parse_html(source: &str, b: &mut Builder) {
    walk(Html::parse_document(source).tree.root(), b);
}

fn walk(node: ego_tree::NodeRef<Node>, b: &mut Builder) {
    match node.value() {
        Node::Element(e) => {
            let attrs: Vec<_> = e
                .attrs()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            b.start(e.name(), &attrs);
            node.children().for_each(|c| walk(c, b));
            b.end(e.name());
        }
        Node::Text(t) => b.text(t),
        _ => node.children().for_each(|c| walk(c, b)),
    }
}

const SKIP: &[&str] = &["head", "script", "style", "template", "noscript"];

/// Elements that end the current block of inline text.
#[rustfmt::skip]
const BREAKS: &[&str] = &[
    "address", "article", "aside", "blockquote", "body", "caption", "center", "dd", "div", "dl",
    "dt", "figcaption", "figure", "footer", "header", "hr", "image", "img", "li", "main", "nav",
    "ol", "p", "pre", "section", "table", "tbody", "td", "tfoot", "th", "thead", "tr", "ul",
];

fn heading_level(name: &str) -> Option<u8> {
    match name.as_bytes() {
        [b'h', n @ b'1'..=b'6'] => Some(n - b'0'),
        _ => None,
    }
}

fn attr<'a>(attrs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0C')
}

/// Maps C1 controls to Windows-1252, as browsers do. Many books use `&#151;` for an em dash.
fn cp1252(c: char) -> char {
    const TABLE: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8D}', 'Ž',
        '\u{8F}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9D}',
        'ž', 'Ÿ',
    ];
    match c {
        '\u{80}'..='\u{9F}' => TABLE[c as usize - 0x80],
        _ => c,
    }
}

pub fn resolve(base: &str, href: &str) -> String {
    let scheme = href
        .find(':')
        .is_some_and(|i| !href[..i].contains(['/', '#', '?']));
    if scheme {
        return href.to_string();
    }
    let base = base.split('#').next().unwrap_or_default();
    if href.starts_with('#') {
        return format!("{base}{href}");
    }
    if href.starts_with('/') {
        return normalize(href);
    }
    let dir = &base[..base.rfind('/').map_or(0, |i| i + 1)];
    normalize(&format!("{dir}{href}"))
}

fn normalize(href: &str) -> String {
    let (path, fragment) = href.split_at(href.find('#').unwrap_or(href.len()));
    let mut parts = Vec::new();
    for segment in path.split('/') {
        match segment {
            "." => {}
            ".." => {
                if parts.len() > 1 {
                    parts.pop();
                }
            }
            s => parts.push(s),
        }
    }
    parts.join("/") + fragment
}

struct List {
    ordered: bool,
    next: u32,
}

struct Builder<'a> {
    base: &'a str,
    blocks: Vec<Block>,
    anchors: HashMap<String, usize>,
    offset: usize,
    runs: Vec<Run>,
    run_len: usize,
    space: bool,
    skip: u32,
    bold: u32,
    italic: u32,
    links: Vec<Option<String>>,
    heading: Option<u8>,
    quote: u8,
    lists: Vec<List>,
    item: u32,
    marker: Option<Marker>,
    pre: Option<String>,
}

impl<'a> Builder<'a> {
    fn new(base: &'a str) -> Self {
        Self {
            base,
            blocks: Vec::new(),
            anchors: HashMap::new(),
            offset: 0,
            runs: Vec::new(),
            run_len: 0,
            space: true,
            skip: 0,
            bold: 0,
            italic: 0,
            links: Vec::new(),
            heading: None,
            quote: 0,
            lists: Vec::new(),
            item: 0,
            marker: None,
            pre: None,
        }
    }

    fn start(&mut self, name: &str, attrs: &[(String, String)]) {
        if SKIP.contains(&name) {
            self.skip += 1;
            return;
        }
        if self.skip > 0 {
            return;
        }
        if let Some(pre) = &mut self.pre {
            if name == "br" {
                pre.push('\n');
            }
            return;
        }

        let level = heading_level(name);
        if level.is_some() || BREAKS.contains(&name) {
            self.flush();
        }
        let id = attr(attrs, "id").or(if name == "a" {
            attr(attrs, "name")
        } else {
            None
        });
        if let Some(id) = id {
            self.anchors
                .entry(id.to_string())
                .or_insert(self.offset + self.run_len);
        }

        if level.is_some() {
            self.heading = level;
            return;
        }
        match name {
            "blockquote" => self.quote = self.quote.saturating_add(1),
            "ul" | "ol" => {
                let next = attr(attrs, "start")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1);
                self.lists.push(List {
                    ordered: name == "ol",
                    next,
                });
            }
            "li" => {
                self.item += 1;
                self.marker = Some(match self.lists.last_mut() {
                    Some(list) if list.ordered => {
                        list.next += 1;
                        Marker::Number(list.next - 1)
                    }
                    _ => Marker::Bullet,
                });
            }
            "pre" => self.pre = Some(String::new()),
            "hr" => self.push(BlockKind::Rule, 0),
            "img" | "image" => {
                if let Some(src) = attr(attrs, "src").or(attr(attrs, "href")) {
                    let href = resolve(self.base, src);
                    let alt = attr(attrs, "alt").unwrap_or_default().to_string();
                    self.push(BlockKind::Image { href, alt }, 0);
                }
            }
            "br" => self.line_break(),
            "b" | "strong" => self.bold += 1,
            "i" | "em" | "cite" | "dfn" | "var" => self.italic += 1,
            "a" => self
                .links
                .push(attr(attrs, "href").map(|h| resolve(self.base, h))),
            _ => {}
        }
    }

    fn end(&mut self, name: &str) {
        if SKIP.contains(&name) {
            self.skip = self.skip.saturating_sub(1);
            return;
        }
        if self.skip > 0 {
            return;
        }
        if self.pre.is_some() {
            if name == "pre" {
                self.end_pre();
            }
            return;
        }

        if heading_level(name).is_some() {
            self.flush();
            self.heading = None;
            return;
        }
        match name {
            "blockquote" => {
                self.flush();
                self.quote = self.quote.saturating_sub(1);
            }
            "ul" | "ol" => {
                self.flush();
                self.lists.pop();
            }
            "li" => {
                self.flush();
                self.item = self.item.saturating_sub(1);
                self.marker = None;
            }
            "b" | "strong" => self.bold = self.bold.saturating_sub(1),
            "i" | "em" | "cite" | "dfn" | "var" => self.italic = self.italic.saturating_sub(1),
            "a" => {
                self.links.pop();
            }
            _ if BREAKS.contains(&name) => self.flush(),
            _ => {}
        }
    }

    fn text(&mut self, s: &str) {
        if self.skip > 0 {
            return;
        }
        if let Some(pre) = &mut self.pre {
            pre.extend(s.chars().map(cp1252));
            return;
        }
        let mut space = self.space;
        let mut len = 0;
        let run = self.current_run();
        for c in s.chars() {
            if is_space(c) {
                if space {
                    continue;
                }
                run.text.push(' ');
                space = true;
            } else {
                run.text.push(cp1252(c));
                space = false;
            }
            len += 1;
        }
        self.space = space;
        self.run_len += len;
    }

    fn line_break(&mut self) {
        if self.run_len == 0 {
            return;
        }
        let run = self.current_run();
        if run.text.ends_with(' ') {
            run.text.pop();
            run.text.push('\n');
        } else {
            run.text.push('\n');
            self.run_len += 1;
        }
        self.space = true;
    }

    fn current_run(&mut self) -> &mut Run {
        let bold = self.bold > 0;
        let italic = self.italic > 0;
        let link = self.links.last().cloned().flatten();
        let same = self
            .runs
            .last()
            .is_some_and(|r| r.bold == bold && r.italic == italic && r.link == link);
        if !same {
            self.runs.push(Run {
                text: String::new(),
                bold,
                italic,
                link,
            });
        }
        self.runs.last_mut().unwrap()
    }

    fn flush(&mut self) {
        let mut runs = mem::take(&mut self.runs);
        self.space = true;
        while let Some(last) = runs.last_mut() {
            let trimmed = last.text.trim_end_matches([' ', '\n']).len();
            self.run_len -= last.text.len() - trimmed;
            last.text.truncate(trimmed);
            if !last.text.is_empty() {
                break;
            }
            runs.pop();
        }
        runs.retain(|r| !r.text.is_empty());
        let len = mem::take(&mut self.run_len);
        if len == 0 {
            return;
        }
        let kind = if let Some(level) = self.heading {
            BlockKind::Heading { level, runs }
        } else if self.item > 0 {
            let depth = self.lists.len().clamp(1, u8::MAX as usize) as u8;
            BlockKind::ListItem {
                depth,
                marker: self.marker.take(),
                runs,
            }
        } else {
            BlockKind::Paragraph { runs }
        };
        self.push(kind, len);
    }

    fn end_pre(&mut self) {
        let text = self.pre.take().unwrap_or_default();
        let text = text.strip_prefix('\n').unwrap_or(&text).trim_end();
        if !text.is_empty() {
            let len = text.chars().count();
            self.push(
                BlockKind::Pre {
                    text: text.to_string(),
                },
                len,
            );
        }
    }

    fn push(&mut self, kind: BlockKind, len: usize) {
        self.blocks.push(Block {
            offset: self.offset,
            quote: self.quote,
            kind,
        });
        self.offset += len + 1;
    }

    fn finish(mut self) -> Chapter {
        if self.pre.is_some() {
            self.end_pre();
        }
        self.flush();
        Chapter {
            blocks: self.blocks,
            anchors: self.anchors,
            len: self.offset,
        }
    }
}

#[cfg(test)]
pub(crate) fn describe(block: &Block) -> String {
    let text = |runs: &[Run]| runs.iter().map(|r| r.text.as_str()).collect::<String>();
    match &block.kind {
        BlockKind::Heading { level, runs } => format!("h{level} {}", text(runs)),
        BlockKind::Paragraph { runs } => format!("p {}", text(runs)),
        BlockKind::ListItem {
            depth,
            marker,
            runs,
        } => {
            let marker = match marker {
                Some(Marker::Bullet) => "*".to_string(),
                Some(Marker::Number(n)) => format!("{n}."),
                None => "-".to_string(),
            };
            format!("li{depth} {marker} {}", text(runs))
        }
        BlockKind::Pre { text } => format!("pre {text}"),
        BlockKind::Image { href, alt } => format!("img {href} {alt}"),
        BlockKind::Rule => "hr".to_string(),
        BlockKind::Error { message } => format!("error {message}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "/OEBPS/text/c1.xhtml";

    fn chapter(body: &str) -> Chapter {
        parse(
            &format!("<html xmlns=\"http://www.w3.org/1999/xhtml\"><body>{body}</body></html>"),
            BASE,
        )
    }

    fn describe_all(chapter: &Chapter) -> Vec<String> {
        chapter.blocks.iter().map(describe).collect()
    }

    fn runs(block: &Block) -> &[Run] {
        match &block.kind {
            BlockKind::Heading { runs, .. }
            | BlockKind::Paragraph { runs }
            | BlockKind::ListItem { runs, .. } => runs,
            _ => &[],
        }
    }

    fn run(text: &str, bold: bool, italic: bool) -> Run {
        Run {
            text: text.to_string(),
            bold,
            italic,
            link: None,
        }
    }

    #[test]
    fn collapses_whitespace_and_keeps_emphasis() {
        let ch = chapter("<p>  Hello   <b>bold <i>both</i></b>\n world  </p>");
        assert_eq!(
            runs(&ch.blocks[0]),
            [
                run("Hello ", false, false),
                run("bold ", true, false),
                run("both", true, true),
                run(" world", false, false),
            ]
        );
        assert_eq!(ch.len, "Hello bold both world".len() + 1);
    }

    #[test]
    fn line_breaks() {
        let ch = chapter("<p><br/>a <br/> b<br/></p>");
        assert_eq!(describe_all(&ch), ["p a\nb"]);
        assert_eq!(ch.len, 4);
    }

    #[test]
    fn lists() {
        let ch = chapter(
            "<ol start=\"3\"><li>a</li><li>b<ul><li>c</li></ul></li></ol>\
             <ul><li><p>d</p><p>e</p></li></ul>",
        );
        assert_eq!(
            describe_all(&ch),
            ["li1 3. a", "li1 4. b", "li2 * c", "li1 * d", "li1 - e"]
        );
    }

    #[test]
    fn quotes_rules_and_pre() {
        let ch = chapter(
            "<blockquote><p>q</p><blockquote>qq</blockquote></blockquote><hr/>\
             <pre>\n  a &amp; <b>b</b>\n    c\n</pre><p>after</p>",
        );
        assert_eq!(
            describe_all(&ch),
            ["p q", "p qq", "hr", "pre   a & b\n    c", "p after"]
        );
        let quotes: Vec<_> = ch.blocks.iter().map(|b| b.quote).collect();
        assert_eq!(quotes, [1, 2, 0, 0, 0]);
    }

    #[test]
    fn anchors_links_and_images() {
        let ch = chapter(
            "<h2 id=\"top\">Title</h2>\
             <p>One <span id=\"mid\">two</span> <a href=\"c2.xhtml#n1\">note</a> \
             <a href=\"#top\">up</a> <a href=\"http://x.org/a\">ext</a></p>\
             <img id=\"pic\" src=\"../images/a.png\" alt=\"A\"/>\
             <svg><image xlink:href=\"../images/b.jpg\"/></svg>",
        );
        assert_eq!(ch.anchors["top"], 0);
        assert_eq!(ch.anchors["mid"], 10);
        assert_eq!(ch.anchors["pic"], ch.blocks[2].offset);
        let links: Vec<_> = runs(&ch.blocks[1])
            .iter()
            .filter_map(|r| r.link.as_deref())
            .collect();
        assert_eq!(
            links,
            [
                "/OEBPS/text/c2.xhtml#n1",
                "/OEBPS/text/c1.xhtml#top",
                "http://x.org/a"
            ]
        );
        assert_eq!(describe(&ch.blocks[2]), "img /OEBPS/images/a.png A");
        assert_eq!(describe(&ch.blocks[3]), "img /OEBPS/images/b.jpg ");
    }

    #[test]
    fn entities() {
        let ch = chapter("<p>a&nbsp;b &amp; c&#151;d &#x2014; &hellip;</p>");
        assert_eq!(describe_all(&ch), ["p a\u{a0}b & c—d — …"]);
    }

    #[test]
    fn ignores_head_script_and_style() {
        let ch = parse(
            "<html><head><title>T</title><style>p { color: red }</style></head>\
             <body><script>var x;</script><p style=\"color: red\" class=\"x\">body</p></body></html>",
            BASE,
        );
        assert_eq!(describe_all(&ch), ["p body"]);
    }

    #[test]
    fn malformed_markup_falls_back_to_html_parser() {
        let ch = parse("<html><body><p>one<br>two<p>three &copy; <b>x</body>", BASE);
        assert_eq!(describe_all(&ch), ["p one\ntwo", "p three © x"]);
        assert_eq!(
            runs(&ch.blocks[1]),
            [run("three © ", false, false), run("x", true, false)]
        );
    }

    #[test]
    fn resolves_hrefs() {
        assert_eq!(resolve(BASE, "../../x.png"), "/x.png");
        assert_eq!(resolve(BASE, "./a/./b.xhtml#f"), "/OEBPS/text/a/b.xhtml#f");
        assert_eq!(resolve("/OEBPS/c.xhtml#old", "#new"), "/OEBPS/c.xhtml#new");
        assert_eq!(resolve(BASE, "mailto:a@b.c"), "mailto:a@b.c");
    }
}
