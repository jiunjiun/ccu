use chrono::{DateTime, Datelike, Local, NaiveDate, Utc};
use chrono_tz::Tz;
use std::str::FromStr;

#[derive(Debug, Clone, Copy)]
pub enum Timezone {
    Local,
    Named(Tz),
}

impl Timezone {
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        if s.eq_ignore_ascii_case("local") {
            return Ok(Timezone::Local);
        }
        Tz::from_str(s)
            .map(Timezone::Named)
            .map_err(|_| anyhow::anyhow!("unknown timezone: {s}"))
    }

    fn date_in_zone(&self, ts: DateTime<Utc>) -> NaiveDate {
        match self {
            Timezone::Local => ts.with_timezone(&Local).date_naive(),
            Timezone::Named(tz) => ts.with_timezone(tz).date_naive(),
        }
    }

    pub fn day_naive(&self, ts: DateTime<Utc>) -> NaiveDate {
        self.date_in_zone(ts)
    }

    /// Return the first-of-month `NaiveDate` for this timestamp in `self`'s
    /// zone — used as a stable, ordered key for monthly buckets.
    pub fn month_naive(&self, ts: DateTime<Utc>) -> NaiveDate {
        let d = self.date_in_zone(ts);
        NaiveDate::from_ymd_opt(d.year(), d.month(), 1).expect("year/month from date_in_zone")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(iso: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(iso)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn naive(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn parse_returns_local_for_local_literal() {
        assert!(matches!(Timezone::parse("Local").unwrap(), Timezone::Local));
        assert!(matches!(Timezone::parse("local").unwrap(), Timezone::Local));
    }

    #[test]
    fn parse_returns_named_for_iana_string() {
        let tz = Timezone::parse("Asia/Taipei").unwrap();
        assert!(matches!(tz, Timezone::Named(_)));
    }

    #[test]
    fn parse_rejects_bogus_name() {
        let err = Timezone::parse("Not/A_Zone").unwrap_err();
        assert!(err.to_string().contains("Not/A_Zone"));
    }

    #[test]
    fn day_naive_named_tz_handles_crossing_midnight() {
        // 17:00 UTC on Apr 24 = 01:00 Apr 25 in Asia/Taipei (UTC+8)
        let tz = Timezone::parse("Asia/Taipei").unwrap();
        assert_eq!(
            tz.day_naive(at("2026-04-24T17:00:00Z")),
            naive("2026-04-25")
        );
    }

    #[test]
    fn day_naive_named_tz_stays_same_day_before_midnight() {
        let tz = Timezone::parse("Asia/Taipei").unwrap();
        assert_eq!(
            tz.day_naive(at("2026-04-24T15:00:00Z")),
            naive("2026-04-24")
        );
    }

    #[test]
    fn month_naive_named_tz_handles_crossing_month() {
        // 2026-03-31T17:00:00Z = 2026-04-01 01:00 in Asia/Taipei
        let tz = Timezone::parse("Asia/Taipei").unwrap();
        assert_eq!(
            tz.month_naive(at("2026-03-31T17:00:00Z")),
            naive("2026-04-01")
        );
    }

    #[test]
    fn day_naive_utc_differs_from_taipei_at_boundary() {
        let utc = Timezone::parse("UTC").unwrap();
        let tp = Timezone::parse("Asia/Taipei").unwrap();
        let ts = at("2026-04-24T17:00:00Z");
        assert_eq!(utc.day_naive(ts), naive("2026-04-24"));
        assert_eq!(tp.day_naive(ts), naive("2026-04-25"));
    }
}
