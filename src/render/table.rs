use crate::aggregate::{Bucket, DATE_FMT, MONTH_FMT};
use chrono::NaiveDate;
use regex::Regex;
use std::collections::BTreeMap;
use std::sync::OnceLock;
use tabled::builder::Builder;
use tabled::settings::object::Columns;
use tabled::settings::{Alignment, Modify, Style};

/// Shorten a Claude model name for table display.
/// `claude-haiku-4-5-20251001` → `haiku-4-5`, `claude-opus-4-7` → `opus-4-7`.
/// JSON output keeps the full name (ccusage-compatible).
fn short_model_name(model: &str) -> String {
    static DATE_SUFFIX: OnceLock<Regex> = OnceLock::new();
    let re = DATE_SUFFIX.get_or_init(|| Regex::new(r"-\d{8}$").unwrap());
    let without_prefix = model.strip_prefix("claude-").unwrap_or(model);
    re.replace(without_prefix, "").into_owned()
}

fn fmt_int(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}

pub(crate) fn fmt_money(n: f64) -> String {
    let rounded = (n * 100.0).round() / 100.0;
    let whole = rounded.trunc() as i64;
    let cents = (rounded.fract().abs() * 100.0).round() as i64;
    format!("${}.{:02}", fmt_int(whole.unsigned_abs()), cents)
}

fn models_summary(bucket: &Bucket) -> String {
    bucket
        .models
        .keys()
        .map(|m| short_model_name(m))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_table(
    buckets: &BTreeMap<NaiveDate, Bucket>,
    compact: bool,
    first_col_label: &str,
    key_format: &str,
) -> String {
    let mut b = Builder::default();
    if compact {
        b.push_record([first_col_label, "Models", "Input", "Output", "Cost (USD)"]);
    } else {
        b.push_record([
            first_col_label,
            "Models",
            "Input",
            "Output",
            "Cache Create",
            "Cache Read",
            "Total Tokens",
            "Cost (USD)",
        ]);
    }

    let mut totals = Bucket::default();
    for (key, bucket) in buckets {
        let key_str = key.format(key_format).to_string();
        if compact {
            b.push_record([
                key_str.as_str(),
                &models_summary(bucket),
                &fmt_int(bucket.input_tokens),
                &fmt_int(bucket.output_tokens),
                &fmt_money(bucket.total_cost),
            ]);
        } else {
            b.push_record([
                key_str.as_str(),
                &models_summary(bucket),
                &fmt_int(bucket.input_tokens),
                &fmt_int(bucket.output_tokens),
                &fmt_int(bucket.cache_creation_tokens),
                &fmt_int(bucket.cache_read_tokens),
                &fmt_int(bucket.total_tokens()),
                &fmt_money(bucket.total_cost),
            ]);
        }
        totals.input_tokens += bucket.input_tokens;
        totals.output_tokens += bucket.output_tokens;
        totals.cache_creation_tokens += bucket.cache_creation_tokens;
        totals.cache_read_tokens += bucket.cache_read_tokens;
        totals.total_cost += bucket.total_cost;
    }

    if compact {
        b.push_record([
            "Total",
            "",
            &fmt_int(totals.input_tokens),
            &fmt_int(totals.output_tokens),
            &fmt_money(totals.total_cost),
        ]);
    } else {
        b.push_record([
            "Total",
            "",
            &fmt_int(totals.input_tokens),
            &fmt_int(totals.output_tokens),
            &fmt_int(totals.cache_creation_tokens),
            &fmt_int(totals.cache_read_tokens),
            &fmt_int(totals.total_tokens()),
            &fmt_money(totals.total_cost),
        ]);
    }

    // First two columns (Date/Month, Models) stay left-aligned text;
    // numeric columns from index 2 onwards get right alignment so digits
    // line up under each other and the eye can spot magnitudes quickly.
    b.build()
        .with(Style::modern())
        .with(Modify::new(Columns::new(2..)).with(Alignment::right()))
        .to_string()
}

pub fn render_daily_table(buckets: &BTreeMap<NaiveDate, Bucket>, compact: bool) -> String {
    render_table(buckets, compact, "Date", DATE_FMT)
}

pub fn render_monthly_table(buckets: &BTreeMap<NaiveDate, Bucket>, compact: bool) -> String {
    render_table(buckets, compact, "Month", MONTH_FMT)
}

/// Wrap Unicode box-drawing characters with ANSI dim codes so borders render
/// less bright than cell contents. Caller decides whether to apply (typically
/// only when stdout is a terminal and `NO_COLOR` is unset).
pub fn dim_border_chars(s: &str) -> String {
    use crate::palette::{DIM_GREY, RESET_FG};
    const BOX_CHARS: &[char] = &['─', '│', '┌', '┐', '└', '┘', '├', '┤', '┬', '┴', '┼'];
    super::wrap_char_runs(s, |c| BOX_CHARS.contains(&c), DIM_GREY, RESET_FG)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregate::{Bucket, ModelTotals};
    use std::collections::BTreeMap;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn sample_buckets() -> BTreeMap<NaiveDate, Bucket> {
        let mut b24 = Bucket {
            input_tokens: 340,
            output_tokens: 84592,
            cache_creation_tokens: 303_910,
            cache_read_tokens: 9_087_615,
            total_cost: 8.559745,
            models: BTreeMap::new(),
        };
        b24.models.insert(
            "claude-opus-4-7".to_string(),
            ModelTotals {
                input_tokens: 340,
                output_tokens: 84592,
                cache_creation_tokens: 303_910,
                cache_read_tokens: 9_087_615,
                cost: 8.559745,
            },
        );

        let mut out = BTreeMap::new();
        out.insert(d("2026-04-24"), b24);
        out
    }

    #[test]
    fn daily_table_contains_header_and_row_values() {
        let table = render_daily_table(&sample_buckets(), false);
        for h in [
            "Date",
            "Models",
            "Input",
            "Output",
            "Cache Create",
            "Cache Read",
            "Total Tokens",
            "Cost (USD)",
        ] {
            assert!(table.contains(h), "missing header {h}: \n{table}");
        }
        assert!(table.contains("2026-04-24"), "row date missing: \n{table}");
        assert!(
            table.contains("opus-4-7"),
            "short model name missing: \n{table}"
        );
        assert!(
            !table.contains("claude-opus-4-7"),
            "full model name leaked into table (should be shortened): \n{table}"
        );
        assert!(table.contains("340"), "input tokens missing: \n{table}");
        assert!(
            table.contains("$8.56"),
            "cost column should be $8.56: \n{table}"
        );
    }

    #[test]
    fn daily_table_includes_total_row() {
        let table = render_daily_table(&sample_buckets(), false);
        assert!(table.contains("Total"), "Total row missing: \n{table}");
    }

    #[test]
    fn daily_table_token_columns_use_thousands_separator() {
        let table = render_daily_table(&sample_buckets(), false);
        assert!(
            table.contains("9,087,615"),
            "thousand-sep missing: \n{table}"
        );
    }

    #[test]
    fn compact_daily_table_has_five_columns() {
        let table = render_daily_table(&sample_buckets(), true);
        assert!(table.contains("Cost (USD)"));
        assert!(
            !table.contains("Cache Create"),
            "compact should omit Cache Create: \n{table}"
        );
        assert!(
            !table.contains("Cache Read"),
            "compact should omit Cache Read: \n{table}"
        );
        assert!(
            !table.contains("Total Tokens"),
            "compact should omit Total Tokens: \n{table}"
        );
    }

    #[test]
    fn dim_border_chars_wraps_box_chars_and_leaves_text_alone() {
        let input = "│ hi │\n├──┤";
        let out = dim_border_chars(input);
        // Text content untouched.
        assert!(out.contains(" hi "));
        // Starts with DIM before first box char and closes RESET before text.
        assert!(out.contains("\x1b[38;5;242m│"));
        assert!(out.contains("│\x1b[39m"));
        // The box-only row is fully wrapped (no mid-run resets).
        assert!(out.contains("\x1b[38;5;242m├──┤\x1b[39m"));
    }

    #[test]
    fn dim_border_chars_is_noop_when_no_box_chars() {
        let input = "just plain text\n42";
        assert_eq!(dim_border_chars(input), input);
    }

    #[test]
    fn short_model_name_strips_prefix_and_date_suffix() {
        assert_eq!(
            super::short_model_name("claude-haiku-4-5-20251001"),
            "haiku-4-5"
        );
        assert_eq!(
            super::short_model_name("claude-opus-4-5-20251101"),
            "opus-4-5"
        );
        assert_eq!(super::short_model_name("claude-opus-4-7"), "opus-4-7");
        assert_eq!(super::short_model_name("claude-sonnet-4-6"), "sonnet-4-6");
        assert_eq!(super::short_model_name("claude-3-opus-20240229"), "3-opus");
        // Unknown / already short names pass through with just prefix stripped.
        assert_eq!(super::short_model_name("foo-model"), "foo-model");
    }

    #[test]
    fn monthly_table_uses_month_header_not_date() {
        let mut buckets = sample_buckets();
        let v = buckets.remove(&d("2026-04-24")).unwrap();
        buckets.insert(d("2026-04-01"), v);
        let table = render_monthly_table(&buckets, false);
        assert!(table.contains("Month"), "Month header missing: \n{table}");
        assert!(
            !table.contains("Date"),
            "Date header leaked into monthly: \n{table}"
        );
    }
}
