pub const SENTINEL_INSTRUCTION: &str =
    "Return only the result itself -- no explanations, no alternatives, no \
     labels or headings, nothing but the transformed text. Wrap it in a \
     single <output></output> block and emit nothing after the closing tag.";

const OPEN: &str = "<output>";
const CLOSE: &str = "</output>";

pub fn wrap_system_prompt(system_prompt: &str) -> String {
    format!("{system_prompt}\n\n{SENTINEL_INSTRUCTION}")
}

pub fn extract(raw: &str) -> Option<String> {
    if raw.matches(CLOSE).count() > 1 {
        return None;
    }
    let start = raw.find(OPEN)? + OPEN.len();
    let rest = &raw[start..];
    let end = rest.find(CLOSE)?;
    let inner = &rest[..end];
    let content = match inner.rfind(OPEN) {
        Some(i) => &inner[i + OPEN.len()..],
        None => inner,
    };
    let content = content.trim();
    if content.is_empty() {
        return None;
    }
    Some(content.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_the_tagged_content() {
        assert_eq!(extract("<output>Corrected.</output>").as_deref(), Some("Corrected."));
    }

    #[test]
    fn discards_preamble_before_the_tag() {
        let raw = "The corrected text:\n\n<output>Corrected.</output>";
        assert_eq!(extract(raw).as_deref(), Some("Corrected."));
    }

    #[test]
    fn discards_commentary_after_the_tag() {
        let raw = "<output>Corrected.</output>\n\nLet me know if you need anything else!";
        assert_eq!(extract(raw).as_deref(), Some("Corrected."));
    }

    #[test]
    fn discards_reasoning_markup_outside_the_tags() {
        let raw = "<think>the user wants grammar fixed</think><output>Corrected.</output>";
        assert_eq!(extract(raw).as_deref(), Some("Corrected."));
    }

    #[test]
    fn preserves_internal_newlines_and_trims_only_the_edges() {
        let raw = "<output>\nline one\nline two\n</output>";
        assert_eq!(extract(raw).as_deref(), Some("line one\nline two"));
    }

    #[test]
    fn refuses_when_there_is_no_tag() {
        assert!(extract("Corrected.").is_none());
    }

    #[test]
    fn refuses_when_the_closing_tag_is_missing() {
        assert!(extract("<output>Corrected.").is_none());
    }

    #[test]
    fn refuses_when_the_opening_tag_is_missing() {
        assert!(extract("Corrected.</output>").is_none());
    }

    #[test]
    fn refuses_when_the_content_is_only_whitespace() {
        assert!(extract("<output>   \n  </output>").is_none());
    }

    #[test]
    fn refuses_when_more_than_one_complete_block_is_present() {
        let raw = "<output>first</output> junk <output>second</output>";
        assert!(extract(raw).is_none());
    }

    #[test]
    fn wrap_appends_the_instruction_to_the_prompt() {
        let wrapped = wrap_system_prompt("Fix grammar.");
        assert!(wrapped.starts_with("Fix grammar."));
        assert!(wrapped.contains(SENTINEL_INSTRUCTION));
    }

    #[test]
    fn a_think_block_that_mentions_output_does_not_leak_into_the_result() {
        let raw = "<think>I need to wrap the result in <output> tags.</think>\n\
                   <output>This sentence has two mistakes.</output>";
        assert_eq!(extract(raw).as_deref(), Some("This sentence has two mistakes."));
    }

    #[test]
    fn a_preamble_mentioning_output_before_the_real_block_is_discarded() {
        let raw = "Here is the text in <output> tags:\n\n<output>Corrected.</output>";
        assert_eq!(extract(raw).as_deref(), Some("Corrected."));
    }

    #[test]
    fn nested_output_tags_take_the_innermost_open_before_the_first_close() {
        let raw = "<output>a<output>b</output>";
        assert_eq!(extract(raw).as_deref(), Some("b"));
    }
}
