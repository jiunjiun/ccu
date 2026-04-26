pub mod aggregate;
pub mod cli;
pub mod dedup;
pub mod discover;
pub mod entry;
pub mod pricing;
pub mod render;
pub mod timezone;
pub mod update;

use chrono::{DateTime, Local, NaiveDate, Utc};

use crate::aggregate::{group_by_day, group_by_month};
use crate::dedup::dedup_entries;
use crate::discover::{discover_jsonl, scan_root};
use crate::entry::{parse_file, UsageEntry};
use crate::pricing::cost_of;
use crate::render::bare::print_bare_cost;
use crate::render::chart::{annotate_ranks, color_chart_bars, highlight_top_cost, render_chart};
use crate::render::json::{render_daily_json, render_monthly_json};
use crate::render::table::{dim_border_chars, render_daily_table, render_monthly_table};
use crate::timezone::Timezone;

/// Dim ANSI escapes only when stdout is a TTY and NO_COLOR is unset.
/// Piping to a file or another command gets clean plain-text output.
fn should_use_color() -> bool {
    use std::io::IsTerminal;
    std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal()
}

fn maybe_dim(table: String) -> String {
    if should_use_color() {
        dim_border_chars(&table)
    } else {
        table
    }
}

fn terminal_width() -> usize {
    terminal_size::terminal_size()
        .map(|(terminal_size::Width(w), _)| w as usize)
        .unwrap_or(80)
}

fn load_entries() -> anyhow::Result<Vec<UsageEntry>> {
    let root = scan_root()?;

    // Sort files by their earliest entry timestamp before dedup. When the same
    // (msgId, requestId) pair appears across files, the FIRST-encountered entry
    // wins, so file order is load-bearing. ccusage sorts files this way too;
    // match that ordering for numeric parity.
    let mut files: Vec<(Option<DateTime<Utc>>, Vec<UsageEntry>)> = discover_jsonl(&root)
        .into_iter()
        .map(|p| {
            let entries = parse_file(&p).unwrap_or_default();
            let earliest = entries.iter().map(|e| e.timestamp).min();
            (earliest, entries)
        })
        .collect();
    files.sort_by(|a, b| match (a.0, b.0) {
        (Some(ta), Some(tb)) => ta.cmp(&tb),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });

    let entries: Vec<UsageEntry> = files.into_iter().flat_map(|(_, e)| e).collect();
    Ok(dedup_entries(entries))
}

pub fn run_today(date: Option<&str>) -> anyhow::Result<()> {
    let target = match date {
        Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|e| anyhow::anyhow!("invalid date {s:?}: {e}"))?,
        None => Local::now().date_naive(),
    };

    let entries = load_entries()?;

    let total: f64 = entries
        .iter()
        .filter(|e| local_day(e.timestamp) == target)
        .filter_map(|e| {
            let model = e.message.model.as_deref()?;
            let usage = e.message.usage.as_ref()?;
            Some(cost_of(usage, model))
        })
        .fold(0.0_f64, |acc, x| acc + x);

    print_bare_cost(total);
    Ok(())
}

pub fn run_month(month: Option<&str>) -> anyhow::Result<()> {
    let target = match month {
        Some(s) => validate_month(s)?.to_string(),
        None => Local::now().format("%Y-%m").to_string(),
    };

    let entries = load_entries()?;

    let total: f64 = entries
        .iter()
        .filter(|e| local_month(e.timestamp) == target)
        .filter_map(|e| {
            let model = e.message.model.as_deref()?;
            let usage = e.message.usage.as_ref()?;
            Some(cost_of(usage, model))
        })
        .fold(0.0_f64, |acc, x| acc + x);

    print_bare_cost(total);
    Ok(())
}

pub fn run_daily(json: bool, compact: bool, tz: Option<&str>) -> anyhow::Result<()> {
    let tz = resolve_tz(tz)?;
    let entries = load_entries()?;
    let buckets = group_by_day(&entries, tz);

    if json {
        println!("{}", render_daily_json(&buckets));
    } else {
        println!("{}", maybe_dim(render_daily_table(&buckets, compact)));
    }
    Ok(())
}

pub fn run_chart(days: u32, tz: Option<&str>) -> anyhow::Result<()> {
    let tz = resolve_tz(tz)?;
    let entries = load_entries()?;
    let buckets = group_by_day(&entries, tz);

    let n = days as usize;
    let mut out = render_chart(&buckets, n, terminal_width());
    if should_use_color() {
        out = color_chart_bars(&out);
        out = dim_border_chars(&out);
        out = highlight_top_cost(&out, &buckets, n);
    }
    out = annotate_ranks(&out, &buckets, n);
    println!("{out}");
    Ok(())
}

pub fn run_monthly(json: bool, compact: bool, tz: Option<&str>) -> anyhow::Result<()> {
    let tz = resolve_tz(tz)?;
    let entries = load_entries()?;
    let buckets = group_by_month(&entries, tz);

    if json {
        println!("{}", render_monthly_json(&buckets));
    } else {
        println!("{}", maybe_dim(render_monthly_table(&buckets, compact)));
    }
    Ok(())
}

fn local_day(ts: DateTime<Utc>) -> NaiveDate {
    ts.with_timezone(&Local).date_naive()
}

fn local_month(ts: DateTime<Utc>) -> String {
    ts.with_timezone(&Local).format("%Y-%m").to_string()
}

fn validate_month(s: &str) -> anyhow::Result<&str> {
    if s.len() == 7
        && s.as_bytes()[4] == b'-'
        && NaiveDate::parse_from_str(&format!("{s}-01"), "%Y-%m-%d").is_ok()
    {
        return Ok(s);
    }
    anyhow::bail!("invalid month {s:?}; expected YYYY-MM")
}

fn resolve_tz(tz: Option<&str>) -> anyhow::Result<Timezone> {
    match tz {
        Some(s) => Timezone::parse(s),
        None => Ok(Timezone::Local),
    }
}
