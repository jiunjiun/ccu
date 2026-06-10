use crate::pricing::Usage;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Initial line-buffer capacity. Real Claude JSONL lines vary wildly because
/// the `content` array can hold large tool blocks; 8 KiB starts us above the
/// median without forcing growth on small lines.
const LINE_BUF_CAPACITY: usize = 8 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageEntry {
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub request_id: Option<String>,
    pub message: MessageFields,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageFields {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

pub fn parse_line(line: &str) -> Option<UsageEntry> {
    serde_json::from_str::<UsageEntry>(line).ok()
}

fn strip_trailing_newline(buf: &[u8]) -> &[u8] {
    let buf = buf.strip_suffix(b"\n").unwrap_or(buf);
    buf.strip_suffix(b"\r").unwrap_or(buf)
}

pub fn parse_file(path: &Path) -> anyhow::Result<Vec<UsageEntry>> {
    let f = File::open(path).map_err(|e| anyhow::anyhow!("open {}: {e}", path.display()))?;
    let mut reader = BufReader::new(f);
    let mut out = Vec::new();
    // Reuse one buffer across all lines so `parse_file` doesn't allocate
    // per-line. `from_slice` lets serde validate UTF-8 lazily inside the
    // tokenizer instead of paying for `BufReader::lines()`'s upfront sweep.
    let mut buf: Vec<u8> = Vec::with_capacity(LINE_BUF_CAPACITY);
    loop {
        buf.clear();
        let n = reader.read_until(b'\n', &mut buf)?;
        if n == 0 {
            break;
        }
        let line = strip_trailing_newline(&buf);
        if line.is_empty() {
            continue;
        }
        if let Ok(e) = serde_json::from_slice::<UsageEntry>(line) {
            out.push(e);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"timestamp":"2026-04-24T03:00:00.000Z","requestId":"req_A","type":"assistant","message":{"id":"msg_A","model":"claude-opus-4-7","usage":{"input_tokens":340,"output_tokens":84592,"cache_creation_input_tokens":303910,"cache_read_input_tokens":9087615}},"extra":"ignored"}"#;

    #[test]
    fn parses_realistic_assistant_entry() {
        let e = parse_line(SAMPLE).expect("should parse");
        assert_eq!(e.request_id.as_deref(), Some("req_A"));
        assert_eq!(e.message.id.as_deref(), Some("msg_A"));
        assert_eq!(e.message.model.as_deref(), Some("claude-opus-4-7"));
        let u = e.message.usage.expect("usage");
        assert_eq!(u.input_tokens, 340);
        assert_eq!(u.output_tokens, 84592);
        assert_eq!(u.cache_creation_input_tokens, 303910);
        assert_eq!(u.cache_read_input_tokens, 9_087_615);
    }

    #[test]
    fn ignores_unknown_top_level_fields() {
        // SAMPLE already includes an `extra` field; success proves we ignore unknowns.
        assert!(parse_line(SAMPLE).is_some());
    }

    #[test]
    fn handles_entry_without_request_id() {
        let line = r#"{"timestamp":"2026-04-24T03:00:00Z","type":"user","message":{"id":"msg_B","model":"claude-sonnet-4-5","usage":{"input_tokens":1,"output_tokens":2,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}}"#;
        let e = parse_line(line).expect("parse");
        assert!(e.request_id.is_none());
    }

    #[test]
    fn parses_cache_creation_breakdown_from_fable_entry() {
        // Shape of a real claude-fable-5 log line: usage carries a
        // cache_creation breakdown plus other new fields we ignore.
        let line = r#"{"timestamp":"2026-06-10T03:00:00Z","requestId":"req_F","type":"assistant","message":{"id":"msg_F","model":"claude-fable-5","usage":{"input_tokens":4962,"cache_creation_input_tokens":6387,"cache_read_input_tokens":16602,"output_tokens":2239,"service_tier":"standard","cache_creation":{"ephemeral_1h_input_tokens":6387,"ephemeral_5m_input_tokens":0},"inference_geo":"not_available","speed":"standard"}}}"#;
        let e = parse_line(line).expect("parse");
        let u = e.message.usage.expect("usage");
        let cc = u.cache_creation.expect("cache_creation breakdown");
        assert_eq!(cc.ephemeral_5m_input_tokens, 0);
        assert_eq!(cc.ephemeral_1h_input_tokens, 6387);
    }

    #[test]
    fn usage_without_breakdown_has_no_cache_creation() {
        let e = parse_line(SAMPLE).expect("parse");
        assert!(e.message.usage.expect("usage").cache_creation.is_none());
    }

    #[test]
    fn handles_entry_without_usage() {
        let line = r#"{"timestamp":"2026-04-24T03:00:00Z","type":"user","message":{"id":"msg_C","model":"claude-sonnet-4-5"}}"#;
        let e = parse_line(line).expect("parse");
        assert!(e.message.usage.is_none());
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(parse_line("not json").is_none());
        assert!(parse_line("{").is_none());
    }

    #[test]
    fn rejects_entry_missing_timestamp() {
        let line = r#"{"message":{"id":"x"}}"#;
        assert!(parse_line(line).is_none());
    }

    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_tmp(contents: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn parse_file_reads_all_valid_lines() {
        let body = format!("{SAMPLE}\n{SAMPLE}\n");
        let f = write_tmp(&body);
        let entries = parse_file(f.path()).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn parse_file_skips_blank_and_malformed_lines() {
        let body = format!("\n{SAMPLE}\nnot json\n{{\n{SAMPLE}\n");
        let f = write_tmp(&body);
        let entries = parse_file(f.path()).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn parse_file_errors_when_path_missing() {
        let err = parse_file(std::path::Path::new("/tmp/does-not-exist-ccu")).unwrap_err();
        assert!(
            err.to_string().contains("does-not-exist-ccu")
                || err.to_string().contains("No such file")
        );
    }
}
