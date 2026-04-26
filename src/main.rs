use anyhow::Result;
use ccu::cli::{Cli, Commands};
use ccu::update::UpdateChecker;
use clap::Parser;
use std::time::Duration;

fn main() -> Result<()> {
    let cli = Cli::parse();

    // `ccu update` is itself the upgrade path — running a stale-version notice
    // alongside the upgrade banner is just noise, so skip the checker here.
    if let Some(Commands::Update) = cli.command {
        return ccu::update::run_update();
    }

    let checker = UpdateChecker::spawn();

    let result = match cli.command {
        // No subcommand → default to `daily` (full table, no flags).
        None => ccu::run_daily(false, false, None),
        Some(Commands::Today { date }) => ccu::run_today(date.as_deref()),
        Some(Commands::Month { month }) => ccu::run_month(month.as_deref()),
        Some(Commands::Daily { json, compact, tz }) => ccu::run_daily(json, compact, tz.as_deref()),
        Some(Commands::Monthly { json, compact, tz }) => {
            ccu::run_monthly(json, compact, tz.as_deref())
        }
        Some(Commands::Chart { days, tz }) => ccu::run_chart(days.unwrap_or(30), tz.as_deref()),
        Some(Commands::Update) => unreachable!("handled above"),
    };

    // Cache hit returns instantly; cache miss falls back to a short budget so
    // we don't stall the user waiting on GitHub API. The fetched result still
    // gets persisted for the next invocation.
    if let Some(latest) = checker.try_recv(Duration::from_millis(200)) {
        eprintln!("{}", ccu::update::format_banner(&latest));
    }

    result
}
