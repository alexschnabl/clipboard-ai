use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};

use crate::config::Formatting;
use crate::osops::Clip;

pub const KEEP_INSTRUCTION: &str = "The text is HTML. Keep its formatting and structure wherever the task does not require changing them: keep the elements, attributes and nesting as they are and change only the text inside. Keep every <img> element exactly as it is and in place. Answer with HTML, never with Markdown and never inside a code fence, and place that HTML inside the <output></output> block described below.";

pub const RICH_INSTRUCTION: &str = "Answer with HTML, never with Markdown and never inside a code fence, and place that HTML inside the <output></output> block described below. Use only the formatting the task calls for: paragraphs, bold, italics, lists and links, with headings or tables only when the task asks for them. If the text is HTML, keep its links and keep every <img> element exactly as it is and in place.";

const IMAGE_PLACEHOLDER_PREFIX: &str = "cai-img-";

const TAGS: &[&str] = &[
    "a", "b", "blockquote", "br", "caption", "code", "dd", "del", "div", "dl", "dt", "em", "font",
    "h1", "h2", "h3", "h4", "h5", "h6", "hr", "i", "img", "ins", "li", "mark", "ol", "p", "pre",
    "s", "small", "span", "strike", "strong", "sub", "sup", "table", "tbody", "td", "tfoot",
    "th", "thead", "tr", "u", "ul",
];

const DROPPED_WITH_CONTENT: &[&str] = &["script", "style", "head", "title", "xml", "noscript"];

const STYLE_PROPERTIES: &[&str] = &[
    "font-weight",
    "font-style",
    "text-decoration",
    "text-decoration-line",
    "text-align",
    "vertical-align",
    "white-space",
    "width",
    "border",
    "border-top",
    "border-bottom",
    "border-left",
    "border-right",
    "border-collapse",
    "padding",
];

const TYPOGRAPHY_PROPERTIES: &[&str] =
    &["color", "background-color", "background", "font-family", "font-size"];

const COLOR_FUNCTIONS: &[&str] = &["rgb(", "rgba(", "hsl(", "hsla("];

const SEMANTIC_TAGS: &[&str] = &[
    "b", "strong", "i", "em", "u", "s", "strike", "del", "ins", "mark", "a", "img", "ul", "ol",
    "li", "table", "h1", "h2", "h3", "h4", "h5", "h6", "blockquote", "hr", "sup", "sub", "font",
    "dl",
];

const BLOCK_TAGS: &[&str] = &[
    "p", "div", "li", "h1", "h2", "h3", "h4", "h5", "h6", "tr", "blockquote", "pre", "table",
    "ul", "ol", "hr", "dl", "dd", "dt",
];

fn build_sanitizer(keep_typography: bool) -> ammonia::Builder<'static> {
    let mut builder = ammonia::Builder::empty();
    builder.tags(TAGS.iter().copied().collect::<HashSet<_>>());
    builder.clean_content_tags(DROPPED_WITH_CONTENT.iter().copied().collect::<HashSet<_>>());
    let mut tag_attributes: HashMap<&'static str, HashSet<&'static str>> = HashMap::new();
    tag_attributes.insert("a", ["href", "title"].into_iter().collect());
    tag_attributes.insert("img", ["src", "alt", "width", "height", "title"].into_iter().collect());
    tag_attributes.insert("td", ["colspan", "rowspan", "width", "valign", "align"].into_iter().collect());
    tag_attributes.insert("th", ["colspan", "rowspan", "width", "valign", "align"].into_iter().collect());
    tag_attributes.insert(
        "table",
        ["border", "cellspacing", "cellpadding", "width", "align"].into_iter().collect(),
    );
    tag_attributes.insert("ol", ["start", "type"].into_iter().collect());
    tag_attributes.insert("font", ["color", "face", "size"].into_iter().collect());
    builder.tag_attributes(tag_attributes);
    builder.generic_attributes(["style", "class"].into_iter().collect());
    builder.url_schemes(["http", "https", "mailto", "file", "data", "cid"].into_iter().collect());
    builder.url_relative(ammonia::UrlRelative::PassThrough);
    builder.link_rel(None);
    builder.strip_comments(true);
    builder.attribute_filter(move |_element, attribute, value| {
        if attribute == "style" {
            filter_style(value, keep_typography).map(Cow::Owned)
        } else {
            Some(Cow::Borrowed(value))
        }
    });
    builder
}

fn style_value_is_safe(property: &str, value: &str) -> bool {
    if value.contains('\\') {
        return false;
    }
    if !value.contains('(') {
        return true;
    }
    let lowered = value.to_ascii_lowercase();
    (property == "color" || property == "background-color" || property == "background")
        && COLOR_FUNCTIONS.iter().any(|f| lowered.starts_with(f))
        && lowered.matches('(').count() == 1
}

fn filter_style(style: &str, keep_typography: bool) -> Option<String> {
    let kept: Vec<String> = style
        .split(';')
        .filter_map(|declaration| {
            let (property, value) = declaration.split_once(':')?;
            let property = property.trim().to_ascii_lowercase();
            let value = value.trim();
            let allowed = STYLE_PROPERTIES.contains(&property.as_str())
                || (keep_typography && TYPOGRAPHY_PROPERTIES.contains(&property.as_str()));
            if value.is_empty() || !allowed || !style_value_is_safe(&property, value) {
                return None;
            }
            Some(format!("{property}: {value}"))
        })
        .collect();
    if kept.is_empty() {
        None
    } else {
        Some(kept.join("; "))
    }
}

pub fn is_office_html(html: &str) -> bool {
    html.contains("Mso") || html.contains("mso-") || html.contains("urn:schemas-microsoft-com")
}

pub fn sanitize(html: &str, keep_typography: bool) -> String {
    build_sanitizer(keep_typography).clean(html).to_string()
}

fn tag_end(s: &str, open: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut quote: Option<u8> = None;
    let mut i = open + 1;
    while i < bytes.len() {
        let b = bytes[i];
        match quote {
            Some(q) => {
                if b == q {
                    quote = None;
                }
            }
            None => match b {
                b'"' | b'\'' => quote = Some(b),
                b'>' => return Some(i + 1),
                _ => {}
            },
        }
        i += 1;
    }
    None
}

fn is_tag_boundary(b: Option<u8>) -> bool {
    matches!(b, Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') | Some(b'/') | Some(b'>'))
}

fn find_tag(s: &str, name: &str, from: usize) -> Option<(usize, usize)> {
    let needle = format!("<{name}");
    let mut search = from;
    while let Some(rel) = s[search..].find(&needle) {
        let open = search + rel;
        if is_tag_boundary(s.as_bytes().get(open + needle.len()).copied()) {
            return tag_end(s, open).map(|end| (open, end));
        }
        search = open + needle.len();
    }
    None
}

fn has_tag(s: &str, name: &str) -> bool {
    find_tag(s, name, 0).is_some()
}

fn attribute_value<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let lowered = tag.to_ascii_lowercase();
    let mut search = 0;
    while let Some(rel) = lowered[search..].find(name) {
        let at = search + rel;
        let before = lowered.as_bytes().get(at.wrapping_sub(1)).copied();
        let boundary_before = at == 0 || matches!(before, Some(b' ') | Some(b'\t') | Some(b'\n'));
        let rest = &lowered[at + name.len()..];
        let rest_trimmed = rest.trim_start();
        if boundary_before && rest_trimmed.starts_with('=') {
            let value_start = tag.len() - rest_trimmed.len() + 1;
            let value = tag[value_start..].trim_start();
            let value_start = tag.len() - value.len();
            return match value.as_bytes().first() {
                Some(&q) if q == b'"' || q == b'\'' => {
                    let inner = &tag[value_start + 1..];
                    inner.find(q as char).map(|end| &inner[..end])
                }
                _ => Some(
                    value
                        .split(|c: char| c.is_whitespace() || c == '>' || c == '/')
                        .next()
                        .unwrap_or(""),
                ),
            };
        }
        search = at + name.len();
    }
    None
}

fn detach_images(html: &str) -> (String, Vec<String>) {
    let mut out = String::with_capacity(html.len());
    let mut images = Vec::new();
    let mut cursor = 0;
    while let Some((open, end)) = find_tag(html, "img", cursor) {
        out.push_str(&html[cursor..open]);
        images.push(html[open..end].to_string());
        out.push_str(&format!("<img src=\"{IMAGE_PLACEHOLDER_PREFIX}{}\">", images.len()));
        cursor = end;
    }
    out.push_str(&html[cursor..]);
    (out, images)
}

fn placeholder_index(tag: &str) -> Option<usize> {
    let src = attribute_value(tag, "src")?.trim();
    let digits = src.strip_prefix(IMAGE_PLACEHOLDER_PREFIX)?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse::<usize>().ok().filter(|n| *n >= 1)
}

fn reattach_images(html: &str, images: &[String]) -> String {
    let mut out = String::with_capacity(html.len());
    let mut cursor = 0;
    while let Some((open, end)) = find_tag(html, "img", cursor) {
        out.push_str(&html[cursor..open]);
        let tag = &html[open..end];
        let is_placeholder = attribute_value(tag, "src")
            .is_some_and(|src| src.trim().starts_with(IMAGE_PLACEHOLDER_PREFIX));
        match placeholder_index(tag).and_then(|n| images.get(n - 1)) {
            Some(original) => out.push_str(original),
            None if is_placeholder => {}
            None => out.push_str(tag),
        }
        cursor = end;
    }
    out.push_str(&html[cursor..]);
    out
}

fn is_real_tag(s: &str, open: usize) -> bool {
    let Some(end) = tag_end(s, open) else { return false };
    let inner = &s[open + 1..end - 1];
    let (closing, inner) = match inner.strip_prefix('/') {
        Some(rest) => (true, rest),
        None => (false, inner),
    };
    let name_len = inner.bytes().take_while(|b| b.is_ascii_alphanumeric()).count();
    if name_len == 0 {
        return false;
    }
    let name = inner[..name_len].to_ascii_lowercase();
    if !TAGS.contains(&name.as_str()) {
        return false;
    }
    let rest = inner[name_len..].trim();
    if closing {
        return rest.is_empty();
    }
    rest.is_empty() || rest == "/" || rest.contains('=')
}

pub fn looks_like_html(s: &str) -> bool {
    let mut i = 0;
    while let Some(rel) = s[i..].find('<') {
        let open = i + rel;
        if is_real_tag(s, open) {
            return true;
        }
        i = open + 1;
    }
    false
}

fn has_block_tag(html: &str) -> bool {
    BLOCK_TAGS.iter().any(|t| has_tag(html, t))
}

fn has_style_value(lowered: &str, property: &str, value: &str) -> bool {
    let needle = format!("{property}: {value}");
    let mut search = 0;
    while let Some(rel) = lowered[search..].find(&needle) {
        let after = lowered.as_bytes().get(search + rel + needle.len()).copied();
        if matches!(after, None | Some(b';') | Some(b'"') | Some(b'\'')) {
            return true;
        }
        search += rel + needle.len();
    }
    false
}

fn has_semantic_tag(lowered: &str, name: &str) -> bool {
    let mut from = 0;
    while let Some((open, end)) = find_tag(lowered, name, from) {
        let tag = &lowered[open..end];
        let fake_bold = (name == "b" || name == "strong") && tag.contains("font-weight: normal");
        if !fake_bold {
            return true;
        }
        from = end;
    }
    false
}

pub fn worth_keeping(raw: &str, sanitized: &str) -> bool {
    let lowered = sanitized.to_ascii_lowercase();
    if has_tag(&lowered, "pre") || has_style_value(&lowered, "white-space", "pre") {
        return false;
    }
    if raw.contains("mso-list") && !has_tag(&lowered, "li") {
        return false;
    }
    if SEMANTIC_TAGS.iter().any(|t| has_semantic_tag(&lowered, t)) {
        return true;
    }
    let style_markers = [
        "font-weight: bold",
        "font-weight: 6",
        "font-weight: 7",
        "font-weight: 8",
        "font-weight: 9",
        "font-style: italic",
        "text-decoration: underline",
        "text-decoration: line-through",
        "text-decoration-line: underline",
        "text-decoration-line: line-through",
    ];
    style_markers.iter().any(|m| lowered.contains(m))
}

pub fn markdown_to_html(markdown: &str) -> String {
    use pulldown_cmark::{html, Options, Parser};
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(markdown, options);
    let mut out = String::with_capacity(markdown.len() * 2);
    html::push_html(&mut out, parser);
    out
}

fn escape_inline(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\n' => out.push_str("<br>"),
            '\r' => {}
            _ => out.push(c),
        }
    }
    out
}

fn is_fence_line(line: &str) -> Option<&'static str> {
    let trimmed = line.trim();
    if trimmed.starts_with("```") {
        Some("```")
    } else if trimmed.starts_with("~~~") {
        Some("~~~")
    } else {
        None
    }
}

fn strip_code_fence(output: &str) -> String {
    let trimmed = output.trim();
    let mut lines = trimmed.lines();
    let Some(first) = lines.next() else { return trimmed.to_string() };
    let Some(fence) = is_fence_line(first) else { return trimmed.to_string() };
    let mut inside = Vec::new();
    for line in lines {
        if line.trim().starts_with(fence) {
            return inside.join("\n").trim().to_string();
        }
        inside.push(line);
    }
    inside.join("\n").trim().to_string()
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let semicolon = tail.find(';').filter(|i| *i <= 10);
        let Some(end) = semicolon else {
            out.push('&');
            rest = &rest[at + 1..];
            continue;
        };
        let entity = &tail[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some('\u{a0}'),
            _ => entity
                .strip_prefix('#')
                .and_then(|n| {
                    if let Some(hex) = n.strip_prefix('x').or_else(|| n.strip_prefix('X')) {
                        u32::from_str_radix(hex, 16).ok()
                    } else {
                        n.parse::<u32>().ok()
                    }
                })
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &tail[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[at + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn tag_name(tag: &str) -> (String, bool) {
    let inner = tag.trim_start_matches('<').trim_end_matches('>').trim();
    let closing = inner.starts_with('/');
    let inner = inner.trim_start_matches('/');
    let name: String = inner
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    (name, closing)
}

fn collapse_whitespace(text: &str, out: &mut String) {
    for c in text.chars() {
        if c == '\u{a0}' {
            out.push(' ');
        } else if c.is_whitespace() {
            if !out.ends_with(' ') && !out.ends_with('\n') && !out.is_empty() {
                out.push(' ');
            }
        } else {
            out.push(c);
        }
    }
}

fn push_newline(out: &mut String) {
    while out.ends_with(' ') {
        out.pop();
    }
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
}

pub fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut cursor = 0;
    let mut pre_depth: usize = 0;
    let mut list_stack: Vec<Option<usize>> = Vec::new();
    while let Some(rel) = html[cursor..].find('<') {
        let open = cursor + rel;
        let text = &html[cursor..open];
        if pre_depth > 0 {
            out.push_str(&decode_entities(text));
        } else {
            collapse_whitespace(&decode_entities(text), &mut out);
        }
        let Some(end) = tag_end(html, open) else {
            cursor = open;
            break;
        };
        let tag = &html[open..end];
        let (name, closing) = tag_name(tag);
        match (name.as_str(), closing) {
            ("br", _) => out.push('\n'),
            ("pre", false) => {
                push_newline(&mut out);
                pre_depth += 1;
            }
            ("pre", true) => {
                pre_depth = pre_depth.saturating_sub(1);
                push_newline(&mut out);
            }
            ("ul", false) => {
                push_newline(&mut out);
                list_stack.push(None);
            }
            ("ol", false) => {
                push_newline(&mut out);
                list_stack.push(Some(0));
            }
            ("ul", true) | ("ol", true) => {
                list_stack.pop();
                push_newline(&mut out);
                if list_stack.is_empty() {
                    out.push('\n');
                }
            }
            ("li", false) => {
                push_newline(&mut out);
                let depth = list_stack.len().saturating_sub(1);
                out.push_str(&"  ".repeat(depth));
                match list_stack.last_mut() {
                    Some(Some(n)) => {
                        *n += 1;
                        out.push_str(&format!("{n}. "));
                    }
                    _ => out.push_str("- "),
                }
            }
            ("td", true) | ("th", true) => out.push('\t'),
            ("p", true) | ("div", true) | ("h1", true) | ("h2", true) | ("h3", true)
            | ("h4", true) | ("h5", true) | ("h6", true) | ("blockquote", true)
            | ("table", true) => {
                push_newline(&mut out);
                if name != "div" {
                    out.push('\n');
                }
            }
            (other, _) if BLOCK_TAGS.contains(&other) => push_newline(&mut out),
            _ => {}
        }
        cursor = end;
    }
    let tail = &html[cursor..];
    if pre_depth > 0 {
        out.push_str(&decode_entities(tail));
    } else {
        collapse_whitespace(&decode_entities(tail), &mut out);
    }
    let mut lines: Vec<&str> = out.lines().map(|l| l.trim_end()).collect();
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    let mut result = String::new();
    let mut blank_run = 0;
    for line in lines {
        if line.is_empty() {
            blank_run += 1;
            if blank_run > 1 || result.is_empty() {
                continue;
            }
        } else {
            blank_run = 0;
        }
        result.push_str(line);
        result.push('\n');
    }
    result.trim_end().to_string()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Prepared {
    pub text: String,
    pub instruction: Option<&'static str>,
    images: Vec<String>,
    rich_output: bool,
    inline_input: bool,
}

fn plain(text: &str, instruction: Option<&'static str>, rich_output: bool) -> Prepared {
    Prepared {
        text: text.to_string(),
        instruction,
        images: Vec::new(),
        rich_output,
        inline_input: false,
    }
}

pub fn prepare(clip: &Clip, formatting: Formatting, backend_supports_rich: bool) -> Prepared {
    let formatting = if backend_supports_rich { formatting } else { Formatting::Plain };
    let raw_html = clip.html.as_deref().filter(|h| !h.trim().is_empty());
    match (formatting, raw_html) {
        (Formatting::Plain, _) | (Formatting::Auto, None) => plain(&clip.text, None, false),
        (Formatting::Rich, None) => plain(&clip.text, Some(RICH_INSTRUCTION), true),
        (mode, Some(raw)) => {
            let sanitized = sanitize(raw, is_office_html(raw));
            if mode == Formatting::Auto && !worth_keeping(raw, &sanitized) {
                return plain(&clip.text, None, false);
            }
            let inline_input = !has_block_tag(&sanitized);
            let (text, images) = detach_images(&sanitized);
            let instruction =
                if mode == Formatting::Rich { RICH_INSTRUCTION } else { KEEP_INSTRUCTION };
            Prepared { text, instruction: Some(instruction), images, rich_output: true, inline_input }
        }
    }
}

pub fn finish(output: &str, prepared: &Prepared) -> Clip {
    if !prepared.rich_output {
        return Clip { text: output.to_string(), html: None };
    }
    let output = strip_code_fence(output);
    if looks_like_html(&output) {
        let html = reattach_images(&sanitize(&output, true), &prepared.images);
        Clip { text: html_to_text(&html), html: Some(html) }
    } else if prepared.inline_input {
        Clip { html: Some(escape_inline(&output)), text: output }
    } else {
        let html = sanitize(&markdown_to_html(&output), true);
        Clip { text: output, html: Some(html) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(text: &str, html: Option<&str>) -> Clip {
        Clip { text: text.to_string(), html: html.map(|h| h.to_string()) }
    }

    fn live_input() -> String {
        let raw = std::fs::read(std::env::var("CAI_LIVE_INPUT_HTML").unwrap()).unwrap();
        if raw.starts_with(b"Version:") {
            crate::cfhtml::fragment(&raw).expect("a CF_HTML payload must carry a fragment")
        } else {
            String::from_utf8_lossy(&raw).into_owned()
        }
    }

    #[test]
    fn plain_formatting_ignores_html_on_the_clipboard() {
        let p = prepare(&clip("hello", Some("<b>hello</b>")), Formatting::Plain, true);
        assert_eq!(p.text, "hello");
        assert!(p.instruction.is_none());
        let out = finish("<b>Hello</b>", &p);
        assert_eq!(out, clip("<b>Hello</b>", None), "plain mode never interprets the output");
    }

    #[test]
    fn auto_with_a_plain_selection_is_exactly_the_plain_path() {
        let p = prepare(&clip("hello", None), Formatting::Auto, true);
        assert_eq!(p.text, "hello");
        assert!(p.instruction.is_none());
        assert_eq!(finish("Hello.", &p), clip("Hello.", None));
    }

    #[test]
    fn a_backend_without_rich_support_forces_plain() {
        let p = prepare(&clip("hello", Some("<b>hello</b>")), Formatting::Rich, false);
        assert_eq!(p.text, "hello");
        assert!(p.instruction.is_none());
        assert_eq!(finish("<p>x</p>", &p), clip("<p>x</p>", None));
    }

    #[test]
    fn auto_stays_plain_when_the_html_carries_nothing_worth_keeping() {
        for html in [
            "<p>just a line</p>",
            "<div>slack message</div>",
            r#"<span style="color: rgb(0,0,0); font-family: Arial; font-size: 14px">text</span>"#,
            r#"<p class="MsoNormal"><span style="font-size: 11pt; font-family: Calibri">Hallo</span></p>"#,
            r#"<b style="font-weight:normal;" id="docs-internal-guid-1">google docs wraps everything</b>"#,
        ] {
            let p = prepare(&clip("text", Some(html)), Formatting::Auto, true);
            assert!(p.instruction.is_none(), "{html} must take the plain path, got {}", p.text);
            assert_eq!(p.text, "text");
        }
    }

    #[test]
    fn auto_goes_rich_for_bold_links_lists_images_and_styled_emphasis() {
        for html in [
            "<p>a <b>bold</b> word</p>",
            r#"<p>see <a href="https://x.y">this</a></p>"#,
            "<ul><li>one</li></ul>",
            r#"<p><img src="https://x/y.png"></p>"#,
            r#"<span style="font-weight:700">docs bold</span>"#,
            r#"<span style="font-style:italic">docs italic</span>"#,
            r#"<span style="white-space: pre-wrap"><b>slack bold</b></span>"#,
            r#"<b style="font-weight:normal;" id="docs-internal-guid-1">docs <i>italic</i></b>"#,
        ] {
            let p = prepare(&clip("text", Some(html)), Formatting::Auto, true);
            assert_eq!(p.instruction, Some(KEEP_INSTRUCTION), "{html} must take the rich path");
        }
    }

    #[test]
    fn code_editor_html_and_word_lists_stay_plain_in_auto() {
        let vscode = r#"<div style="color: #d4d4d4;background-color: #1e1e1e;font-family: Consolas;font-size: 14px;line-height: 19px;white-space: pre;"><div><span style="color: #569cd6;">let</span> <b>x</b></div></div>"#;
        let p = prepare(&clip("let x", Some(vscode)), Formatting::Auto, true);
        assert!(p.instruction.is_none(), "code must stay plain, got {}", p.text);

        let word_list = r#"<p class=MsoListParagraph style='text-indent:-18.0pt;mso-list:l0 level1 lfo1'><span style='font-family:Symbol'>·<span style='font:7.0pt "Times New Roman"'>&nbsp;&nbsp;</span></span><b>Item</b><o:p></o:p></p>"#;
        let p = prepare(&clip("· Item", Some(word_list)), Formatting::Auto, true);
        assert!(p.instruction.is_none(), "Word lists must stay plain, got {}", p.text);

        let pre = "<p><b>x</b></p><pre>code</pre>";
        let p = prepare(&clip("x code", Some(pre)), Formatting::Auto, true);
        assert!(p.instruction.is_none(), "pre blocks keep the selection plain");
    }

    #[test]
    fn office_html_keeps_fonts_but_browser_html_drops_computed_style_noise() {
        let office = r#"<p class=MsoNormal><span style='font-size:11.0pt;font-family:"Calibri",sans-serif;mso-fareast-font-family:"Times New Roman"'>Helo <b>world</b><o:p></o:p></span></p>"#;
        let p = prepare(&clip("Helo world", Some(office)), Formatting::Auto, true);
        assert_eq!(p.instruction, Some(KEEP_INSTRUCTION));
        assert!(p.text.contains("<b>world</b>"), "got: {}", p.text);
        assert!(!p.text.contains("mso-"), "office cruft must be stripped: {}", p.text);
        assert!(!p.text.contains("<o:p>"), "unknown tags must be stripped: {}", p.text);
        assert!(p.text.contains("font-family"), "office fonts are kept: {}", p.text);
        assert!(p.text.contains("MsoNormal"), "the class is kept for Word/Outlook: {}", p.text);

        let chrome = r#"<span style="color: rgb(32,33,36); font-family: Roboto; font-size: 14px; background-color: rgb(255,255,255)">a <b>b</b></span>"#;
        let p = prepare(&clip("a b", Some(chrome)), Formatting::Auto, true);
        assert_eq!(p.text, "<span>a <b>b</b></span>", "computed styles are noise: {}", p.text);
    }

    #[test]
    fn images_are_replaced_by_placeholders_and_restored_after_the_model() {
        let html = r#"<p>Before <img src="data:image/png;base64,AAAA" width="10" height="10"> after <img src="file:///C:/tmp/clip_image001.png"></p>"#;
        let p = prepare(&clip("Before after", Some(html)), Formatting::Auto, true);
        assert!(!p.text.contains("base64"), "data URIs never reach the model: {}", p.text);
        assert!(p.text.contains(r#"<img src="cai-img-1">"#), "got: {}", p.text);
        assert!(p.text.contains(r#"<img src="cai-img-2">"#), "got: {}", p.text);

        let model_output = r#"<p>Before, <img src="cai-img-1" alt=""> after. <img src="cai-img-2"/></p>"#;
        let out = finish(model_output, &p);
        let html_out = out.html.expect("rich output");
        assert!(html_out.contains("data:image/png;base64,AAAA"), "got: {html_out}");
        assert!(html_out.contains("file:///C:/tmp/clip_image001.png"), "got: {html_out}");
        assert!(!html_out.contains("cai-img"), "no placeholder may leak: {html_out}");
        assert_eq!(out.text, "Before, after.");
    }

    #[test]
    fn invented_placeholders_are_dropped_and_real_images_from_the_model_are_kept() {
        let html = r#"<p>a <img src="https://x/1.png"> b <img src="https://x/2.png"></p>"#;
        let p = prepare(&clip("a b", Some(html)), Formatting::Auto, true);
        let out = finish(
            r#"<p>a <img src="cai-img-0"> b <img src="cai-img-99"> c <img src="cai-img-2"> d <img src="https://cdn/cai-img-3.png"> e <img alt="x" src='cai-img-1'></p>"#,
            &p,
        );
        let html_out = out.html.unwrap();
        assert!(!html_out.contains("cai-img-0"), "got: {html_out}");
        assert!(!html_out.contains("cai-img-99"), "got: {html_out}");
        assert!(html_out.contains(r#"c <img src="https://x/2.png"> d"#), "got: {html_out}");
        assert!(html_out.contains("https://cdn/cai-img-3.png"), "a real src is kept: {html_out}");
        assert!(html_out.ends_with(r#"e <img src="https://x/1.png"></p>"#), "got: {html_out}");
    }

    #[test]
    fn a_dropped_placeholder_just_loses_that_image() {
        let html = r#"<p>a <img src="https://x/y.png"> b</p>"#;
        let p = prepare(&clip("a b", Some(html)), Formatting::Auto, true);
        let out = finish("<p>a b</p>", &p);
        assert_eq!(out.html.as_deref(), Some("<p>a b</p>"));
    }

    #[test]
    fn word_conditional_comments_vanish_but_the_fallback_img_survives() {
        let html = r#"<p><b>x</b><!--[if gte vml 1]><v:shape id="x"><v:imagedata src="file:///C:/a.png"/></v:shape><![endif]--><![if !vml]><img width=100 height=50 src="file:///C:/b.png" v:shapes="x"><![endif]></p>"#;
        let p = prepare(&clip("x", Some(html)), Formatting::Auto, true);
        assert!(!p.text.contains("vml"), "got: {}", p.text);
        assert_eq!(p.images.len(), 1, "got: {}", p.text);
        assert!(p.images[0].contains("file:///C:/b.png"));
        assert!(!p.images[0].contains("v:shapes"));
    }

    #[test]
    fn scripts_styles_heads_and_event_handlers_never_survive() {
        let html = r#"<head><title>Page title</title><style>p{color:red}</style></head><p onclick="x()" style="color: red; mso-bidi: x"><b>t</b></p><script>alert(1)</script>"#;
        let p = prepare(&clip("t", Some(html)), Formatting::Auto, true);
        assert!(!p.text.contains("script"), "got: {}", p.text);
        assert!(!p.text.contains("alert"), "got: {}", p.text);
        assert!(!p.text.contains("onclick"), "got: {}", p.text);
        assert!(!p.text.contains("Page title"), "title text must not leak: {}", p.text);
        assert!(!p.text.contains("color:red}"), "style blocks go: {}", p.text);
        assert!(p.text.contains("color: red"), "inline colour stays for office html: {}", p.text);
        assert!(!p.text.contains("mso-bidi"), "got: {}", p.text);
    }

    #[test]
    fn the_model_output_is_sanitized_too() {
        let p = prepare(&clip("x", Some("<p><b>x</b></p>")), Formatting::Auto, true);
        let out = finish(
            r#"<p onmouseover="evil()">y</p><script>z</script><a href="javascript:alert(1)">l</a><span style="background: u\72l(http://x)">s</span>"#,
            &p,
        );
        let html = out.html.unwrap();
        assert!(!html.contains("onmouseover"), "got: {html}");
        assert!(!html.contains("script"), "got: {html}");
        assert!(!html.contains("javascript:"), "got: {html}");
        assert!(!html.contains("72l"), "css escapes never survive: {html}");
        assert_eq!(out.text, "y\n\nls");
    }

    #[test]
    fn a_code_fence_around_the_answer_is_stripped_even_with_trailing_chatter() {
        let p = prepare(&clip("x", Some("<p><b>x</b></p>")), Formatting::Auto, true);
        let out = finish("```html\n<p><b>y</b></p>\n```\nDone.", &p);
        assert_eq!(out.html.as_deref(), Some("<p><b>y</b></p>"));
        assert_eq!(out.text, "y");
        let out = finish("~~~\n<p><b>z</b></p>\n~~~", &p);
        assert_eq!(out.html.as_deref(), Some("<p><b>z</b></p>"));
    }

    #[test]
    fn rich_with_a_plain_selection_asks_for_html_and_accepts_markdown_back() {
        let p = prepare(&clip("some notes", None), Formatting::Rich, true);
        assert_eq!(p.text, "some notes");
        assert_eq!(p.instruction, Some(RICH_INSTRUCTION));
        let out = finish("# Title\n\n- one\n- two", &p);
        let html = out.html.expect("markdown becomes html");
        assert!(html.contains("<h1>Title</h1>"), "got: {html}");
        assert!(html.contains("<li>one</li>"), "got: {html}");
        assert_eq!(out.text, "# Title\n\n- one\n- two", "a plain target gets the markdown itself");
    }

    #[test]
    fn rich_uses_html_input_even_when_auto_would_not() {
        let p = prepare(&clip("line", Some("<p>line</p>")), Formatting::Rich, true);
        assert_eq!(p.instruction, Some(RICH_INSTRUCTION));
        assert_eq!(p.text, "<p>line</p>");
    }

    #[test]
    fn rich_with_html_output_produces_both_formats() {
        let p = prepare(&clip("some notes", None), Formatting::Rich, true);
        let out = finish("<h2>Title</h2><ul><li>one</li><li>two</li></ul>", &p);
        assert_eq!(out.html.as_deref(), Some("<h2>Title</h2><ul><li>one</li><li>two</li></ul>"));
        assert_eq!(out.text, "Title\n\n- one\n- two");
    }

    #[test]
    fn a_tagless_answer_to_a_block_selection_is_pasted_as_a_paragraph() {
        let p = prepare(&clip("a b", Some("<p><b>a</b></p><p>b</p>")), Formatting::Auto, true);
        let out = finish("Just one line.", &p);
        assert_eq!(out.html.as_deref(), Some("<p>Just one line.</p>\n"));
        assert_eq!(out.text, "Just one line.");
    }

    #[test]
    fn a_tagless_answer_to_an_inline_selection_stays_inline_and_keeps_its_angle_brackets() {
        let p = prepare(&clip("a b", Some("some <b>bold</b> words")), Formatting::Auto, true);
        assert!(p.inline_input);
        let out = finish("compare a<b and c>d, mail <john@example.com>\nmore", &p);
        assert_eq!(
            out.html.as_deref(),
            Some("compare a&lt;b and c&gt;d, mail &lt;john@example.com&gt;<br>more")
        );
        assert_eq!(out.text, "compare a<b and c>d, mail <john@example.com>\nmore");
    }

    #[test]
    fn looks_like_html_needs_a_real_tag_from_the_allowed_subset() {
        assert!(looks_like_html("<p>x</p>"));
        assert!(looks_like_html("a <br> b"));
        assert!(looks_like_html("a <br/> b"));
        assert!(looks_like_html(r#"<a href="x">l</a>"#));
        assert!(looks_like_html("x <B>y</B>"));
        assert!(!looks_like_html("a < b and c > d"));
        assert!(!looks_like_html("compare a<b and c>d then"));
        assert!(!looks_like_html("Contact <john@example.com> now"));
        assert!(!looks_like_html("plain text"));
        assert!(!looks_like_html("x <3 y"));
        assert!(!looks_like_html("<custom>tag</custom>"));
    }

    #[test]
    fn html_to_text_handles_blocks_lists_breaks_and_entities() {
        let html = "<p>First &amp; <b>bold</b></p><p>Second<br>line</p><ul><li>one</li><li>two &lt;x&gt;</li></ul><ol><li>uno</li><li>due</li></ol>";
        assert_eq!(
            html_to_text(html),
            "First & bold\n\nSecond\nline\n\n- one\n- two <x>\n\n1. uno\n2. due"
        );
    }

    #[test]
    fn html_to_text_collapses_source_newlines_except_inside_pre() {
        let html = "<p>Word wraps\nits HTML   source\nmid paragraph.</p><pre>line 1\n  line 2</pre>";
        assert_eq!(html_to_text(html), "Word wraps its HTML source mid paragraph.\n\nline 1\n  line 2");
    }

    #[test]
    fn html_to_text_keeps_table_cells_apart_and_turns_nbsp_into_spaces() {
        let html = "<table><tr><td>a&nbsp;1</td><td>b</td></tr><tr><td>c</td><td>d</td></tr></table>";
        assert_eq!(html_to_text(html), "a 1\tb\nc\td");
    }

    #[test]
    fn style_filter_keeps_only_known_safe_properties() {
        assert_eq!(
            filter_style("color: red; mso-ansi-language: EN-GB; font-size: 11pt; margin: 0", true),
            Some("color: red; font-size: 11pt".to_string())
        );
        assert_eq!(
            filter_style("color: red; font-weight: bold; font-size: 11pt", false),
            Some("font-weight: bold".to_string())
        );
        assert_eq!(
            filter_style("color: rgb(1, 2, 3); background-color: RGBA(0,0,0,0.5)", true),
            Some("color: rgb(1, 2, 3); background-color: RGBA(0,0,0,0.5)".to_string())
        );
        assert_eq!(filter_style("mso-x: y", true), None);
        assert_eq!(filter_style("background: url(http://x)", true), None);
        assert_eq!(filter_style(r"background: u\72l(http://x)", true), None);
        assert_eq!(filter_style("color: expression(alert(1))", true), None);
        assert_eq!(filter_style("width: calc(100% - 1px)", true), None);
    }

    #[test]
    fn links_keep_their_href_without_an_injected_rel() {
        let p = prepare(
            &clip("x", Some(r#"<a href="https://example.com" onclick="x">x</a>"#)),
            Formatting::Auto,
            true,
        );
        assert_eq!(p.text, r#"<a href="https://example.com">x</a>"#);
    }

    #[test]
    fn img_tags_with_attribute_values_containing_a_closing_bracket_are_scanned_whole() {
        let html = r#"<img src="https://x/y?a=b>c" alt="q"> tail"#;
        let (out, images) = detach_images(html);
        assert_eq!(images.len(), 1);
        assert_eq!(images[0], r#"<img src="https://x/y?a=b>c" alt="q">"#);
        assert_eq!(out, r#"<img src="cai-img-1"> tail"#);
    }

    #[test]
    fn attribute_values_are_read_whether_quoted_or_not() {
        assert_eq!(attribute_value(r#"<img src="a b" alt=c>"#, "src"), Some("a b"));
        assert_eq!(attribute_value(r#"<img src="a b" alt=c>"#, "alt"), Some("c"));
        assert_eq!(attribute_value("<img SRC='x'/>", "src"), Some("x"));
        assert_eq!(attribute_value(r#"<img data-src="y">"#, "src"), None);
        assert_eq!(attribute_value("<img>", "src"), None);
    }

    #[test]
    #[ignore]
    fn live_model_step_one_writes_the_prompt_and_text_to_send() {
        let input = live_input();
        let base_prompt = std::fs::read_to_string(std::env::var("CAI_LIVE_BASE_PROMPT").unwrap()).unwrap();
        let out_dir = std::path::PathBuf::from(std::env::var("CAI_LIVE_DIR").unwrap());
        let p = prepare(&clip("", Some(&input)), Formatting::Auto, true);
        let instruction = p.instruction.expect("the sample must take the rich path");
        let prompt = crate::sentinel::wrap_system_prompt(&format!("{base_prompt}

{instruction}"));
        std::fs::write(out_dir.join("live-system-prompt.txt"), prompt).unwrap();
        std::fs::write(out_dir.join("live-model-input.html"), &p.text).unwrap();
        println!("MODEL SEES:
{}", p.text);
    }

    #[test]
    #[ignore]
    fn live_model_step_two_runs_the_answer_through_finish() {
        let input = live_input();
        let dir = std::path::PathBuf::from(std::env::var("CAI_LIVE_DIR").unwrap());
        let answer = std::fs::read_to_string(dir.join("live-model-answer.txt")).unwrap();
        let p = prepare(&clip("", Some(&input)), Formatting::Auto, true);
        let extracted = crate::sentinel::extract(&answer).expect("the answer must sit inside <output> tags");
        let out = finish(&extracted, &p);
        let html = out.html.clone().expect("rich output");
        println!("PASTED HTML:
{html}

PASTED TEXT:
{}", out.text);
        assert!(!html.contains("cai-img"), "no placeholder may leak");
        for img in &p.images {
            assert!(html.contains(img.as_str()), "image must be back in place: {img}");
        }
        std::fs::write(dir.join("live-pasted.html"), &html).unwrap();
        std::fs::write(dir.join("live-pasted.txt"), &out.text).unwrap();
    }

    #[test]
    #[ignore]
    fn live_model_openai_compatible_backend_round_trip() {
        use crate::backend::Backend;
        let input = live_input();
        let base_prompt = std::fs::read_to_string(std::env::var("CAI_LIVE_BASE_PROMPT").unwrap()).unwrap();
        let dir = std::path::PathBuf::from(std::env::var("CAI_LIVE_DIR").unwrap());
        let config = crate::config::BackendConfig {
            id: "live".into(),
            name: "live".into(),
            kind: crate::config::KIND_OPENAI.into(),
            base_url: Some(std::env::var("CAI_LIVE_OPENAI_URL").unwrap()),
            model: Some(std::env::var("CAI_LIVE_OPENAI_MODEL").unwrap()),
            api_key: None,
            timeout_ms: 900_000,
            effort: crate::config::Effort::Low,
            expects_response: true,
        };
        let backend = crate::backend::openai::OpenAiCompatible::new(&config, crate::backend::shared_http());
        let p = prepare(&clip("", Some(&input)), Formatting::Auto, true);
        let instruction = p.instruction.expect("the sample must take the rich path");
        let prompt = crate::sentinel::wrap_system_prompt(&format!("{base_prompt}

{instruction}"));
        let started = std::time::Instant::now();
        let raw = backend.transform(&prompt, &p.text).expect("the backend must answer");
        println!("MODEL {} ANSWERED IN {:?}:
{raw}
", config.model.as_deref().unwrap_or(""), started.elapsed());
        std::fs::write(dir.join("live-local-answer.txt"), &raw).unwrap();
        let extracted = crate::sentinel::extract(&raw).expect("the answer must sit inside <output> tags");
        let out = finish(&extracted, &p);
        let html = out.html.clone().expect("rich output");
        println!("PASTED HTML:
{html}

PASTED TEXT:
{}", out.text);
        assert!(!html.contains("cai-img"), "no placeholder may leak");
        for img in &p.images {
            assert!(html.contains(img.as_str()), "image must be back in place: {img}");
        }
        std::fs::write(dir.join("live-local-pasted.html"), &html).unwrap();
    }

    #[test]
    fn outlook_lists_with_real_li_tags_go_rich_despite_mso_list_styles() {
        let outlook = r#"<ul type=disc><li class=MsoNormal style='color:#222222;mso-list:l0 level1 lfo1;tab-stops:list 36.0pt'><span lang=DE style='font-size:11.5pt;font-family:"Calibri",sans-serif'>gilt der <b>rabatt</b><o:p></o:p></span></li></ul>"#;
        let p = prepare(&clip("gilt der rabatt", Some(outlook)), Formatting::Auto, true);
        assert_eq!(p.instruction, Some(KEEP_INSTRUCTION), "got: {}", p.text);
        assert!(p.text.starts_with("<ul>"), "got: {}", p.text);
        assert!(p.text.contains("<li class=\"MsoNormal\""), "got: {}", p.text);
        assert!(!p.text.contains("mso-list"), "got: {}", p.text);
    }

    #[test]
    #[ignore]
    fn live_clipboard_write_pasted_result() {
        let dir = std::path::PathBuf::from(std::env::var("CAI_LIVE_DIR").unwrap());
        let html = std::fs::read_to_string(dir.join("live-pasted.html")).unwrap();
        let text = std::fs::read_to_string(dir.join("live-pasted.txt")).unwrap();
        let ops = crate::osops::WindowsOps::new().expect("os layer");
        crate::osops::OsOps::write_clipboard(&ops, &Clip { text, html: Some(html) }).expect("write");
        println!("the clipboard now holds the pasted result as text plus CF_HTML");
    }

    #[test]
    #[ignore]
    fn live_clipboard_read_to_files() {
        let dir = std::path::PathBuf::from(std::env::var("CAI_LIVE_DIR").unwrap());
        let ops = crate::osops::WindowsOps::new().expect("os layer");
        let captured = crate::osops::OsOps::read_clipboard(&ops).expect("clipboard text");
        std::fs::write(dir.join("live-captured.txt"), &captured.text).unwrap();
        match &captured.html {
            Some(html) => {
                std::fs::write(dir.join("live-captured.html"), html).unwrap();
                println!("captured {} chars of text and {} chars of HTML", captured.text.chars().count(), html.chars().count());
            }
            None => println!("captured {} chars of text and no HTML", captured.text.chars().count()),
        }
    }
}
