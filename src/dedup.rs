use crate::entry::UsageEntry;
use std::collections::hash_map::{DefaultHasher, Entry};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// Hash `(msg_id, req_id)` to a u64 key so the dedup map doesn't have to clone
/// every string twice per entry. SipHash collision probability for ~50k
/// entries is ~1e-10; acceptable for cost aggregation.
fn key_for(msg: &str, req: &str) -> u64 {
    let mut h = DefaultHasher::new();
    msg.hash(&mut h);
    req.hash(&mut h);
    h.finish()
}

/// Token sum used to rank duplicate snapshots of one response. Usage only
/// grows while a response streams, so the largest snapshot is the final
/// billed total.
fn usage_magnitude(e: &UsageEntry) -> u64 {
    e.message.usage.as_ref().map_or(0, |u| {
        u.input_tokens + u.output_tokens + u.cache_creation_input_tokens + u.cache_read_input_tokens
    })
}

pub fn dedup_entries(entries: Vec<UsageEntry>) -> Vec<UsageEntry> {
    let mut seen: HashMap<u64, usize> = HashMap::new();
    let mut out: Vec<UsageEntry> = Vec::with_capacity(entries.len());
    for e in entries {
        match (e.message.id.as_deref(), e.request_id.as_deref()) {
            (Some(m), Some(r)) => match seen.entry(key_for(m, r)) {
                // Claude Code (Fable-era format) appends progressive usage
                // snapshots under the same (msg, req); keep the largest, in
                // the first occurrence's position.
                Entry::Occupied(slot) => {
                    let i = *slot.get();
                    if usage_magnitude(&e) > usage_magnitude(&out[i]) {
                        out[i] = e;
                    }
                }
                Entry::Vacant(slot) => {
                    slot.insert(out.len());
                    out.push(e);
                }
            },
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

    fn entry_with_output(ts: &str, msg: &str, req: &str, out_tokens: u64) -> UsageEntry {
        let line = format!(
            r#"{{"timestamp":"{ts}","type":"assistant","requestId":"{req}","message":{{"id":"{msg}","model":"claude-haiku-4-5","usage":{{"input_tokens":1,"output_tokens":{out_tokens},"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}}}}}"#
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
    fn keeps_largest_usage_snapshot_among_duplicates() {
        // Progressive snapshots of one streaming response: output grows.
        let a = entry_with_output("2026-06-10T00:00:00Z", "msg1", "req1", 3);
        let b = entry_with_output("2026-06-10T00:00:01Z", "msg1", "req1", 4406);
        let out = dedup_entries(vec![a, b]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].message.usage.as_ref().unwrap().output_tokens, 4406);
    }

    #[test]
    fn keeps_largest_usage_snapshot_regardless_of_order() {
        // Duplicates can arrive across files in any order; the final billed
        // snapshot must win even when it is read first.
        let a = entry_with_output("2026-06-10T00:00:01Z", "msg1", "req1", 4406);
        let b = entry_with_output("2026-06-10T00:00:00Z", "msg1", "req1", 3);
        let out = dedup_entries(vec![a, b]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].message.usage.as_ref().unwrap().output_tokens, 4406);
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
