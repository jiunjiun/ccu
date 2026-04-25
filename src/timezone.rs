use chrono::{DateTime, Local, Utc};
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

    pub fn day_key(&self, ts: DateTime<Utc>) -> String {
        match self {
            Timezone::Local => ts.with_timezone(&Local).format("%Y-%m-%d").to_string(),
            Timezone::Named(tz) => ts.with_timezone(tz).format("%Y-%m-%d").to_string(),
        }
    }

    pub fn month_key(&self, ts: DateTime<Utc>) -> String {
        match self {
            Timezone::Local => ts.with_timezone(&Local).format("%Y-%m").to_string(),
            Timezone::Named(tz) => ts.with_timezone(tz).format("%Y-%m").to_string(),
        }
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
    fn day_key_named_tz_handles_crossing_midnight() {
        // 17:00 UTC on Apr 24 = 01:00 Apr 25 in Asia/Taipei (UTC+8)
        let tz = Timezone::parse("Asia/Taipei").unwrap();
        assert_eq!(tz.day_key(at("2026-04-24T17:00:00Z")), "2026-04-25");
    }

    #[test]
    fn day_key_named_tz_stays_same_day_before_midnight() {
        let tz = Timezone::parse("Asia/Taipei").unwrap();
        // 15:00 UTC = 23:00 same day in TP
        assert_eq!(tz.day_key(at("2026-04-24T15:00:00Z")), "2026-04-24");
    }

    #[test]
    fn month_key_named_tz_handles_crossing_month() {
        // 2026-03-31T17:00:00Z = 2026-04-01 01:00 in Asia/Taipei
        let tz = Timezone::parse("Asia/Taipei").unwrap();
        assert_eq!(tz.month_key(at("2026-03-31T17:00:00Z")), "2026-04");
    }

    #[test]
    fn day_key_utc_differs_from_taipei_at_boundary() {
        let utc = Timezone::parse("UTC").unwrap();
        let tp = Timezone::parse("Asia/Taipei").unwrap();
        let ts = at("2026-04-24T17:00:00Z");
        assert_eq!(utc.day_key(ts), "2026-04-24");
        assert_eq!(tp.day_key(ts), "2026-04-25");
    }
}
