use crate::aggregate::{Bucket, ModelTotals};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DailyRow<'a> {
    date: &'a str,
    input_tokens: u64,
    output_tokens: u64,
    cache_creation_tokens: u64,
    cache_read_tokens: u64,
    total_tokens: u64,
    total_cost: f64,
    models_used: Vec<&'a str>,
    model_breakdowns: Vec<ModelBreakdown<'a>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MonthlyRow<'a> {
    month: &'a str,
    input_tokens: u64,
    output_tokens: u64,
    cache_creation_tokens: u64,
    cache_read_tokens: u64,
    total_tokens: u64,
    total_cost: f64,
    models_used: Vec<&'a str>,
    model_breakdowns: Vec<ModelBreakdown<'a>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelBreakdown<'a> {
    model_name: &'a str,
    input_tokens: u64,
    output_tokens: u64,
    cache_creation_tokens: u64,
    cache_read_tokens: u64,
    cost: f64,
}

#[derive(Debug, Serialize)]
struct DailyEnvelope<'a> {
    daily: Vec<DailyRow<'a>>,
}

#[derive(Debug, Serialize)]
struct MonthlyEnvelope<'a> {
    monthly: Vec<MonthlyRow<'a>>,
}

fn breakdowns(models: &BTreeMap<String, ModelTotals>) -> Vec<ModelBreakdown<'_>> {
    models
        .iter()
        .map(|(name, t)| ModelBreakdown {
            model_name: name,
            input_tokens: t.input_tokens,
            output_tokens: t.output_tokens,
            cache_creation_tokens: t.cache_creation_tokens,
            cache_read_tokens: t.cache_read_tokens,
            cost: t.cost,
        })
        .collect()
}

pub fn render_daily_json(buckets: &BTreeMap<String, Bucket>) -> String {
    let rows: Vec<DailyRow> = buckets
        .iter()
        .map(|(date, b)| DailyRow {
            date,
            input_tokens: b.input_tokens,
            output_tokens: b.output_tokens,
            cache_creation_tokens: b.cache_creation_tokens,
            cache_read_tokens: b.cache_read_tokens,
            total_tokens: b.total_tokens(),
            total_cost: b.total_cost,
            models_used: b.models.keys().map(String::as_str).collect(),
            model_breakdowns: breakdowns(&b.models),
        })
        .collect();
    serde_json::to_string_pretty(&DailyEnvelope { daily: rows })
        .expect("DailyEnvelope contains only owned scalar types; serialization cannot fail")
}

pub fn render_monthly_json(buckets: &BTreeMap<String, Bucket>) -> String {
    let rows: Vec<MonthlyRow> = buckets
        .iter()
        .map(|(month, b)| MonthlyRow {
            month,
            input_tokens: b.input_tokens,
            output_tokens: b.output_tokens,
            cache_creation_tokens: b.cache_creation_tokens,
            cache_read_tokens: b.cache_read_tokens,
            total_tokens: b.total_tokens(),
            total_cost: b.total_cost,
            models_used: b.models.keys().map(String::as_str).collect(),
            model_breakdowns: breakdowns(&b.models),
        })
        .collect();
    serde_json::to_string_pretty(&MonthlyEnvelope { monthly: rows })
        .expect("MonthlyEnvelope contains only owned scalar types; serialization cannot fail")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregate::{Bucket, ModelTotals};

    fn sample_daily() -> BTreeMap<String, Bucket> {
        let mut b = Bucket {
            input_tokens: 340,
            output_tokens: 84592,
            cache_creation_tokens: 303_910,
            cache_read_tokens: 9_087_615,
            total_cost: 8.559744999999998,
            models: BTreeMap::new(),
        };
        b.models.insert(
            "claude-opus-4-7".to_string(),
            ModelTotals {
                input_tokens: 340,
                output_tokens: 84592,
                cache_creation_tokens: 303_910,
                cache_read_tokens: 9_087_615,
                cost: 8.559744999999998,
            },
        );
        let mut map = BTreeMap::new();
        map.insert("2026-04-24".to_string(), b);
        map
    }

    #[test]
    fn daily_json_uses_camel_case_keys() {
        let s = render_daily_json(&sample_daily());
        assert!(s.contains("\"daily\""));
        assert!(s.contains("\"inputTokens\""));
        assert!(s.contains("\"outputTokens\""));
        assert!(s.contains("\"cacheCreationTokens\""));
        assert!(s.contains("\"cacheReadTokens\""));
        assert!(s.contains("\"totalTokens\""));
        assert!(s.contains("\"totalCost\""));
        assert!(s.contains("\"modelsUsed\""));
        assert!(s.contains("\"modelBreakdowns\""));
        assert!(s.contains("\"modelName\""));
    }

    #[test]
    fn daily_json_preserves_full_float_precision() {
        let s = render_daily_json(&sample_daily());
        assert!(s.contains("8.559744999999998"), "\n{s}");
    }

    #[test]
    fn daily_json_has_total_tokens_sum() {
        let s = render_daily_json(&sample_daily());
        let expected = 340u64 + 84592 + 303_910 + 9_087_615;
        assert!(s.contains(&format!("\"totalTokens\": {expected}")), "\n{s}");
    }

    #[test]
    fn monthly_json_uses_month_key_not_date() {
        let mut m = sample_daily();
        let v = m.remove("2026-04-24").unwrap();
        m.insert("2026-04".to_string(), v);
        let s = render_monthly_json(&m);
        assert!(s.contains("\"monthly\""));
        assert!(s.contains("\"month\""));
        assert!(!s.contains("\"date\""));
    }
}
