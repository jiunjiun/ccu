# Changelog

All notable changes to `ccu` will be documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Removed

- Intel Mac (`x86_64-apple-darwin`) target from the release matrix.
  Apple Silicon-only on the Mac side from the next tag onwards.
  Existing v0.1.0 / v0.1.1 Intel binaries are unaffected (GitHub Releases
  are immutable). Users on Intel Mac can still `cargo install cc-usage`.

## [0.1.1] — 2026-04-26

Infrastructure-only release. No behavior changes from `0.1.0`; same
binary, same numerical alignment with `ccusage`.

### Added

- Release pipeline now builds two additional targets:
  - `aarch64-unknown-linux-gnu` (Linux ARM64)
  - `x86_64-pc-windows-msvc` (Windows x64)
- Automated `cargo publish` to crates.io on tag push, gated on a
  Cargo.toml-version vs. tag-name check.

### Changed

- `actions/checkout` upgraded from v4 → v6 in CI / Release workflows.
- `tabled` upgraded from 0.16 → 0.20 (visual output unchanged).

## [0.1.0] — 2026-04-26

Initial release. A Rust port of [`ccusage`](https://github.com/ryoppippi/ccusage)'s
core data path, aligned to within ~0.003% on real datasets.

### Added

- **CLI subcommands**
  - `ccu` (no args) → daily table (default)
  - `ccu today [YYYY-MM-DD]` → bare cost (USD) for one day
  - `ccu month [YYYY-MM]` → bare cost for one month
  - `ccu daily [--json] [--compact] [--tz TZ]` → per-day breakdown
  - `ccu monthly [--json] [--compact] [--tz TZ]` → per-month breakdown
  - `ccu -v` / `ccu --version`, `ccu -h` / `ccu --help`

- **Pricing**
  - Four hardcoded tiers (Opus 4.5+ / Opus legacy / Sonnet / Haiku) matched
    by regex with `opus-4-[5-9]` checked before generic `opus`.
  - Numbers verified to align with `ccusage` to within $0.10 on $3000+ totals.

- **Pipeline**
  - Streaming `BufReader` + partial serde deserialize.
  - `(messageId, requestId)` dedup with null-hash retention (matches
    `ccusage`'s algorithm).
  - File iteration sorted by earliest-entry timestamp before dedup, also
    matching `ccusage`.
  - `<synthetic>` model rows filtered out at the aggregate stage.

- **Output formats**
  - Bare float for `today` / `month` (script-friendly).
  - `tabled`-rendered table for `daily` / `monthly`, with horizontal row
    separators between days.
  - ccusage-compatible camelCase JSON via `--json`.
  - `--compact` flag drops cache columns to a 5-column view.
  - Model names abbreviated for table display (`claude-haiku-4-5-20251001`
    → `haiku-4-5`); JSON keeps the full name.

- **Timezones**
  - Default to system local TZ.
  - `--tz <IANA>` accepts any chrono-tz zone (`UTC`, `Asia/Taipei`, …).

- **Terminal UX**
  - Border characters dimmed via 256-color ANSI on a TTY, plain when piped.
  - Honors [`NO_COLOR`](https://no-color.org).

- **Configuration**
  - `CCU_PROJECTS_DIR` env var overrides the default `~/.claude/projects`.

- **Distribution**
  - Published to crates.io as `cc-usage` (binary remains `ccu`).
  - GitHub Releases with prebuilt binaries for aarch64-apple-darwin,
    x86_64-apple-darwin, x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu,
    x86_64-pc-windows-msvc.
  - GitHub Actions: CI (test + clippy + fmt on macOS and Ubuntu), automated
    Release pipeline on tag push, automated crates.io publish on tag push,
    monthly Dependabot updates.

### Tests

- 73 tests total: 51 unit + 22 integration via `assert_cmd`.
- 4 fixtures: single_day, with_duplicates, multi_model, cross_tz_boundary.

[Unreleased]: https://github.com/jiunjiun/ccu/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/jiunjiun/ccu/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/jiunjiun/ccu/releases/tag/v0.1.0
