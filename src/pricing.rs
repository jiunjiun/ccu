use regex::Regex;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    pub cache_write: f64,
    pub cache_read: f64,
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
                // Why: Opus 4.5 (Nov 2025) shifted to a $5 input tier; older
                // Opus models stay at $15. The trailing `-` separator after
                // the major version is load-bearing — without it, `\d{2,}`
                // would greedily match the date digits in legacy IDs like
                // `claude-3-opus-20240229` and misprice them at the new tier.
                pattern: Regex::new(r"opus-(4-([5-9]|\d{2,})|([5-9]|\d{2,})-)").unwrap(),
                price: Price {
                    input: 5.0,
                    output: 25.0,
                    cache_write: 6.25,
                    cache_read: 0.5,
                },
            },
            Tier {
                pattern: Regex::new(r"opus").unwrap(),
                price: Price {
                    input: 15.0,
                    output: 75.0,
                    cache_write: 18.75,
                    cache_read: 1.5,
                },
            },
            Tier {
                pattern: Regex::new(r"sonnet").unwrap(),
                price: Price {
                    input: 3.0,
                    output: 15.0,
                    cache_write: 3.75,
                    cache_read: 0.3,
                },
            },
            Tier {
                pattern: Regex::new(r"haiku").unwrap(),
                price: Price {
                    input: 1.0,
                    output: 5.0,
                    cache_write: 1.25,
                    cache_read: 0.1,
                },
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
    Price {
        input: 0.0,
        output: 0.0,
        cache_write: 0.0,
        cache_read: 0.0,
    }
}

pub fn cost_of(usage: &Usage, model: &str) -> f64 {
    let p = price_for(model);
    (usage.input_tokens as f64 * p.input
        + usage.output_tokens as f64 * p.output
        + usage.cache_creation_input_tokens as f64 * p.cache_write
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
                cache_write: 6.25,
                cache_read: 0.5
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
                cache_write: 18.75,
                cache_read: 1.5
            }
        );
    }

    #[test]
    fn sonnet_matches_sonnet_tier() {
        assert_eq!(price_for("claude-sonnet-4-5").input, 3.0);
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
                cache_write: 0.0,
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
        };
        assert_eq!(cost_of(&usage, "claude-opus-4-7"), 5.0 + 25.0 + 6.25 + 0.5);
    }

    #[test]
    fn cost_matches_cc_alias_fixture() {
        let usage = Usage {
            input_tokens: 340,
            output_tokens: 84592,
            cache_creation_input_tokens: 303910,
            cache_read_input_tokens: 9_087_615,
        };
        let expected =
            (340.0 * 5.0 + 84592.0 * 25.0 + 303910.0 * 6.25 + 9_087_615.0 * 0.5) / 1_000_000.0;
        let got = cost_of(&usage, "claude-opus-4-7");
        assert!((got - expected).abs() < 1e-12, "{got} vs {expected}");
    }
}
