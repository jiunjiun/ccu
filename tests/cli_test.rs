use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Copy the named fixture into a private temp dir and return the dir.
/// Callers point `CCU_PROJECTS_DIR` at it so the CLI scans only that data.
fn fixture_dir(names: &[&str]) -> TempDir {
    let tmp = TempDir::new().unwrap();
    for name in names {
        let src = format!("tests/fixtures/{name}");
        let dst = tmp.path().join(name);
        fs::copy(&src, &dst).unwrap_or_else(|e| panic!("copy {src}: {e}"));
    }
    tmp
}

fn ccu() -> Command {
    Command::cargo_bin("ccu").unwrap()
}

#[test]
fn today_prints_cost_for_single_day_fixture() {
    let tmp = fixture_dir(&["single_day.jsonl"]);
    ccu()
        .env("CCU_PROJECTS_DIR", tmp.path())
        .args(["today", "2026-04-24"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("8.5597"));
}

#[test]
fn today_prints_zero_for_day_with_no_entries() {
    let tmp = fixture_dir(&["single_day.jsonl"]);
    ccu()
        .env("CCU_PROJECTS_DIR", tmp.path())
        .args(["today", "2026-01-01"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("0"));
}

#[test]
fn today_dedups_duplicate_message_request_pairs() {
    let tmp = fixture_dir(&["with_duplicates.jsonl"]);
    ccu()
        .env("CCU_PROJECTS_DIR", tmp.path())
        .args(["today", "2026-04-24"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("8.5652"));
}

#[test]
fn month_prints_cost_for_fixture_month() {
    let tmp = fixture_dir(&["with_duplicates.jsonl"]);
    ccu()
        .env("CCU_PROJECTS_DIR", tmp.path())
        .args(["month", "2026-04"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("8.5652"));
}

#[test]
fn month_prints_zero_for_month_with_no_entries() {
    let tmp = fixture_dir(&["with_duplicates.jsonl"]);
    ccu()
        .env("CCU_PROJECTS_DIR", tmp.path())
        .args(["month", "2020-01"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("0"));
}

#[test]
fn today_without_date_succeeds_and_prints_a_number() {
    let tmp = fixture_dir(&["single_day.jsonl"]);
    ccu()
        .env("CCU_PROJECTS_DIR", tmp.path())
        .arg("today")
        .assert()
        .success()
        .stdout(predicate::function(|s: &str| {
            s.trim().parse::<f64>().is_ok()
        }));
}

#[test]
fn month_without_arg_succeeds_and_prints_a_number() {
    let tmp = fixture_dir(&["single_day.jsonl"]);
    ccu()
        .env("CCU_PROJECTS_DIR", tmp.path())
        .arg("month")
        .assert()
        .success()
        .stdout(predicate::function(|s: &str| {
            s.trim().parse::<f64>().is_ok()
        }));
}

#[test]
fn today_rejects_malformed_date() {
    let tmp = fixture_dir(&["single_day.jsonl"]);
    ccu()
        .env("CCU_PROJECTS_DIR", tmp.path())
        .args(["today", "not-a-date"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid date"));
}

#[test]
fn month_rejects_malformed_month() {
    let tmp = fixture_dir(&["single_day.jsonl"]);
    ccu()
        .env("CCU_PROJECTS_DIR", tmp.path())
        .args(["month", "2026-13"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid month"));
}

#[test]
fn daily_table_shows_all_days_and_models() {
    let tmp = fixture_dir(&["multi_model.jsonl"]);
    let cmd = ccu()
        .env("CCU_PROJECTS_DIR", tmp.path())
        .arg("daily")
        .assert()
        .success();

    let out = String::from_utf8(cmd.get_output().stdout.clone()).unwrap();
    assert!(out.contains("2026-04-24"), "\n{out}");
    assert!(out.contains("2026-04-25"), "\n{out}");
    // Table displays shortened model names (claude- prefix stripped, date suffix trimmed).
    assert!(out.contains("opus-4-7"), "\n{out}");
    assert!(out.contains("sonnet-4-5"), "\n{out}");
    assert!(out.contains("haiku-4-5"), "\n{out}");
    assert!(
        !out.contains("claude-opus-4-7"),
        "full name should be shortened: \n{out}"
    );
    assert!(out.contains("Total"), "\n{out}");
    assert!(out.contains("$9.00"), "\n{out}");
}

#[test]
fn daily_json_flag_emits_schema_compatible_output() {
    let tmp = fixture_dir(&["multi_model.jsonl"]);
    let out = ccu()
        .env("CCU_PROJECTS_DIR", tmp.path())
        .args(["daily", "--json"])
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();

    let v: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("invalid JSON: {e}\n{stdout}"));

    let daily = v["daily"].as_array().expect("daily array");
    assert!(!daily.is_empty());

    let first = &daily[0];
    assert!(first["date"].is_string());
    assert!(first["inputTokens"].is_u64());
    assert!(first["modelBreakdowns"].is_array());
    assert!(first["modelsUsed"].is_array());
}

#[test]
fn monthly_table_shows_month_column_and_total() {
    let tmp = fixture_dir(&["multi_model.jsonl"]);
    let out = ccu()
        .env("CCU_PROJECTS_DIR", tmp.path())
        .arg("monthly")
        .assert()
        .success();
    let s = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(s.contains("Month"), "\n{s}");
    assert!(s.contains("2026-04"), "\n{s}");
    assert!(s.contains("Total"), "\n{s}");
}

#[test]
fn monthly_json_uses_month_key() {
    let tmp = fixture_dir(&["multi_model.jsonl"]);
    let out = ccu()
        .env("CCU_PROJECTS_DIR", tmp.path())
        .args(["monthly", "--json"])
        .assert()
        .success();
    let s = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    let arr = v["monthly"].as_array().expect("monthly array");
    assert!(!arr.is_empty());
    assert!(arr[0]["month"].is_string(), "\n{s}");
}

#[test]
fn daily_compact_omits_cache_and_total_columns() {
    let tmp = fixture_dir(&["multi_model.jsonl"]);
    let out = ccu()
        .env("CCU_PROJECTS_DIR", tmp.path())
        .args(["daily", "--compact"])
        .assert()
        .success();
    let s = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(s.contains("Date"));
    assert!(s.contains("Cost (USD)"));
    assert!(!s.contains("Cache Create"), "\n{s}");
    assert!(!s.contains("Cache Read"), "\n{s}");
    assert!(!s.contains("Total Tokens"), "\n{s}");
}

#[test]
fn monthly_compact_omits_cache_and_total_columns() {
    let tmp = fixture_dir(&["multi_model.jsonl"]);
    let out = ccu()
        .env("CCU_PROJECTS_DIR", tmp.path())
        .args(["monthly", "--compact"])
        .assert()
        .success();
    let s = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(s.contains("Month"));
    assert!(!s.contains("Cache Create"), "\n{s}");
}

#[test]
fn daily_compact_with_json_still_emits_full_schema() {
    let tmp = fixture_dir(&["multi_model.jsonl"]);
    let out = ccu()
        .env("CCU_PROJECTS_DIR", tmp.path())
        .args(["daily", "--json", "--compact"])
        .assert()
        .success();
    let s = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(s.contains("\"cacheCreationTokens\""), "\n{s}");
}

#[test]
fn daily_tz_utc_buckets_entry_on_utc_day() {
    let tmp = fixture_dir(&["cross_tz_boundary.jsonl"]);
    let out = ccu()
        .env("CCU_PROJECTS_DIR", tmp.path())
        .args(["daily", "--json", "--tz", "UTC"])
        .assert()
        .success();
    let s = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    let daily = v["daily"].as_array().unwrap();
    assert_eq!(daily.len(), 1);
    assert_eq!(daily[0]["date"], "2026-04-24");
}

#[test]
fn daily_tz_taipei_buckets_entry_on_next_day() {
    let tmp = fixture_dir(&["cross_tz_boundary.jsonl"]);
    let out = ccu()
        .env("CCU_PROJECTS_DIR", tmp.path())
        .args(["daily", "--json", "--tz", "Asia/Taipei"])
        .assert()
        .success();
    let s = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    let daily = v["daily"].as_array().unwrap();
    assert_eq!(daily.len(), 1);
    assert_eq!(daily[0]["date"], "2026-04-25");
}

#[test]
fn daily_tz_rejects_bogus_zone() {
    let tmp = fixture_dir(&["cross_tz_boundary.jsonl"]);
    ccu()
        .env("CCU_PROJECTS_DIR", tmp.path())
        .args(["daily", "--tz", "Not/A_Zone"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not/A_Zone"));
}

#[test]
fn no_subcommand_defaults_to_daily() {
    let tmp = fixture_dir(&["multi_model.jsonl"]);
    let out = ccu().env("CCU_PROJECTS_DIR", tmp.path()).assert().success();
    let s = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    // Same table headers that `ccu daily` emits.
    assert!(s.contains("Date"), "\n{s}");
    assert!(s.contains("Cost (USD)"), "\n{s}");
    assert!(s.contains("Total"), "\n{s}");
}

#[test]
fn version_flag_uses_lowercase_v() {
    ccu()
        .arg("-v")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("ccu "));
}

#[test]
fn uppercase_v_short_is_not_accepted() {
    // clap's built-in `-V` is disabled; `-V` should fail as unknown arg.
    ccu().arg("-V").assert().failure();
}
