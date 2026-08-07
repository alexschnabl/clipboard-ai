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
///
/// C1: pairing the *first* `<output>` with the first `</output>` after it
/// assumes those two tags belong to each other, which is not true whenever
/// `<output>` is merely mentioned before the real block. The sentinel
/// instruction we append to every system prompt contains the literal token
/// `<output>`, and a reasoning model restating it inside a `<think>` block
/// (or a plain model prefacing its answer with "wrapped in <output> tags:")
/// is ordinary output, not an edge case. Instead, once the first closing
/// tag is found, take the *innermost* opening tag that precedes it — i.e.
/// the one closest to the content — so an earlier mention of the literal
/// string `<output>` is treated as prose, not as the start of the block.
pub fn extract(raw: &str) -> Option<String> {
    // R2: two *complete* `<output>...</output>` blocks are genuinely
    // ambiguous. C1 above only handles an echoed opening tag inside the
    // first pair; it has no way to tell a real second block (an echoed
    // example, a retry the model narrated, a hallucinated second answer)
    // from the intended one. Silently pairing "first open with first
    // close" used to paste whichever block happened to come first --
    // including a decoy -- straight into the user's document with no
    // sign anything was wrong. This module's contract is that a missing
    // tag is a refusal, never empty text; ambiguity about *which* block
    // is the answer must fail the same way, because a retry in a rare
    // case beats silently corrupting the user's work.
    if raw.matches(CLOSE).count() > 1 {
        return None;
    }
    let start = raw.find(OPEN)? + OPEN.len();
    let rest = &raw[start..];
    let end = rest.find(CLOSE)?;
    let inner = &rest[..end];
    // An echoed or nested `<output>` means the real block starts later.
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

    // R2: this used to be `takes_the_first_block_when_several_are_present`,
    // asserting `Some("first")` -- i.e. it pinned the vulnerable behaviour
    // as intended. Two complete blocks are ambiguous (see the comment on
    // `extract`), and picking "first" is exactly how a decoy block gets
    // silently pasted into a real document. Refusal is the correct,
    // deliberate outcome here, not a regression.
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

    // C1: the sentinel instruction we append to every system prompt itself
    // contains the literal token `<output>`. A reasoning model restating
    // that instruction inside a `<think>` block is ordinary output, not an
    // edge case -- and the old `find(OPEN)` / `find(CLOSE)` pairing landed
    // on that mention, extracting the tail of the think block plus the
    // real opening tag instead of the real content.
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
