use crate::pricing::Usage;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

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

pub fn parse_file(path: &Path) -> anyhow::Result<Vec<UsageEntry>> {
    let f = File::open(path).map_err(|e| anyhow::anyhow!("open {}: {e}", path.display()))?;
    let reader = BufReader::new(f);
    let mut out = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(e) = parse_line(&line) {
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
