pub mod aggregate;
pub mod cli;
pub mod dedup;
pub mod discover;
pub mod entry;
pub mod palette;
pub mod pricing;
pub mod render;
pub mod timezone;
pub mod update;

use chrono::{DateTime, Duration, Local, NaiveDate, TimeZone, Utc};

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

fn no_color_env() -> bool {
    std::env::var_os("NO_COLOR").is_some()
}

/// Whether stdout should receive ANSI color: TTY + `NO_COLOR` unset. Piping to
/// a file or another command gets clean plain-text output.
pub fn stdout_supports_color() -> bool {
    use std::io::IsTerminal;
    !no_color_env() && std::io::stdout().is_terminal()
}

/// Same rule for stderr (used by the version-check banner).
pub fn stderr_supports_color() -> bool {
    use std::io::IsTerminal;
    !no_color_env() && std::io::stderr().is_terminal()
}

fn maybe_dim(table: String) -> String {
    if stdout_supports_color() {
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
    load_entries_since(None)
}

/// JSONL files are append-only, so a file last modified before `threshold`
/// can't contain newer entries. Skipping those files lets `ccu today` /
/// `ccu month` avoid touching months of stale history on disk.
fn load_entries_since(threshold: Option<DateTime<Utc>>) -> anyhow::Result<Vec<UsageEntry>> {
    use rayon::prelude::*;
    let root = scan_root()?;

    // Parse files in parallel — they're independent JSONL files. Then sort
    // by earliest entry timestamp before dedup so that when the same
    // (msgId, requestId) appears across files, the FIRST-encountered entry
    // wins (matches ccusage for numeric parity).
    let mut files: Vec<(Option<DateTime<Utc>>, Vec<UsageEntry>)> = discover_jsonl(&root)
        .into_par_iter()
        .filter(|(_, mtime)| match threshold {
            Some(min) => mtime.is_none_or(|mt| mt >= min),
            None => true,
        })
        .map(|(p, _)| {
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

    let total: usize = files.iter().map(|(_, v)| v.len()).sum();
    let mut entries: Vec<UsageEntry> = Vec::with_capacity(total);
    for (_, v) in files {
        entries.extend(v);
    }
    Ok(dedup_entries(entries))
}

/// Local-midnight of `day` in UTC, minus one day of slack — any file last
/// modified before this can't contain entries for `day` regardless of the
/// user's timezone.
fn day_threshold_utc(day: NaiveDate) -> DateTime<Utc> {
    let local_midnight = day.and_hms_opt(0, 0, 0).expect("00:00 is a valid time");
    let utc = Local
        .from_local_datetime(&local_midnight)
        .earliest()
        .expect("Local midnight resolves to at least one UTC instant")
        .with_timezone(&Utc);
    utc - Duration::days(1)
}

fn sum_cost_where<F: Fn(DateTime<Utc>) -> bool>(entries: &[UsageEntry], predicate: F) -> f64 {
    entries
        .iter()
        .filter(|e| predicate(e.timestamp))
        .filter_map(|e| {
            let model = e.message.model.as_deref()?;
            let usage = e.message.usage.as_ref()?;
            Some(cost_of(usage, model))
        })
        .fold(0.0_f64, |acc, x| acc + x)
}

pub fn run_today(date: Option<&str>) -> anyhow::Result<()> {
    let target = match date {
        Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|e| anyhow::anyhow!("invalid date {s:?}: {e}"))?,
        None => Local::now().date_naive(),
    };
    let entries = load_entries_since(Some(day_threshold_utc(target)))?;
    print_bare_cost(sum_cost_where(&entries, |ts| {
        Timezone::Local.day_naive(ts) == target
    }));
    Ok(())
}

pub fn run_month(month: Option<&str>) -> anyhow::Result<()> {
    let target = match month {
        Some(s) => parse_month(s)?,
        None => Timezone::Local.month_naive(Utc::now()),
    };
    let entries = load_entries_since(Some(day_threshold_utc(target)))?;
    print_bare_cost(sum_cost_where(&entries, |ts| {
        Timezone::Local.month_naive(ts) == target
    }));
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
    if stdout_supports_color() {
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

/// Parse `YYYY-MM` to the first-of-month `NaiveDate`, matching the shape
/// `Timezone::month_naive` returns so callers can compare directly.
fn parse_month(s: &str) -> anyhow::Result<NaiveDate> {
    if s.len() == 7 && s.as_bytes()[4] == b'-' {
        if let Ok(d) = NaiveDate::parse_from_str(&format!("{s}-01"), "%Y-%m-%d") {
            return Ok(d);
        }
    }
    anyhow::bail!("invalid month {s:?}; expected YYYY-MM")
}

fn resolve_tz(tz: Option<&str>) -> anyhow::Result<Timezone> {
    match tz {
        Some(s) => Timezone::parse(s),
        None => Ok(Timezone::Local),
    }
}
