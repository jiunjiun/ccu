use crate::entry::UsageEntry;
use crate::pricing::{cost_of, Usage};
use crate::timezone::Timezone;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct ModelTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub cost: f64,
}

#[derive(Debug, Clone, Default)]
pub struct Bucket {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub total_cost: f64,
    /// Model name → totals. BTreeMap so output is deterministic.
    pub models: BTreeMap<String, ModelTotals>,
}

impl Bucket {
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.cache_creation_tokens + self.cache_read_tokens
    }
}

fn accumulate(bucket: &mut Bucket, model: &str, usage: &Usage) {
    bucket.input_tokens += usage.input_tokens;
    bucket.output_tokens += usage.output_tokens;
    bucket.cache_creation_tokens += usage.cache_creation_input_tokens;
    bucket.cache_read_tokens += usage.cache_read_input_tokens;
    let cost = cost_of(usage, model);
    bucket.total_cost += cost;

    let m = bucket.models.entry(model.to_string()).or_default();
    m.input_tokens += usage.input_tokens;
    m.output_tokens += usage.output_tokens;
    m.cache_creation_tokens += usage.cache_creation_input_tokens;
    m.cache_read_tokens += usage.cache_read_input_tokens;
    m.cost += cost;
}

fn group_by<F>(entries: &[UsageEntry], key: F) -> BTreeMap<String, Bucket>
where
    F: Fn(&UsageEntry) -> String,
{
    let mut out: BTreeMap<String, Bucket> = BTreeMap::new();
    for e in entries {
        let Some(model) = e.message.model.as_deref() else {
            continue;
        };
        // ccusage filters "<synthetic>" (Claude Code's internal placeholder
        // messages) from aggregate output; match that behavior.
        if model == "<synthetic>" {
            continue;
        }
        let Some(usage) = e.message.usage.as_ref() else {
            continue;
        };
        let bucket = out.entry(key(e)).or_default();
        accumulate(bucket, model, usage);
    }
    out
}

pub fn group_by_day(entries: &[UsageEntry], tz: Timezone) -> BTreeMap<String, Bucket> {
    group_by(entries, |e| tz.day_key(e.timestamp))
}

pub fn group_by_month(entries: &[UsageEntry], tz: Timezone) -> BTreeMap<String, Bucket> {
    group_by(entries, |e| tz.month_key(e.timestamp))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::parse_line;

    fn e(ts: &str, model: &str, input: u64, output: u64) -> UsageEntry {
        let line = format!(
            r#"{{"timestamp":"{ts}","requestId":"r","type":"assistant","message":{{"id":"m","model":"{model}","usage":{{"input_tokens":{input},"output_tokens":{output},"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}}}}}"#
        );
        parse_line(&line).unwrap()
    }

    #[test]
    fn group_by_day_buckets_entries_by_key() {
        let tz = Timezone::parse("UTC").unwrap();
        let entries = vec![
            e("2026-04-24T01:00:00Z", "claude-opus-4-7", 1000, 2000),
            e("2026-04-24T03:00:00Z", "claude-opus-4-7", 3000, 4000),
            e("2026-04-25T01:00:00Z", "claude-opus-4-7", 500, 500),
        ];
        let g = group_by_day(&entries, tz);
        assert_eq!(g.len(), 2);
        assert_eq!(g["2026-04-24"].input_tokens, 4000);
        assert_eq!(g["2026-04-24"].output_tokens, 6000);
        assert_eq!(g["2026-04-25"].input_tokens, 500);
    }

    #[test]
    fn group_by_day_accumulates_model_totals() {
        let tz = Timezone::parse("UTC").unwrap();
        let entries = vec![
            e("2026-04-24T01:00:00Z", "claude-opus-4-7", 1_000_000, 0),
            e("2026-04-24T02:00:00Z", "claude-sonnet-4-5", 1_000_000, 0),
            e("2026-04-24T03:00:00Z", "claude-opus-4-7", 1_000_000, 0),
        ];
        let g = group_by_day(&entries, tz);
        let day = &g["2026-04-24"];
        assert_eq!(day.models.len(), 2);
        assert_eq!(day.models["claude-opus-4-7"].input_tokens, 2_000_000);
        assert_eq!(day.models["claude-sonnet-4-5"].input_tokens, 1_000_000);
        assert!((day.total_cost - (2.0 * 5.0 + 1.0 * 3.0)).abs() < 1e-9);
    }

    #[test]
    fn group_by_day_respects_timezone() {
        // 17:00 UTC on Apr 24 = 01:00 Apr 25 in Asia/Taipei
        let entries = vec![e("2026-04-24T17:00:00Z", "claude-opus-4-7", 1, 1)];
        let utc = group_by_day(&entries, Timezone::parse("UTC").unwrap());
        let tp = group_by_day(&entries, Timezone::parse("Asia/Taipei").unwrap());
        assert!(utc.contains_key("2026-04-24"));
        assert!(tp.contains_key("2026-04-25"));
    }

    #[test]
    fn group_by_day_ignores_entries_without_usage_or_model() {
        let mut a = e("2026-04-24T01:00:00Z", "claude-opus-4-7", 1, 1);
        a.message.usage = None;
        let mut b = e("2026-04-24T01:00:00Z", "claude-opus-4-7", 1, 1);
        b.message.model = None;
        let g = group_by_day(&[a, b], Timezone::parse("UTC").unwrap());
        assert!(g.is_empty());
    }

    #[test]
    fn group_by_month_buckets_by_yyyy_mm() {
        let tz = Timezone::parse("UTC").unwrap();
        let entries = vec![
            e("2026-04-01T00:00:00Z", "claude-opus-4-7", 1, 1),
            e("2026-04-30T00:00:00Z", "claude-opus-4-7", 1, 1),
            e("2026-05-01T00:00:00Z", "claude-opus-4-7", 1, 1),
        ];
        let g = group_by_month(&entries, tz);
        assert_eq!(g.len(), 2);
        assert_eq!(g["2026-04"].input_tokens, 2);
    }

    #[test]
    fn group_by_month_respects_timezone_at_month_boundary() {
        let entries = vec![e("2026-03-31T17:00:00Z", "claude-opus-4-7", 1, 1)];
        let utc = group_by_month(&entries, Timezone::parse("UTC").unwrap());
        let tp = group_by_month(&entries, Timezone::parse("Asia/Taipei").unwrap());
        assert!(utc.contains_key("2026-03"));
        assert!(tp.contains_key("2026-04"));
    }
}
