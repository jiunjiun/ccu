use crate::aggregate::Bucket;
use crate::render::table::fmt_money;
use std::collections::BTreeMap;
use tabled::builder::Builder;
use tabled::settings::object::Columns;
use tabled::settings::{Alignment, Modify, Style};

/// Render a three-column box-bordered table (Date | Chart | Cost) of daily
/// costs for the most recent `days` entries in `buckets`. `term_width` is the
/// available column width; the bar column adjusts to fit.
pub fn render_chart(buckets: &BTreeMap<String, Bucket>, days: usize, term_width: usize) -> String {
    if buckets.is_empty() || days == 0 {
        return "(no data)\n".to_string();
    }

    // Newest-first via `.rev().take(days)`, then reverse in place so the table
    // displays oldest-to-newest top-to-bottom.
    let mut last_n: Vec<(&String, &Bucket)> = buckets.iter().rev().take(days).collect();
    last_n.reverse();

    let max_cost = last_n
        .iter()
        .map(|(_, b)| b.total_cost)
        .fold(0.0_f64, f64::max);

    // Width budget for the bar column. The Date column is 10 chars, the Cost
    // column is around 9 chars, plus 4 vertical bars and 6 padding spaces from
    // tabled's modern style. Leave a small right margin so the longest bar
    // doesn't slam against the terminal edge.
    const FIXED_OVERHEAD: usize = 10 + 9 + 4 + 6 + 2;
    const MAX_BAR: usize = 60;
    const MIN_BAR: usize = 10;
    let bar_width = term_width
        .saturating_sub(FIXED_OVERHEAD)
        .clamp(MIN_BAR, MAX_BAR);

    let bar_for = |cost: f64| -> String {
        let filled = if max_cost > 0.0 {
            ((cost / max_cost) * bar_width as f64).round() as usize
        } else {
            0
        };
        let filled = filled.min(bar_width);
        // Pad to full bar_width with spaces so every cell has the same width
        // and the Cost column stays vertically aligned.
        "█".repeat(filled) + &" ".repeat(bar_width - filled)
    };

    let mut b = Builder::default();
    b.push_record(["Date", "Chart", "Cost (USD)"]);
    for (date, bucket) in &last_n {
        b.push_record([
            date.as_str(),
            &bar_for(bucket.total_cost),
            &fmt_money(bucket.total_cost),
        ]);
    }

    b.build()
        .with(Style::modern())
        .with(Modify::new(Columns::new(2..=2)).with(Alignment::right()))
        .to_string()
}

/// Append a `Top N` rank label to each data row of the rendered chart string,
/// outside the right border. Rank 1 also gets a 👑 emoji. Ranking is over the
/// same N-day window the chart covers, with rank 1 = highest cost.
pub fn annotate_ranks(s: &str, buckets: &BTreeMap<String, Bucket>, days: usize) -> String {
    if buckets.is_empty() || days == 0 {
        return s.to_string();
    }
    // newest → oldest within the window; stable sort means ties resolve to the
    // newer day taking the better rank, which is fine.
    let mut by_cost: Vec<(String, f64)> = buckets
        .iter()
        .rev()
        .take(days)
        .map(|(d, b)| (d.clone(), b.total_cost))
        .collect();
    by_cost.sort_by(|a, b| {
        b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
    });
    let ranks: BTreeMap<String, usize> = by_cost
        .into_iter()
        .enumerate()
        .map(|(i, (d, _))| (d, i + 1))
        .collect();

    let mut out = String::with_capacity(s.len() + ranks.len() * 16);
    for line in s.lines() {
        out.push_str(line);
        for (date, &rank) in &ranks {
            if rank > 3 {
                continue;
            }
            if line.contains(date.as_str()) {
                if rank == 1 {
                    out.push_str(" Top 1 👑");
                } else {
                    out.push_str(&format!(" Top {rank}"));
                }
                break;
            }
        }
        out.push('\n');
    }
    out
}

/// Wrap the cost cell of the highest-cost row with a 256-color accent so the
/// top spender stands out at a glance. Operates on the rendered chart string
/// after `render_chart`; caller decides whether to apply (TTY only).
pub fn highlight_top_cost(s: &str, buckets: &BTreeMap<String, Bucket>, days: usize) -> String {
    if buckets.is_empty() || days == 0 {
        return s.to_string();
    }
    let last_n: Vec<(&String, &Bucket)> = buckets.iter().rev().take(days).collect();
    let max_cost = last_n
        .iter()
        .map(|(_, b)| b.total_cost)
        .fold(0.0_f64, f64::max);
    if max_cost == 0.0 {
        return s.to_string();
    }
    let max_date = match last_n
        .into_iter()
        .find(|(_, b)| b.total_cost == max_cost)
    {
        Some((d, _)) => d.clone(),
        None => return s.to_string(),
    };
    let cost_str = fmt_money(max_cost);
    // 256-color #ffaf00 amber + bold for emphasis; reset both attrs together.
    let highlighted = format!("\x1b[1;38;5;214m{cost_str}\x1b[22;39m");

    let mut out = String::with_capacity(s.len() + highlighted.len());
    let mut replaced = false;
    for line in s.lines() {
        if !replaced && line.contains(&max_date) {
            out.push_str(&line.replacen(&cost_str, &highlighted, 1));
            replaced = true;
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// Wrap contiguous runs of `█` (filled bar) chars with an ANSI 256-color
/// accent so bars stand out against text. Caller decides when to apply
/// (TTY only).
pub fn color_chart_bars(s: &str) -> String {
    // 256-color #5f87ff (cool azure) — readable on dark and light backgrounds.
    super::wrap_char_runs(s, |c| c == '█', "\x1b[38;5;69m", "\x1b[39m")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregate::{Bucket, ModelTotals};
    use std::collections::BTreeMap;

    fn b(cost: f64) -> Bucket {
        let mut models = BTreeMap::new();
        models.insert(
            "claude-opus-4-7".to_string(),
            ModelTotals {
                input_tokens: 0,
                output_tokens: 0,
                cache_creation_tokens: 0,
                cache_read_tokens: 0,
                cost,
            },
        );
        Bucket {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
            total_cost: cost,
            models,
        }
    }

    fn sample(entries: &[(&str, f64)]) -> BTreeMap<String, Bucket> {
        let mut map = BTreeMap::new();
        for (date, cost) in entries {
            map.insert(date.to_string(), b(*cost));
        }
        map
    }

    #[test]
    fn empty_buckets_print_no_data() {
        let out = render_chart(&BTreeMap::new(), 14, 80);
        assert_eq!(out, "(no data)\n");
    }

    #[test]
    fn zero_days_prints_no_data() {
        let buckets = sample(&[("2026-04-24", 8.0)]);
        let out = render_chart(&buckets, 0, 80);
        assert_eq!(out, "(no data)\n");
    }

    #[test]
    fn single_day_renders_full_bar() {
        let buckets = sample(&[("2026-04-24", 8.0)]);
        let out = render_chart(&buckets, 14, 80);
        assert!(out.contains("2026-04-24"));
        assert!(out.contains("$8.00"));
        assert!(out.contains("█"), "expected bar chars: \n{out}");
        // Box borders are present.
        assert!(out.contains("┌") && out.contains("│"), "no box chars: \n{out}");
    }

    #[test]
    fn ratios_proportional_to_cost() {
        let buckets = sample(&[
            ("2026-04-22", 100.0),
            ("2026-04-23", 50.0),
            ("2026-04-24", 25.0),
        ]);
        let out = render_chart(&buckets, 14, 80);
        let mut filled_per_line: Vec<usize> = out
            .lines()
            .map(|l| l.chars().filter(|&c| c == '█').count())
            .filter(|&n| n > 0)
            .collect();
        filled_per_line.sort_unstable_by(|a, b| b.cmp(a));
        assert_eq!(filled_per_line.len(), 3, "expected 3 bars: \n{out}");
        let f0 = filled_per_line[0];
        let f1 = filled_per_line[1];
        let f2 = filled_per_line[2];
        assert!(f0 > 0);
        assert!(
            (f1 as i32 - (f0 as i32 / 2)).abs() <= 1,
            "expected ~half: f0={f0} f1={f1}"
        );
        assert!(
            (f2 as i32 - (f0 as i32 / 4)).abs() <= 1,
            "expected ~quarter: f0={f0} f2={f2}"
        );
    }

    #[test]
    fn limits_to_last_n_days() {
        let buckets = sample(&[
            ("2026-04-20", 10.0),
            ("2026-04-21", 20.0),
            ("2026-04-22", 30.0),
            ("2026-04-23", 40.0),
            ("2026-04-24", 50.0),
        ]);
        let out = render_chart(&buckets, 3, 80);
        assert!(!out.contains("2026-04-20"), "\n{out}");
        assert!(!out.contains("2026-04-21"), "\n{out}");
        assert!(out.contains("2026-04-22"), "\n{out}");
        assert!(out.contains("2026-04-23"), "\n{out}");
        assert!(out.contains("2026-04-24"), "\n{out}");
    }

    #[test]
    fn header_has_three_columns_only() {
        let buckets = sample(&[("2026-04-24", 8.0)]);
        let out = render_chart(&buckets, 14, 80);
        assert!(out.contains("Date"));
        assert!(out.contains("Chart"));
        assert!(out.contains("Cost (USD)"));
        // Should NOT include daily-table columns.
        assert!(!out.contains("Models"), "Models leaked: \n{out}");
        assert!(!out.contains("Cache"), "Cache leaked: \n{out}");
        assert!(!out.contains("Total Tokens"), "Total Tokens leaked: \n{out}");
    }

    #[test]
    fn color_chart_bars_wraps_only_bar_chars() {
        let input = "███   $1.00";
        let out = color_chart_bars(input);
        assert!(out.contains("\x1b[38;5;69m███\x1b[39m"));
        assert!(out.contains("$1.00"));
    }

    #[test]
    fn color_chart_bars_is_noop_without_bar_chars() {
        let input = "no bars here";
        assert_eq!(color_chart_bars(input), input);
    }
}
