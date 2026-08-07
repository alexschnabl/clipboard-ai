pub const SENTINEL_INSTRUCTION: &str =
    "Return only the result, wrapped in <output></output> tags. \
     Emit nothing after the closing tag.";

const OPEN: &str = "<output>";
const CLOSE: &str = "</output>";

pub fn wrap_system_prompt(system_prompt: &str) -> String {
    format!("{system_prompt}\n\n{SENTINEL_INSTRUCTION}")
}

/// Extract the first `<output>…</output>` block.
///
/// Returns `None` when the tags are absent, unbalanced, or wrap only
/// whitespace. Callers must treat `None` as a refusal and paste nothing —
/// never as empty text.
pub fn extract(raw: &str) -> Option<String> {
    let start = raw.find(OPEN)? + OPEN.len();
    let rest = &raw[start..];
    let end = rest.find(CLOSE)?;
    let content = rest[..end].trim();
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
    fn takes_the_first_block_when_several_are_present() {
        let raw = "<output>first</output> junk <output>second</output>";
        assert_eq!(extract(raw).as_deref(), Some("first"));
    }

    #[test]
    fn wrap_appends_the_instruction_to_the_prompt() {
        let wrapped = wrap_system_prompt("Fix grammar.");
        assert!(wrapped.starts_with("Fix grammar."));
        assert!(wrapped.contains(SENTINEL_INSTRUCTION));
    }
}
