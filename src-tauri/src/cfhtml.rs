const START_MARKER: &[u8] = b"<!--StartFragment-->";
const END_MARKER: &[u8] = b"<!--EndFragment-->";

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() || from > haystack.len() - needle.len()
    {
        return None;
    }
    (from..=haystack.len() - needle.len()).find(|&i| &haystack[i..i + needle.len()] == needle)
}

fn rfind(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    (0..=haystack.len() - needle.len()).rev().find(|&i| &haystack[i..i + needle.len()] == needle)
}

struct Header {
    end: usize,
    start_fragment: Option<usize>,
    end_fragment: Option<usize>,
}

fn parse_header(data: &[u8]) -> Header {
    let mut header = Header { end: 0, start_fragment: None, end_fragment: None };
    let mut pos = 0;
    while pos < data.len() {
        let Some(newline) = data[pos..].iter().position(|&b| b == b'\n') else { break };
        let mut line = &data[pos..pos + newline];
        if let Some(stripped) = line.strip_suffix(b"\r") {
            line = stripped;
        }
        let Some(colon) = line.iter().position(|&b| b == b':') else { break };
        let key = &line[..colon];
        if key.is_empty() || !key.iter().all(|b| b.is_ascii_alphabetic()) {
            break;
        }
        let value = std::str::from_utf8(&line[colon + 1..]).ok().map(str::trim);
        let number = value.and_then(|v| v.parse::<usize>().ok());
        match key {
            b"StartFragment" => header.start_fragment = number,
            b"EndFragment" => header.end_fragment = number,
            _ => {}
        }
        pos += newline + 1;
        header.end = pos;
    }
    header
}

pub fn fragment(raw: &[u8]) -> Option<String> {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    let data = &raw[..end];
    let header = parse_header(data);
    let body_start = header.end;

    let by_markers = match (find(data, START_MARKER, body_start), rfind(data, END_MARKER)) {
        (Some(s), Some(e)) if s + START_MARKER.len() <= e => Some((s + START_MARKER.len(), e)),
        _ => None,
    };
    let by_offsets = match (header.start_fragment, header.end_fragment) {
        (Some(s), Some(e)) if s >= body_start && s <= e && e <= data.len() => Some((s, e)),
        _ => None,
    };
    let (start, stop) = by_markers.or(by_offsets).unwrap_or((body_start, data.len()));

    let text = String::from_utf8_lossy(&data[start..stop]).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

pub fn build(fragment: &str) -> Vec<u8> {
    const PREFIX: &str = "<html>\r\n<body>\r\n<!--StartFragment-->";
    const SUFFIX: &str = "<!--EndFragment-->\r\n</body>\r\n</html>";
    const HEADER_LEN: usize = "Version:0.9\r\nStartHTML:0000000000\r\nEndHTML:0000000000\r\n\
                              StartFragment:0000000000\r\nEndFragment:0000000000\r\n"
        .len();
    let start_html = HEADER_LEN;
    let start_fragment = start_html + PREFIX.len();
    let end_fragment = start_fragment + fragment.len();
    let end_html = end_fragment + SUFFIX.len();
    let header = format!(
        "Version:0.9\r\nStartHTML:{start_html:010}\r\nEndHTML:{end_html:010}\r\n\
         StartFragment:{start_fragment:010}\r\nEndFragment:{end_fragment:010}\r\n"
    );
    debug_assert_eq!(header.len(), HEADER_LEN);
    let mut out = Vec::with_capacity(end_html + 1);
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(PREFIX.as_bytes());
    out.extend_from_slice(fragment.as_bytes());
    out.extend_from_slice(SUFFIX.as_bytes());
    out.push(0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_built_payload_reads_back_to_the_same_fragment() {
        let payload = build("<p>Grüße &amp; <b>bold</b></p>");
        assert_eq!(payload.last(), Some(&0), "CF_HTML carries a trailing NUL");
        assert_eq!(fragment(&payload).as_deref(), Some("<p>Grüße &amp; <b>bold</b></p>"));
    }

    #[test]
    fn the_header_offsets_point_exactly_at_the_fragment() {
        let frag = "<i>ü</i>";
        let payload = build(frag);
        let text = String::from_utf8_lossy(&payload);
        let offset = |key: &str| -> usize {
            let at = text.find(key).unwrap() + key.len();
            text[at..at + 10].parse().unwrap()
        };
        let (s, e) = (offset("StartFragment:"), offset("EndFragment:"));
        assert_eq!(&payload[s..e], frag.as_bytes(), "offsets are bytes, not chars");
        let (hs, he) = (offset("StartHTML:"), offset("EndHTML:"));
        assert!(payload[hs..].starts_with(b"<html>"));
        assert!(payload[..he].ends_with(b"</html>"));
    }

    #[test]
    fn a_browser_payload_with_source_url_is_read_by_its_markers() {
        let raw = b"Version:0.9\r\nStartHTML:0000000170\r\nEndHTML:0000000300\r\nStartFragment:0000000200\r\nEndFragment:0000000250\r\nSourceURL:https://example.com/page\r\n<html>\r\n<body>\r\n<!--StartFragment--><span style=\"color: rgb(0,0,0)\">hello</span><!--EndFragment-->\r\n</body>\r\n</html>\0";
        assert_eq!(
            fragment(raw).as_deref(),
            Some("<span style=\"color: rgb(0,0,0)\">hello</span>"),
            "markers win over header offsets that do not line up"
        );
    }

    #[test]
    fn offsets_are_used_when_there_are_no_markers_and_bounds_are_checked() {
        let body = "<html><body><p>x</p></body></html>";
        let header = "Version:0.9\r\nStartHTML:0000000000\r\nEndHTML:0000000000\r\nStartFragment:0000000000\r\nEndFragment:0000000000\r\n";
        let start = header.len() + "<html><body>".len();
        let end = start + "<p>x</p>".len();
        let good = format!(
            "Version:0.9\r\nStartHTML:0000000000\r\nEndHTML:0000000000\r\nStartFragment:{start:010}\r\nEndFragment:{end:010}\r\n{body}"
        );
        assert_eq!(fragment(good.as_bytes()).as_deref(), Some("<p>x</p>"));

        let bogus = format!(
            "Version:0.9\r\nStartHTML:0000000000\r\nEndHTML:0000000000\r\nStartFragment:{start:010}\r\nEndFragment:{:010}\r\n{body}",
            end + 100_000
        );
        assert_eq!(
            fragment(bogus.as_bytes()).as_deref(),
            Some(body),
            "out-of-range offsets fall back to the whole body instead of reading past the buffer"
        );
    }

    #[test]
    fn garbage_after_the_nul_terminator_is_ignored() {
        let mut payload = build("<b>ok</b>");
        payload.extend_from_slice(&[0xff, 0xfe, 0x80, 0x00, 0xc3]);
        assert_eq!(fragment(&payload).as_deref(), Some("<b>ok</b>"));
    }

    #[test]
    fn an_empty_fragment_is_none() {
        assert!(fragment(&build("   ")).is_none());
        assert!(fragment(b"").is_none());
        assert!(fragment(b"Version:0.9\r\n").is_none());
    }
}
