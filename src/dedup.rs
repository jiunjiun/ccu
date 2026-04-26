use crate::entry::UsageEntry;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

/// Hash `(msg_id, req_id)` to a u64 key so the dedup set doesn't have to clone
/// every string twice per entry. SipHash collision probability for ~50k
/// entries is ~1e-10; acceptable for cost aggregation.
fn key_for(msg: &str, req: &str) -> u64 {
    let mut h = DefaultHasher::new();
    msg.hash(&mut h);
    req.hash(&mut h);
    h.finish()
}

pub fn dedup_entries(entries: Vec<UsageEntry>) -> Vec<UsageEntry> {
    let mut seen: HashSet<u64> = HashSet::new();
    let mut out = Vec::with_capacity(entries.len());
    for e in entries {
        match (e.message.id.as_deref(), e.request_id.as_deref()) {
            (Some(m), Some(r)) => {
                if seen.insert(key_for(m, r)) {
                    out.push(e);
                }
            }
            _ => out.push(e),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::parse_line;

    fn entry(ts: &str, msg: Option<&str>, req: Option<&str>) -> UsageEntry {
        let req_json = req
            .map(|r| format!(r#","requestId":"{r}""#))
            .unwrap_or_default();
        let msg_json = msg
            .map(|m| format!(r#""id":"{m}","model":"claude-haiku-4-5","usage":{{"input_tokens":1,"output_tokens":1,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}"#))
            .unwrap_or_else(|| r#""model":"claude-haiku-4-5","usage":{"input_tokens":1,"output_tokens":1,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}"#.to_string());
        let line = format!(
            r#"{{"timestamp":"{ts}","type":"assistant"{req_json},"message":{{{msg_json}}}}}"#
        );
        parse_line(&line).expect("fixture parses")
    }

    #[test]
    fn collapses_duplicate_msg_and_req() {
        let a = entry("2026-04-24T00:00:00Z", Some("msg1"), Some("req1"));
        let b = entry("2026-04-24T00:00:01Z", Some("msg1"), Some("req1"));
        let out = dedup_entries(vec![a, b]);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn keeps_same_msg_with_different_reqs() {
        let a = entry("2026-04-24T00:00:00Z", Some("msg1"), Some("req1"));
        let b = entry("2026-04-24T00:00:01Z", Some("msg1"), Some("req2"));
        assert_eq!(dedup_entries(vec![a, b]).len(), 2);
    }

    #[test]
    fn keeps_entries_with_null_request_id() {
        let a = entry("2026-04-24T00:00:00Z", Some("msg1"), None);
        let b = entry("2026-04-24T00:00:01Z", Some("msg1"), None);
        assert_eq!(dedup_entries(vec![a, b]).len(), 2);
    }

    #[test]
    fn keeps_entries_with_null_message_id() {
        let a = entry("2026-04-24T00:00:00Z", None, Some("req1"));
        let b = entry("2026-04-24T00:00:01Z", None, Some("req1"));
        assert_eq!(dedup_entries(vec![a, b]).len(), 2);
    }

    #[test]
    fn preserves_insertion_order_of_kept_entries() {
        let a = entry("2026-04-24T00:00:00Z", Some("msg1"), Some("req1"));
        let b = entry("2026-04-24T00:00:01Z", Some("msg2"), Some("req2"));
        let c = entry("2026-04-24T00:00:02Z", Some("msg1"), Some("req1")); // dup of a
        let out = dedup_entries(vec![a, b, c]);
        let reqs: Vec<_> = out.iter().map(|e| e.request_id.clone().unwrap()).collect();
        assert_eq!(reqs, vec!["req1", "req2"]);
    }
}
