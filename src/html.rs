use std::sync::OnceLock;

use ego_tree::NodeRef;
use scraper::{ElementRef, Html, Node, Selector};
use serde::Serialize;
use unicode_width::UnicodeWidthStr;

use crate::style::{St, Style};

#[derive(Clone, Copy, Debug)]
pub struct RenderOpts {
    /// 0 disables wrapping.
    pub width: usize,
    pub style: Style,
    pub links: bool,
}

impl Default for RenderOpts {
    fn default() -> Self {
        Self {
            width: 0,
            style: Style::PLAIN,
            links: false,
        }
    }
}

#[derive(Clone)]
enum Tok {
    Word { text: String, style: St, glue: bool },
    Break,
}

fn word(text: impl Into<String>, style: St) -> Tok {
    Tok::Word {
        text: text.into(),
        style,
        glue: false,
    }
}

#[derive(Serialize, Debug, Default)]
pub struct Section {
    pub heading: Option<String>,
    pub text: String,
}

#[derive(Serialize, Debug, Default)]
pub struct Link {
    pub text: String,
    pub href: String,
}

#[derive(Serialize, Debug, Default)]
pub struct Page {
    pub title: String,
    pub sections: Vec<Section>,
    pub links: Vec<Link>,
    pub text: String,
}

fn main_selector() -> &'static Selector {
    static SELECTOR: OnceLock<Selector> = OnceLock::new();
    SELECTOR.get_or_init(|| Selector::parse("main#content").unwrap())
}

fn h1_selector() -> &'static Selector {
    static SELECTOR: OnceLock<Selector> = OnceLock::new();
    SELECTOR.get_or_init(|| Selector::parse("h1").unwrap())
}

fn link_selector() -> &'static Selector {
    static SELECTOR: OnceLock<Selector> = OnceLock::new();
    SELECTOR.get_or_init(|| Selector::parse("a[href]").unwrap())
}

fn row_selector() -> &'static Selector {
    static SELECTOR: OnceLock<Selector> = OnceLock::new();
    SELECTOR.get_or_init(|| Selector::parse("tr").unwrap())
}

fn cell_selector() -> &'static Selector {
    static SELECTOR: OnceLock<Selector> = OnceLock::new();
    SELECTOR.get_or_init(|| Selector::parse("td, th").unwrap())
}

/// Render the main content of a page to styled terminal text.
pub fn render_page(html: &str, opts: &RenderOpts) -> String {
    let document = Html::parse_document(html);
    let Some(main) = document.select(main_selector()).next() else {
        return String::new();
    };
    let mut renderer = Renderer::new(opts);
    renderer.render_element(main);
    renderer.finish()
}

pub fn plain_text(html: &str, width: usize) -> String {
    render_page(
        html,
        &RenderOpts {
            width,
            style: Style::PLAIN,
            links: false,
        },
    )
}

/// Structured view of a page, used by `--json` and section-aware commands.
pub fn parse_page(html: &str) -> Page {
    let document = Html::parse_document(html);
    let Some(main) = document.select(main_selector()).next() else {
        return Page::default();
    };

    let mut page = Page {
        title: main
            .select(h1_selector())
            .next()
            .map(|heading| collapse(&heading.text().collect::<String>()))
            .unwrap_or_default(),
        ..Page::default()
    };

    for link in main.select(link_selector()) {
        let href = link.value().attr("href").unwrap_or("").trim();
        let text = collapse(&link.text().collect::<String>());
        if !href.is_empty() && !text.is_empty() {
            page.links.push(Link {
                text,
                href: href.to_string(),
            });
        }
    }

    let opts = RenderOpts {
        width: 100,
        style: Style::PLAIN,
        links: false,
    };
    let mut heading: Option<String> = None;
    let mut renderer = Renderer::new(&opts);

    for child in main.children() {
        if let Node::Element(element) = child.value()
            && matches!(element.name(), "h1" | "h2" | "h3" | "h4" | "h5" | "h6")
        {
            let text = renderer.finish();
            push_section(&mut page.sections, heading.take(), text);
            renderer = Renderer::new(&opts);
            heading = Some(collapse(
                &ElementRef::wrap(child)
                    .map(|element| element.text().collect::<String>())
                    .unwrap_or_default(),
            ));
            continue;
        }
        renderer.render_node(child);
    }
    let text = renderer.finish();
    push_section(&mut page.sections, heading.take(), text);

    page.text = plain_text(html, 100);
    page
}

fn push_section(sections: &mut Vec<Section>, heading: Option<String>, text: String) {
    let text = text.trim_end().to_string();
    if heading.is_some() || !text.is_empty() {
        sections.push(Section { heading, text });
    }
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn combine(base: St, extra: St) -> St {
    match extra {
        St::Normal => base,
        _ => extra,
    }
}

/// Punctuation attaches to the previous word even when the HTML source has
/// whitespace around it (the site puts list commas on their own line).
fn is_punctuation(piece: &str) -> bool {
    !piece.is_empty()
        && piece.chars().all(|c| {
            matches!(
                c,
                '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}' | '%' | '…' | '’' | '"' | '\''
            )
        })
}

fn is_block_tag(tag: &str) -> bool {
    matches!(
        tag,
        "p" | "div"
            | "section"
            | "article"
            | "main"
            | "body"
            | "ul"
            | "ol"
            | "li"
            | "pre"
            | "blockquote"
            | "table"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "figure"
            | "details"
            | "hr"
            | "form"
            | "nav"
            | "header"
            | "footer"
            | "aside"
    )
}

fn is_visible_link(href: &str) -> bool {
    if href.starts_with('#') || href.starts_with('/') {
        return false;
    }
    if let Some(rest) = href
        .strip_prefix("https://")
        .or_else(|| href.strip_prefix("http://"))
    {
        let host = rest.split('/').next().unwrap_or("");
        return !matches!(
            host,
            "overthewire.org"
                | "www.overthewire.org"
                | "manpages.ubuntu.com"
                | "manpages.debian.org"
        );
    }
    false
}

struct Renderer {
    out: String,
    width: usize,
    style: Style,
    links: bool,
}

impl Renderer {
    fn new(opts: &RenderOpts) -> Self {
        Self {
            out: String::new(),
            width: opts.width,
            style: opts.style,
            links: opts.links,
        }
    }

    fn finish(mut self) -> String {
        while self.out.ends_with('\n') {
            self.out.pop();
        }
        if !self.out.is_empty() {
            self.out.push('\n');
        }
        self.out
    }

    fn blank(&mut self) {
        if self.out.is_empty() {
            return;
        }
        if !self.out.ends_with('\n') {
            self.out.push('\n');
        }
        if !self.out.ends_with("\n\n") {
            self.out.push('\n');
        }
    }

    fn line(&mut self, text: &str) {
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn render_node(&mut self, node: NodeRef<'_, Node>) {
        match node.value() {
            Node::Text(text) => {
                let text = text.text.trim();
                if text.is_empty() {
                    return;
                }
                let words: Vec<Tok> = text
                    .split_whitespace()
                    .map(|piece| word(piece, St::Normal))
                    .collect();
                self.blank();
                self.write_tokens(&words, 0, None);
            }
            Node::Element(_) => {
                if let Some(element) = ElementRef::wrap(node) {
                    self.render_element(element);
                }
            }
            _ => {}
        }
    }

    fn render_element(&mut self, element: ElementRef<'_>) {
        let tag = element.value().name();
        match tag {
            "script" | "style" | "noscript" | "template" | "svg" => {}
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let level = tag[1..].parse::<usize>().unwrap_or(2);
                let mut tokens = self.inline(element);
                if tokens.is_empty() {
                    return;
                }
                self.blank();
                match level {
                    1 => {
                        restyle(&mut tokens, St::Accent);
                        self.write_tokens(&tokens, 0, None);
                    }
                    2 => {
                        restyle(&mut tokens, St::Bold);
                        self.write_tokens(&tokens, 0, Some(("▌ ", St::Dim)));
                    }
                    _ => {
                        restyle(&mut tokens, St::Bold);
                        self.write_tokens(&tokens, 0, None);
                    }
                }
            }
            "p" => {
                let tokens = self.inline(element);
                if !tokens.is_empty() {
                    self.blank();
                    self.write_tokens(&tokens, 0, None);
                }
            }
            "ul" | "ol" => {
                self.blank();
                self.render_list(element, 0);
                self.blank();
            }
            "pre" => self.render_pre(element),
            "blockquote" => self.render_quote(element),
            "hr" => {
                self.blank();
                let width = if self.width == 0 {
                    40
                } else {
                    self.width.min(40)
                };
                self.line(&self.style.paint(St::Dim, &"─".repeat(width)));
                self.blank();
            }
            "br" => self.blank(),
            "img" => {
                if let Some(alt) = element.value().attr("alt") {
                    let alt = alt.trim();
                    if !alt.is_empty() {
                        self.blank();
                        self.line(&self.style.paint(St::Dim, &format!("[image: {alt}]")));
                    }
                }
            }
            "table" => self.render_table(element),
            _ => self.render_children(element),
        }
    }

    fn render_children(&mut self, element: ElementRef<'_>) {
        let mut tokens = Vec::new();
        let mut pending_space = true;
        for child in element.children() {
            let is_block = match child.value() {
                Node::Element(child) => is_block_tag(child.name()),
                _ => false,
            };
            if is_block {
                if !tokens.is_empty() {
                    self.blank();
                    self.write_tokens(&tokens, 0, None);
                    tokens.clear();
                    pending_space = true;
                }
                self.render_node(child);
            } else {
                self.collect_inline(child, St::Normal, &mut tokens, &mut pending_space);
            }
        }
        if !tokens.is_empty() {
            self.blank();
            self.write_tokens(&tokens, 0, None);
        }
    }

    fn inline(&self, element: ElementRef<'_>) -> Vec<Tok> {
        let mut tokens = Vec::new();
        let mut pending_space = true;
        for child in element.children() {
            self.collect_inline(child, St::Normal, &mut tokens, &mut pending_space);
        }
        tokens
    }

    fn collect_inline(
        &self,
        node: NodeRef<'_, Node>,
        style: St,
        tokens: &mut Vec<Tok>,
        pending_space: &mut bool,
    ) {
        match node.value() {
            Node::Text(text) => {
                let raw = text.text.as_ref();
                if raw.is_empty() {
                    return;
                }
                let leading_space = raw.chars().next().is_none_or(char::is_whitespace);
                for (index, piece) in raw.split_whitespace().enumerate() {
                    let glue = if index == 0 {
                        (!*pending_space && !leading_space) || is_punctuation(piece)
                    } else {
                        is_punctuation(piece)
                    };
                    tokens.push(Tok::Word {
                        text: piece.to_string(),
                        style,
                        glue,
                    });
                }
                *pending_space = raw.chars().last().is_none_or(char::is_whitespace);
            }
            Node::Element(element) => {
                let tag = element.name();
                match tag {
                    "br" => {
                        tokens.push(Tok::Break);
                        *pending_space = true;
                    }
                    "script" | "style" | "noscript" | "template" | "svg" => {}
                    "strong" | "b" => {
                        let next = combine(style, St::Bold);
                        for child in node.children() {
                            self.collect_inline(child, next, tokens, pending_space);
                        }
                    }
                    "em" | "i" | "cite" | "q" => {
                        let next = combine(style, St::Italic);
                        for child in node.children() {
                            self.collect_inline(child, next, tokens, pending_space);
                        }
                    }
                    "code" | "kbd" | "samp" | "tt" => {
                        let next = combine(style, St::Code);
                        for child in node.children() {
                            self.collect_inline(child, next, tokens, pending_space);
                        }
                    }
                    "a" => {
                        let href = element.attr("href").unwrap_or("");
                        let label = collapse(
                            &ElementRef::wrap(node)
                                .map(|element| element.text().collect::<String>())
                                .unwrap_or_default(),
                        );
                        for child in node.children() {
                            self.collect_inline(child, style, tokens, pending_space);
                        }
                        if self.links
                            && is_visible_link(href)
                            && !label.is_empty()
                            && !label.eq_ignore_ascii_case(href)
                        {
                            tokens.push(word(format!("<{href}>"), St::Dim));
                            *pending_space = true;
                        }
                    }
                    "img" => {
                        if let Some(alt) = element.attr("alt") {
                            let alt = alt.trim();
                            if !alt.is_empty() {
                                tokens.push(word(format!("[{alt}]"), St::Dim));
                                *pending_space = false;
                            }
                        }
                    }
                    _ => {
                        for child in node.children() {
                            self.collect_inline(child, style, tokens, pending_space);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn write_tokens(&mut self, tokens: &[Tok], indent: usize, marker: Option<(&str, St)>) {
        let (marker_text, marker_style) = marker.unwrap_or(("", St::Normal));
        let marker_width = marker_text.width();
        let painted_marker = self.style.paint(marker_style, marker_text);

        let mut line = String::new();
        let mut line_width = 0usize;
        let mut has_word = false;
        let mut wrote_anything = false;

        let begin_line = |line: &mut String, first: bool, line_width: &mut usize| {
            line.clear();
            line.push_str(&" ".repeat(indent));
            if first {
                line.push_str(&painted_marker);
            } else {
                line.push_str(&" ".repeat(marker_width));
            }
            *line_width = indent + marker_width;
        };

        begin_line(&mut line, true, &mut line_width);

        let flush = |out: &mut String, line: &mut String| {
            out.push_str(line.trim_end());
            out.push('\n');
        };

        for token in tokens {
            match token {
                Tok::Break => {
                    flush(&mut self.out, &mut line);
                    begin_line(&mut line, false, &mut line_width);
                    has_word = false;
                }
                Tok::Word { text, style, glue } => {
                    let word_width = text.width();
                    let space = has_word && !glue;
                    let extra = usize::from(space);
                    if has_word && self.width != 0 && line_width + extra + word_width > self.width {
                        flush(&mut self.out, &mut line);
                        begin_line(&mut line, false, &mut line_width);
                        has_word = false;
                    }
                    if has_word && !glue {
                        line.push(' ');
                        line_width += 1;
                    }
                    line.push_str(&self.style.paint(*style, text));
                    line_width += word_width;
                    has_word = true;
                    wrote_anything = true;
                }
            }
        }

        if wrote_anything {
            flush(&mut self.out, &mut line);
        }
    }

    fn render_list(&mut self, list: ElementRef<'_>, indent: usize) {
        let ordered = list.value().name() == "ol";
        let mut number = 1;

        for child in list.children() {
            let Some(item) = ElementRef::wrap(child) else {
                continue;
            };
            if item.value().name() != "li" {
                continue;
            }

            let mut tokens = Vec::new();
            let mut blocks = Vec::new();
            let mut pending_space = true;
            for node in item.children() {
                if let Node::Element(element) = node.value()
                    && matches!(
                        element.name(),
                        "ul" | "ol"
                            | "pre"
                            | "p"
                            | "div"
                            | "blockquote"
                            | "table"
                            | "h1"
                            | "h2"
                            | "h3"
                            | "h4"
                            | "h5"
                            | "h6"
                    )
                {
                    blocks.push(node);
                    continue;
                }
                self.collect_inline(node, St::Normal, &mut tokens, &mut pending_space);
            }

            let marker = if ordered {
                format!("{number}. ")
            } else {
                "• ".to_string()
            };
            let marker_style = St::Dim;
            number += 1;

            if !tokens.is_empty() {
                self.write_tokens(&tokens, indent, Some((&marker, marker_style)));
            }
            for block in blocks {
                self.render_nested(block, indent + 2);
            }
        }
    }

    fn render_nested(&mut self, node: NodeRef<'_, Node>, indent: usize) {
        let width = if self.width == 0 {
            0
        } else {
            self.width.saturating_sub(indent).max(20)
        };
        let mut sub = Renderer::new(&RenderOpts {
            width,
            style: self.style,
            links: self.links,
        });
        sub.render_node(node);
        let text = sub.finish();
        for line in text.lines() {
            if line.trim().is_empty() {
                self.blank();
            } else {
                self.out.push_str(&" ".repeat(indent));
                self.out.push_str(line);
                self.out.push('\n');
            }
        }
    }

    fn render_pre(&mut self, element: ElementRef<'_>) {
        let text: String = element.text().collect();
        let text = text.strip_suffix('\n').unwrap_or(&text);
        if text.trim().is_empty() {
            return;
        }
        self.blank();
        for line in text.lines() {
            let line = line.trim_end();
            if line.is_empty() {
                self.out.push('\n');
            } else {
                self.out.push_str("  ");
                self.out.push_str(line);
                self.out.push('\n');
            }
        }
        self.blank();
    }

    fn render_quote(&mut self, element: ElementRef<'_>) {
        let mut sub = Renderer::new(&RenderOpts {
            width: if self.width == 0 {
                0
            } else {
                self.width.saturating_sub(2).max(20)
            },
            style: self.style,
            links: self.links,
        });
        sub.render_children(element);
        let text = sub.finish();
        if text.is_empty() {
            return;
        }
        self.blank();
        let bar = self.style.paint(St::Dim, "│");
        for line in text.lines() {
            self.line(&format!("{bar} {line}"));
        }
        self.blank();
    }

    fn render_table(&mut self, element: ElementRef<'_>) {
        self.blank();
        for row in element.select(row_selector()) {
            let cells: Vec<String> = row
                .select(cell_selector())
                .map(|cell| collapse(&cell.text().collect::<String>()))
                .filter(|cell| !cell.is_empty())
                .collect();
            if cells.is_empty() {
                continue;
            }
            let words: Vec<Tok> = cells
                .join("  ")
                .split_whitespace()
                .map(|piece| word(piece, St::Normal))
                .collect();
            self.write_tokens(&words, 2, None);
        }
        self.blank();
    }
}

fn restyle(tokens: &mut [Tok], style: St) {
    for token in tokens {
        if let Tok::Word { style: current, .. } = token {
            *current = style;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(html: &str) -> String {
        render_page(
            html,
            &RenderOpts {
                width: 80,
                style: Style::PLAIN,
                links: false,
            },
        )
    }

    #[test]
    fn renders_headings_paragraphs_and_lists() {
        let html = r#"
            <html><body><main id="content">
              <h2>Level Goal</h2>
              <p>Use <code>ls</code> now.</p>
              <ul><li>one</li><li>two</li></ul>
            </main></body></html>
        "#;
        let text = render(html);
        assert!(text.contains("▌ Level Goal"));
        assert!(text.contains("Use ls now."));
        assert!(text.contains("• one"));
        assert!(text.contains("• two"));
    }

    #[test]
    fn keeps_punctuation_attached_to_links() {
        let html = r#"
            <html><body><main id="content">
              <p>The page for <a href="/x">Level 1</a>. Then <strong>read</strong> it.</p>
            </main></body></html>
        "#;
        let text = render(html);
        assert!(
            text.contains("The page for Level 1. Then read it."),
            "{text}"
        );
    }

    #[test]
    fn groups_inline_content_in_divs() {
        let html = r#"
            <html><body><main id="content">
              <div>Included file: <a href="x.c">x.c</a><pre>int main;</pre></div>
            </main></body></html>
        "#;
        let text = render(html);
        assert!(text.contains("Included file: x.c"), "{text}");
        assert!(text.contains("int main;"));
    }

    #[test]
    fn extracts_sections_and_links() {
        let html = r#"
            <html><body><main id="content">
              <h1>Bandit</h1>
              <p>Intro text.</p>
              <h2>Level Goal</h2>
              <p>Find the <a href="https://example.org">password</a>.</p>
            </main></body></html>
        "#;
        let page = parse_page(html);
        assert_eq!(page.title, "Bandit");
        assert_eq!(page.sections.len(), 2);
        assert_eq!(page.sections[0].heading.as_deref(), Some("Bandit"));
        assert!(page.sections[0].text.contains("Intro text."));
        assert_eq!(page.sections[1].heading.as_deref(), Some("Level Goal"));
        assert!(page.sections[1].text.contains("Find the password."));
        assert_eq!(page.links[0].text, "password");
        assert_eq!(page.links[0].href, "https://example.org");
    }
}
