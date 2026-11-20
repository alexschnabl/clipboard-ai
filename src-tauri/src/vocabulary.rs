use crate::config::{Replacement, Vocabulary};

pub fn keep_instruction(vocabulary: &Vocabulary) -> Option<String> {
    let mut words: Vec<&str> = Vec::new();
    for word in &vocabulary.keep {
        let word = word.trim();
        if !word.is_empty() && !words.contains(&word) {
            words.push(word);
        }
    }
    if words.is_empty() {
        return None;
    }
    let mut out = String::from(
        "Write these words and names exactly as they appear in this list, with the same \
         spelling, capitalization and punctuation. Never translate, correct or reword them:",
    );
    for word in words {
        out.push_str("\n- ");
        out.push_str(word);
    }
    Some(out)
}

struct Pair {
    from: Vec<char>,
    to: String,
    to_has_upper: bool,
}

fn pairs(replacements: &[Replacement]) -> Vec<Pair> {
    let mut out: Vec<Pair> = replacements
        .iter()
        .filter(|r| !r.from.trim().is_empty())
        .map(|r| {
            let to = r.to.trim().to_string();
            Pair {
                from: r.from.trim().chars().collect(),
                to_has_upper: to.chars().any(char::is_uppercase),
                to,
            }
        })
        .collect();
    out.sort_by(|a, b| b.from.len().cmp(&a.from.len()));
    out
}

fn fold_space(c: char) -> char {
    if c == '\u{a0}' {
        ' '
    } else {
        c
    }
}

fn same_letter(a: char, b: char) -> bool {
    let (a, b) = (fold_space(a), fold_space(b));
    a == b || a.to_lowercase().eq(b.to_lowercase())
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric()
}

fn matches_at(text: &[char], at: usize, pair: &Pair) -> bool {
    let len = pair.from.len();
    if at + len > text.len() {
        return false;
    }
    if !text[at..at + len].iter().zip(&pair.from).all(|(a, b)| same_letter(*a, *b)) {
        return false;
    }
    let starts_word = pair.from.first().copied().is_some_and(is_word);
    let ends_word = pair.from.last().copied().is_some_and(is_word);
    if starts_word && at > 0 && is_word(text[at - 1]) {
        return false;
    }
    if ends_word && text.get(at + len).copied().is_some_and(is_word) {
        return false;
    }
    true
}

fn push_replacement(out: &mut String, matched: &[char], pair: &Pair) {
    let letters = matched.iter().filter(|c| c.is_alphabetic()).count();
    let all_caps = letters > 1 && matched.iter().all(|c| !c.is_lowercase());
    if all_caps && !pair.to_has_upper {
        out.push_str(&pair.to.to_uppercase());
        return;
    }
    let matched_first = matched.first().copied().unwrap_or_default();
    let carry_capital = matched_first.is_uppercase() && !pair.to_has_upper;
    let mut chars = pair.to.chars();
    match chars.next() {
        Some(first) if carry_capital => {
            out.extend(first.to_uppercase());
            out.extend(chars);
        }
        _ => out.push_str(&pair.to),
    }
}

fn replace_run(text: &[char], pairs: &[Pair], out: &mut String) {
    let mut i = 0;
    'outer: while i < text.len() {
        for pair in pairs {
            if matches_at(text, i, pair) {
                push_replacement(out, &text[i..i + pair.from.len()], pair);
                i += pair.from.len();
                continue 'outer;
            }
        }
        out.push(text[i]);
        i += 1;
    }
}

const TEXT_ENTITIES: [(&str, char); 4] =
    [("&amp;", '&'), ("&lt;", '<'), ("&gt;", '>'), ("&nbsp;", '\u{a0}')];

fn escape_html_text(text: &str, out: &mut String) {
    for c in text.chars() {
        match TEXT_ENTITIES.iter().find(|(_, plain)| *plain == c) {
            Some((entity, _)) => out.push_str(entity),
            None => out.push(c),
        }
    }
}

fn other_entity_len(s: &str) -> Option<usize> {
    let rest = s.strip_prefix('&')?;
    let end = rest.find(';')?;
    let name = &rest[..end];
    let valid = !name.is_empty()
        && name.len() <= 32
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '#');
    valid.then_some(end + 2)
}

fn replace_html(html: &str, pairs: &[Pair]) -> String {
    let mut out = String::with_capacity(html.len());
    let mut run: Vec<char> = Vec::new();
    let flush = |run: &mut Vec<char>, out: &mut String| {
        if !run.is_empty() {
            let mut replaced = String::new();
            replace_run(run, pairs, &mut replaced);
            escape_html_text(&replaced, out);
            run.clear();
        }
    };
    let mut i = 0;
    while i < html.len() {
        let rest = &html[i..];
        if rest.starts_with('<') {
            let end = crate::richtext::tag_end(html, i).unwrap_or(html.len());
            flush(&mut run, &mut out);
            out.push_str(&html[i..end]);
            i = end;
            continue;
        }
        if let Some((entity, c)) = TEXT_ENTITIES.iter().find(|(e, _)| rest.starts_with(e)) {
            run.push(*c);
            i += entity.len();
            continue;
        }
        if let Some(len) = other_entity_len(rest) {
            flush(&mut run, &mut out);
            out.push_str(&rest[..len]);
            i += len;
            continue;
        }
        let c = rest.chars().next().unwrap_or_default();
        run.push(c);
        i += c.len_utf8();
    }
    flush(&mut run, &mut out);
    out
}

pub fn apply_replacements(text: &str, replacements: &[Replacement], text_is_html: bool) -> String {
    let pairs = pairs(replacements);
    if pairs.is_empty() {
        return text.to_string();
    }
    if text_is_html {
        replace_html(text, &pairs)
    } else {
        let chars: Vec<char> = text.chars().collect();
        let mut out = String::with_capacity(text.len());
        replace_run(&chars, &pairs, &mut out);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rep(from: &str, to: &str) -> Replacement {
        Replacement { from: from.into(), to: to.into() }
    }

    fn plain(text: &str, reps: &[Replacement]) -> String {
        apply_replacements(text, reps, false)
    }

    fn html(text: &str, reps: &[Replacement]) -> String {
        apply_replacements(text, reps, true)
    }

    #[test]
    fn no_keep_words_means_no_instruction() {
        assert_eq!(keep_instruction(&Vocabulary::default()), None);
        let blank = Vocabulary { keep: vec!["  ".into(), String::new()], replacements: vec![] };
        assert_eq!(keep_instruction(&blank), None);
    }

    #[test]
    fn keep_words_are_listed_one_per_line_trimmed_and_deduplicated() {
        let vocab = Vocabulary {
            keep: vec!["Clipboard AI".into(), " Tauri ".into(), "Clipboard AI".into(), "Q3, Q4".into()],
            replacements: vec![],
        };
        let instruction = keep_instruction(&vocab).unwrap();
        assert!(instruction.ends_with(":\n- Clipboard AI\n- Tauri\n- Q3, Q4"), "{instruction}");
    }

    #[test]
    fn replaces_whole_words_only() {
        let reps = [rep("teh", "the")];
        assert_eq!(plain("teh cat sat on teh mat", &reps), "the cat sat on the mat");
        assert_eq!(plain("tehran and stehen", &reps), "tehran and stehen");
        assert_eq!(plain("(teh)", &reps), "(the)");
    }

    #[test]
    fn matching_ignores_case_and_a_capital_carries_over_to_a_lowercase_target() {
        let reps = [rep("teh", "the")];
        assert_eq!(plain("Teh end. TEH end.", &reps), "The end. THE end.");
        assert_eq!(plain("A teh", &[rep("a", "an")]), "An teh");
    }

    #[test]
    fn a_target_with_its_own_capitals_is_written_exactly_as_given() {
        let reps = [rep("clipboard ai", "Clipboard AI"), rep("iphone", "iPhone")];
        assert_eq!(plain("I use clipboard ai on my Iphone", &reps), "I use Clipboard AI on my iPhone");
    }

    #[test]
    fn longer_sources_win_and_replacements_never_chain() {
        let reps = [rep("sus", "SUS"), rep("sus tech", "Sus-Tech"), rep("Sus-Tech", "nope")];
        assert_eq!(plain("sus tech and sus", &reps), "Sus-Tech and SUS");
    }

    #[test]
    fn sources_with_punctuation_at_the_edges_need_no_word_boundary_there() {
        let reps = [rep("->", "\u{2192}")];
        assert_eq!(plain("a->b", &reps), "a\u{2192}b");
    }

    #[test]
    fn empty_sources_are_ignored_and_an_empty_target_deletes() {
        let reps = [rep("  ", "x"), rep("um", "")];
        assert_eq!(plain("so um yes", &reps), "so  yes");
    }

    #[test]
    fn non_ascii_text_survives_and_matches() {
        let reps = [rep("strasse", "Stra\u{df}e")];
        assert_eq!(
            plain("Hauptstrasse, strasse, \u{c4}rger", &reps),
            "Hauptstrasse, Stra\u{df}e, \u{c4}rger"
        );
    }

    #[test]
    fn html_replacement_never_touches_tags_or_attributes() {
        let reps = [rep("bold", "BOLD"), rep("href", "x"), rep("example", "sample")];
        let input = r#"<p><b class="bold">bold</b> <a href="https://example.com">example</a></p>"#;
        assert_eq!(
            html(input, &reps),
            r#"<p><b class="bold">BOLD</b> <a href="https://example.com">sample</a></p>"#
        );
    }

    #[test]
    fn html_replacement_keeps_entities_intact_and_escapes_what_it_inserts() {
        let reps = [rep("amp", "AMP"), rep("at&t", "AT&T"), rep("lt", "<x>")];
        assert_eq!(
            html("<p>amp &amp; at&amp;t &lt; lt &copy;</p>", &reps),
            "<p>AMP &amp; AT&amp;T &lt; &lt;x&gt; &copy;</p>"
        );
    }

    #[test]
    fn html_replacement_leaves_image_placeholders_alone() {
        let reps = [rep("cai", "x"), rep("img", "y")];
        let input = r#"<p>img <img src="cai-img-0"> cai</p>"#;
        assert_eq!(html(input, &reps), r#"<p>y <img src="cai-img-0"> x</p>"#);
    }

    #[test]
    fn a_non_breaking_space_in_the_text_matches_a_plain_space_in_the_source() {
        let reps = [rep("sus tech", "Sus-Tech")];
        assert_eq!(html("<p>sus tech</p>", &reps), "<p>Sus-Tech</p>");
        assert_eq!(html("<p>sus&nbsp;tech</p>", &reps), "<p>Sus-Tech</p>");
        assert_eq!(plain("sus\u{a0}tech", &reps), "Sus-Tech");
    }

    #[test]
    fn no_replacements_return_the_text_unchanged() {
        assert_eq!(plain("same", &[]), "same");
        assert_eq!(html("<p>same</p>", &[]), "<p>same</p>");
    }
}
