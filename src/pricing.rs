use regex::Regex;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    pub cache_write_5m: f64,
    pub cache_write_1h: f64,
    pub cache_read: f64,
}

impl Price {
    /// Every published tier fixes the other four rates as multiples of the
    /// base input rate: output 5x, 5-minute cache write 1.25x, 1-hour cache
    /// write 2x, cache read 0.1x. Fable 5.1 is the lone exception and
    /// overrides `cache_read`.
    ///
    /// `input / 10.0` rather than `input * 0.1`: the latter yields
    /// 0.30000000000000004 for the $3 Sonnet tier.
    fn from_input(input: f64) -> Self {
        Price {
            input,
            output: input * 5.0,
            cache_write_5m: input * 1.25,
            cache_write_1h: input * 2.0,
            cache_read: input / 10.0,
        }
    }
}

/// Per-duration breakdown of `cache_creation_input_tokens`. 1-hour cache
/// writes bill at 2x base input vs 1.25x for 5-minute, so pricing the flat
/// total at the 5m rate undercounts whenever Claude Code uses 1h caching.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
pub struct CacheCreation {
    #[serde(default)]
    pub ephemeral_5m_input_tokens: u64,
    #[serde(default)]
    pub ephemeral_1h_input_tokens: u64,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
    #[serde(default)]
    pub cache_creation: Option<CacheCreation>,
}

struct Tier {
    pattern: Regex,
    price: Price,
}

fn table() -> &'static [Tier] {
    static T: OnceLock<Vec<Tier>> = OnceLock::new();
    T.get_or_init(|| {
        vec![
            Tier {
                // Fable 5.1 keeps the Fable 5 rates but prices cache hits at
                // 0.025x base input ($0.25/MTok) instead of the usual 0.1x.
                pattern: Regex::new(r"fable-5-1\b").unwrap(),
                price: Price {
                    cache_read: 0.25,
                    ..Price::from_input(10.0)
                },
            },
            Tier {
                // Fable 5 (Jun 2026): $10/$50 flagship tier. Full 1M context
                // at standard pricing — no long-context premium to model.
                pattern: Regex::new(r"fable").unwrap(),
                price: Price::from_input(10.0),
            },
            Tier {
                // Why: Opus 4.5 (Nov 2025) shifted to a $5 input tier; older
                // Opus models stay at $15. The trailing `\b` is load-bearing:
                // it stops `\d{2,3}` from matching the date digits in legacy
                // IDs like `claude-3-opus-20240229`, while still admitting
                // bare IDs like `claude-opus-5` and `claude-opus-5[1m]`.
                pattern: Regex::new(r"opus-(4-([5-9]|\d{2,3})|[5-9]|\d{2,3})\b").unwrap(),
                price: Price::from_input(5.0),
            },
            Tier {
                pattern: Regex::new(r"opus").unwrap(),
                price: Price::from_input(15.0),
            },
            Tier {
                // Sonnet 5 (2026) sits at $2/$10; Sonnet 4.6 and earlier stay
                // at $3/$15. Same `\b` guard as the Opus tier above.
                pattern: Regex::new(r"sonnet-([5-9]|\d{2,3})\b").unwrap(),
                price: Price::from_input(2.0),
            },
            Tier {
                pattern: Regex::new(r"sonnet").unwrap(),
                price: Price::from_input(3.0),
            },
            Tier {
                pattern: Regex::new(r"haiku").unwrap(),
                price: Price::from_input(1.0),
            },
        ]
    })
}

pub fn price_for(model: &str) -> Price {
    for t in table() {
        if t.pattern.is_match(model) {
            return t.price;
        }
    }
    Price::from_input(0.0)
}

pub fn cost_of(usage: &Usage, model: &str) -> f64 {
    let p = price_for(model);
    // Entries without a breakdown predate 1h caching in Claude Code, so the
    // flat total at the 5m rate is the price they were actually billed at.
    let cache_write = match &usage.cache_creation {
        Some(c) => {
            c.ephemeral_5m_input_tokens as f64 * p.cache_write_5m
                + c.ephemeral_1h_input_tokens as f64 * p.cache_write_1h
        }
        None => usage.cache_creation_input_tokens as f64 * p.cache_write_5m,
    };
    (usage.input_tokens as f64 * p.input
        + usage.output_tokens as f64 * p.output
        + cache_write
        + usage.cache_read_input_tokens as f64 * p.cache_read)
        / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_4_7_matches_new_tier_before_generic_opus() {
        let p = price_for("claude-opus-4-7");
        assert_eq!(
            p,
            Price {
                input: 5.0,
                output: 25.0,
                cache_write_5m: 6.25,
                cache_write_1h: 10.0,
                cache_read: 0.5
            }
        );
    }

    #[test]
    fn fable_5_matches_fable_tier() {
        let p = price_for("claude-fable-5");
        assert_eq!(
            p,
            Price {
                input: 10.0,
                output: 50.0,
                cache_write_5m: 12.5,
                cache_write_1h: 20.0,
                cache_read: 1.0
            }
        );
    }

    #[test]
    fn opus_4_6_matches_new_tier() {
        assert_eq!(price_for("claude-opus-4-6").input, 5.0);
    }

    #[test]
    fn opus_4_5_matches_new_tier() {
        // Opus 4.5 (Nov 2025) was the first Opus at the $5 tier.
        assert_eq!(price_for("claude-opus-4-5-20251101").input, 5.0);
    }

    #[test]
    fn opus_double_digit_minor_matches_new_tier() {
        // Future-proof: opus-4-10..4-99 should not silently fall to legacy.
        assert_eq!(price_for("claude-opus-4-10").input, 5.0);
        assert_eq!(price_for("claude-opus-4-99").input, 5.0);
    }

    #[test]
    fn opus_major_5_plus_matches_new_tier() {
        assert_eq!(price_for("claude-opus-5-0").input, 5.0);
        assert_eq!(price_for("claude-opus-10-0").input, 5.0);
    }

    #[test]
    fn opus_5_matches_new_tier() {
        // Regression: `claude-opus-5` has no trailing `-`, so the old pattern
        // missed it and billed it at the legacy $15 tier — 3x too high.
        assert_eq!(price_for("claude-opus-5").input, 5.0);
        // The 1M context window bills at standard rates, not a premium.
        assert_eq!(price_for("claude-opus-5[1m]").input, 5.0);
    }

    #[test]
    fn sonnet_5_matches_cheaper_tier() {
        assert_eq!(
            price_for("claude-sonnet-5"),
            Price {
                input: 2.0,
                output: 10.0,
                cache_write_5m: 2.5,
                cache_write_1h: 4.0,
                cache_read: 0.2
            }
        );
        assert_eq!(price_for("claude-sonnet-5[1m]").input, 2.0);
    }

    #[test]
    fn sonnet_4_x_stays_at_legacy_sonnet_tier() {
        assert_eq!(price_for("claude-sonnet-4-6").input, 3.0);
        assert_eq!(price_for("claude-sonnet-4-5").input, 3.0);
    }

    #[test]
    fn legacy_dated_sonnet_ids_stay_at_legacy_sonnet_tier() {
        // Guards the `\b` in the Sonnet 5 pattern: without it, `\d{2,3}`
        // would match the leading date digits here and underprice these at
        // the $2 tier.
        assert_eq!(price_for("claude-3-5-sonnet-20241022").input, 3.0);
        assert_eq!(price_for("claude-3-7-sonnet-20250219").input, 3.0);
        assert_eq!(price_for("claude-sonnet-4-20250514").input, 3.0);
    }

    #[test]
    fn fable_5_1_prices_cache_hits_at_a_quarter() {
        assert_eq!(
            price_for("claude-fable-5-1"),
            Price {
                input: 10.0,
                output: 50.0,
                cache_write_5m: 12.5,
                cache_write_1h: 20.0,
                cache_read: 0.25
            }
        );
        assert_eq!(price_for("claude-fable-5-1[1m]").cache_read, 0.25);
        // Fable 5 keeps the standard 0.1x cache hit rate.
        assert_eq!(price_for("claude-fable-5").cache_read, 1.0);
    }

    #[test]
    fn opus_4_below_5_minor_falls_to_legacy() {
        // 4-0..4-4 don't exist in practice but if Anthropic ever ships one
        // it should price as legacy until we explicitly add a tier.
        assert_eq!(price_for("claude-opus-4-2").input, 15.0);
    }

    #[test]
    fn claude_3_opus_matches_legacy_tier() {
        let p = price_for("claude-3-opus-20240229");
        assert_eq!(
            p,
            Price {
                input: 15.0,
                output: 75.0,
                cache_write_5m: 18.75,
                cache_write_1h: 30.0,
                cache_read: 1.5
            }
        );
    }

    #[test]
    fn haiku_matches_haiku_tier() {
        assert_eq!(price_for("claude-haiku-4-5-20251001").input, 1.0);
    }

    #[test]
    fn unknown_model_is_zero_priced() {
        let p = price_for("synth-model-9000");
        assert_eq!(
            p,
            Price {
                input: 0.0,
                output: 0.0,
                cache_write_5m: 0.0,
                cache_write_1h: 0.0,
                cache_read: 0.0
            }
        );
    }

    #[test]
    fn cost_applies_per_million_tokens() {
        let usage = Usage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            cache_creation_input_tokens: 1_000_000,
            cache_read_input_tokens: 1_000_000,
            cache_creation: None,
        };
        assert_eq!(cost_of(&usage, "claude-opus-4-7"), 5.0 + 25.0 + 6.25 + 0.5);
    }

    #[test]
    fn cost_splits_cache_writes_by_duration_when_breakdown_present() {
        let usage = Usage {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_input_tokens: 1_000_000,
            cache_read_input_tokens: 0,
            cache_creation: Some(CacheCreation {
                ephemeral_5m_input_tokens: 400_000,
                ephemeral_1h_input_tokens: 600_000,
            }),
        };
        // 0.4M at 6.25 + 0.6M at 10.0, NOT 1M at the flat 5m rate (6.25).
        let got = cost_of(&usage, "claude-opus-4-7");
        let expected = 0.4 * 6.25 + 0.6 * 10.0;
        assert!((got - expected).abs() < 1e-12, "{got} vs {expected}");
    }

    #[test]
    fn cost_without_breakdown_falls_back_to_5m_rate() {
        // Pre-breakdown Claude Code logs only ever wrote 5m cache; the flat
        // total at the 5m rate is the historically correct price for them.
        let usage = Usage {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_input_tokens: 1_000_000,
            cache_read_input_tokens: 0,
            cache_creation: None,
        };
        assert_eq!(cost_of(&usage, "claude-fable-5"), 12.5);
    }

    #[test]
    fn fable_cost_all_buckets() {
        let usage = Usage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            cache_creation_input_tokens: 1_000_000,
            cache_read_input_tokens: 1_000_000,
            cache_creation: Some(CacheCreation {
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 1_000_000,
            }),
        };
        assert_eq!(cost_of(&usage, "claude-fable-5"), 10.0 + 50.0 + 20.0 + 1.0);
    }

    #[test]
    fn cost_matches_cc_alias_fixture() {
        let usage = Usage {
            input_tokens: 340,
            output_tokens: 84592,
            cache_creation_input_tokens: 303910,
            cache_read_input_tokens: 9_087_615,
            cache_creation: None,
        };
        let expected =
            (340.0 * 5.0 + 84592.0 * 25.0 + 303910.0 * 6.25 + 9_087_615.0 * 0.5) / 1_000_000.0;
        let got = cost_of(&usage, "claude-opus-4-7");
        assert!((got - expected).abs() < 1e-12, "{got} vs {expected}");
    }
}
