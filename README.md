# ccu

[![CI](https://github.com/jiunjiun/ccu/actions/workflows/ci.yml/badge.svg)](https://github.com/jiunjiun/ccu/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Rust](https://img.shields.io/badge/rust-2021-orange.svg)

> A fast, local Claude Code usage and cost tracker — Rust port of [`ccusage`](https://github.com/ryoppippi/ccusage)'s core, optimized for daily terminal use.

`ccu` reads your `~/.claude/projects/**/*.jsonl` files and prints token / cost summaries by day or month. Numbers align with `ccusage` to within ~0.003% (cent-level on $3,000+ totals). Designed to replace shell + jq scripts that take ~10s on 1+ GB of data with a single Rust binary that finishes in under 1.5s.

---

## Features

- **Fast.** Streams JSONL with `BufReader`, partial serde deserialize, single-pass dedup. ~1.3s on 1.3 GB / 166k entries (Apple Silicon).
- **`ccusage`-compatible.** Same dedup rule (`(messageId, requestId)`, largest snapshot wins, null-hash retention), same pricing tiers, same JSON schema (camelCase). Drop-in replacement in scripts.
- **Local-only.** No network, no daemon, no telemetry. Reads files; prints text.
- **TZ-aware.** Bucket by `--tz` (any IANA zone) or system local. Same data, different days at midnight boundaries.
- **Three output modes.** Bare float (`today`/`month`), table (`daily`/`monthly`), or JSON (`--json`).
- **Polite to your terminal.** Borders dim to grey on a TTY, plain when piped, respects [`NO_COLOR`](https://no-color.org).

---

## Install

The crate is published as **`cc-usage`**; the binary is **`ccu`**.

### From crates.io (recommended)

```bash
cargo install cc-usage
```

Compiles from source on your machine (~30–60 s on Apple Silicon),
installs to `~/.cargo/bin/ccu`. If `~/.cargo/bin` isn't on your `PATH`,
add it to your shell rc:

```bash
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

### From a prebuilt binary (no Rust toolchain needed)

Pick the asset for your platform from the [latest release](https://github.com/jiunjiun/ccu/releases/latest):

```bash
# Apple Silicon Mac
curl -L https://github.com/jiunjiun/ccu/releases/latest/download/ccu-aarch64-apple-darwin.tar.gz | tar xz
sudo install ccu-aarch64-apple-darwin/ccu /usr/local/bin/ccu

# Linux x86_64
curl -L https://github.com/jiunjiun/ccu/releases/latest/download/ccu-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo install ccu-x86_64-unknown-linux-gnu/ccu /usr/local/bin/ccu

# Linux ARM64
curl -L https://github.com/jiunjiun/ccu/releases/latest/download/ccu-aarch64-unknown-linux-gnu.tar.gz | tar xz
sudo install ccu-aarch64-unknown-linux-gnu/ccu /usr/local/bin/ccu

# Windows x64 — extract ccu-x86_64-pc-windows-msvc.zip and put ccu.exe on PATH
```

### From source

```bash
git clone https://github.com/jiunjiun/ccu ~/git/ccu
cd ~/git/ccu
cargo install --path .
```

### Verify

```bash
ccu --version       # ccu 0.1.1
ccu                 # prints today's daily table
```

### Update

```bash
cargo install cc-usage --force
```

Or, if you installed from source, `cd ~/git/ccu && git pull && cargo install --path . --force`.

### Uninstall

```bash
cargo uninstall cc-usage
```

---

## Usage

### Subcommands

| Command | Output | Use case |
|---------|--------|----------|
| `ccu` | Daily table | Quick glance — same as `ccu daily` |
| `ccu today [DATE]` | Bare float | Pipe into another tool / shell prompt |
| `ccu month [YYYY-MM]` | Bare float | Same, but for a whole month |
| `ccu daily` | Table | Per-day breakdown across all time |
| `ccu monthly` | Table | Per-month breakdown |
| `ccu --help` | Help text | List subcommands + flags |
| `ccu -v` | Version | `ccu 0.1.0` |

`DATE` defaults to today, `YYYY-MM` to the current month — both in local timezone.

### Flags (apply to `daily` and `monthly`)

| Flag | Effect |
|------|--------|
| `--json` | Emit a `ccusage`-compatible JSON object instead of a table. |
| `--compact` | Five-column table (Date, Models, Input, Output, Cost). Drops cache columns. JSON unaffected. |
| `--tz <IANA>` | Bucket entries in this timezone (e.g., `UTC`, `Asia/Taipei`). Default: system local. |

### Examples

```bash
# One-off cost lookups
$ ccu today
8.559744999999998
$ ccu month 2026-03
666.4014687000001

# Pipe today's cost into a shell prompt
$ printf '$%.2f\n' "$(ccu today)"
$8.56

# Daily breakdown, table
$ ccu daily | head
┌────────────┬───────────┬────────┬─────────┬──────────────┬───────────┬──────────────┬────────────┐
│ Date       │ Models    │ Input  │ Output  │ Cache Create │ Cache Read│ Total Tokens │ Cost (USD) │
├────────────┼───────────┼────────┼─────────┼──────────────┼───────────┼──────────────┼────────────┤
│ 2026-04-24 │ opus-4-7  │ 340    │ 84,592  │ 303,910      │ 9,087,615 │ 9,476,457    │ $8.56      │
...

# Compact daily (five columns only)
$ ccu daily --compact | head -3

# JSON, drop into another tool
$ ccu daily --json | jq '.daily[-7:] | map(.totalCost) | add'

# Different timezone — boundary entries land on different days
$ ccu daily --tz UTC --json | jq '.daily | length'
$ ccu daily --tz Asia/Taipei --json | jq '.daily | length'
```

---

## Output formats

### Bare (today / month)

```
8.559744999999998
```

A single line: the number, in USD, no `$`, no rounding. Designed to be parsed.

### Table (daily / monthly)

A `tabled`-rendered ASCII table with one row per day (or month) plus a `Total` row. Default style is `modern` (horizontal separators between rows). Borders are dimmed via 256-color ANSI when stdout is a TTY; raw text when piped.

Model names are abbreviated for display (`claude-haiku-4-5-20251001` → `haiku-4-5`). Full names are preserved in JSON output.

### JSON (`--json`)

```json
{
  "daily": [
    {
      "date": "2026-04-24",
      "inputTokens": 340,
      "outputTokens": 84592,
      "cacheCreationTokens": 303910,
      "cacheReadTokens": 9087615,
      "totalTokens": 9476457,
      "totalCost": 8.559744999999998,
      "modelsUsed": ["claude-opus-4-7"],
      "modelBreakdowns": [
        {
          "modelName": "claude-opus-4-7",
          "inputTokens": 340,
          "outputTokens": 84592,
          "cacheCreationTokens": 303910,
          "cacheReadTokens": 9087615,
          "cost": 8.559744999999998
        }
      ]
    }
  ]
}
```

Schema is byte-compatible with `ccusage daily --json` / `ccusage monthly --json` — `monthly` swaps the top-level key and the per-row `date` field for `month`.

---

## Pricing

Four hardcoded tiers (USD per 1M tokens), regex-matched against the model name in order:

| Pattern | Input | Output | Cache Write | Cache Read |
|---------|-------|--------|-------------|------------|
| `opus-4-[5-9]` | 5.00 | 25.00 | 6.25 | 0.50 |
| `opus` | 15.00 | 75.00 | 18.75 | 1.50 |
| `sonnet` | 3.00 | 15.00 | 3.75 | 0.30 |
| `haiku` | 1.00 | 5.00 | 1.25 | 0.10 |

`opus-4-[5-9]` is deliberately checked first; `claude-opus-4-5`, `4-6`, `4-7` all match it. Anything else with `opus` (e.g., `claude-3-opus`) falls through to the legacy tier.

Anthropic doesn't currently advertise prices for `opus-4-0` or `opus-4-1` separately; if you've used either model, file an issue with their actual price and the regex can be widened.

---

## Configuration

### Environment variables

| Var | Effect |
|-----|--------|
| `CCU_PROJECTS_DIR` | Override the JSONL scan root. Defaults to `~/.claude/projects`. Mostly useful for tests. |
| `NO_COLOR` | If set (any value), disables border dimming even in a TTY. Honors the [no-color.org](https://no-color.org) standard. |

Color is also automatically disabled when stdout is piped (file, less, another command).

---

## Comparison with `ccusage`

`ccu` is a deliberately scoped subset of `ccusage`. If you need the full feature surface, keep using `ccusage`.

|  | `ccu` | `ccusage` |
|--|-------|-----------|
| `daily`, `monthly`, JSON output | ✅ | ✅ |
| `today`, `month` (one-line cost) | ✅ | — |
| `weekly`, `session`, `blocks`, `statusline` | — | ✅ |
| `--breakdown`, `--instances`, `--project` | — | ✅ |
| `--since`, `--until` | — | ✅ |
| Live pricing fetch (LiteLLM API) | — | ✅ (with `--offline` opt-out) |
| Single static binary | ✅ | — (Node.js / npx) |
| Cold-start time | <50 ms | ~700 ms (npm spawn) |
| Full-scan time on 1.3 GB | ~1.3 s | ~3 s |
| Numerical alignment | within ~0.003% | ground truth |

The residual <0.01% gap traces to file-iteration tie-breaking when multiple JSONL files share the same earliest entry timestamp; both tools dedup by `(messageId, requestId)` with the same algorithm, but tie-breaking on file order isn't perfectly reproducible across `node` glob and `walkdir`. Cent-level discrepancies on a multi-thousand-dollar total.

---

## Verifying against `ccusage`

```bash
# scripts/compare_with_ccusage.sh diffs sorted JSON outputs of both tools
./scripts/compare_with_ccusage.sh | head -40
```

The script uses `npx ccusage@latest --offline`, so no global install of `ccusage` is needed.

---

## Development

```bash
# Run tests (unit + integration with assert_cmd)
cargo test

# Lint
cargo clippy --all-targets -- -D warnings

# Format check
cargo fmt --check

# Release build
cargo build --release       # binary at target/release/ccu

# Performance smoke
time ./target/release/ccu daily --json > /dev/null
```

73 tests cover pricing regex ordering, dedup with null-hash, timezone bucketing across midnight, JSON schema shape, CLI surface (subcommands, flags, error paths), and the table renderer's column structure / Total row / model-name shortening.

### Project layout

```
src/
├── main.rs          # clap dispatch
├── lib.rs           # run_today / run_month / run_daily / run_monthly + load_entries
├── cli.rs           # clap Parser + Commands
├── pricing.rs       # regex-ordered price table + cost_of
├── entry.rs         # UsageEntry serde + parse_line + parse_file
├── discover.rs      # walkdir scan, env-overridable root
├── dedup.rs         # (msgId, reqId) dedup with null-hash retention
├── aggregate.rs     # group_by_day / group_by_month → Bucket
├── timezone.rs      # enum Timezone { Local, Named(Tz) }
└── render/
    ├── bare.rs      # today / month one-liner
    ├── table.rs     # tabled-based daily/monthly + dim_border_chars + short_model_name
    └── json.rs      # ccusage-schema serde structs

tests/
├── cli_test.rs      # 22 integration tests via assert_cmd
└── fixtures/        # 4 JSONL fixtures: single_day, with_duplicates, multi_model, cross_tz_boundary
```

---

## Acknowledgments

- [`ccusage`](https://github.com/ryoppippi/ccusage) by [@ryoppippi](https://github.com/ryoppippi) — JSON schema reference and pricing source of truth. `ccu` is essentially a Rust port of its core data path.
- The `tabled`, `clap`, `chrono-tz`, `serde`, and `walkdir` crates do most of the actual work.

---

## License

[MIT](LICENSE) © 2026 jiunjiun
