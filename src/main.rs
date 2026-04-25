use anyhow::Result;
use ccu::cli::{Cli, Commands};
use clap::Parser;

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        // No subcommand → default to `daily` (full table, no flags).
        None => ccu::run_daily(false, false, None),
        Some(Commands::Today { date }) => ccu::run_today(date.as_deref()),
        Some(Commands::Month { month }) => ccu::run_month(month.as_deref()),
        Some(Commands::Daily { json, compact, tz }) => ccu::run_daily(json, compact, tz.as_deref()),
        Some(Commands::Monthly { json, compact, tz }) => {
            ccu::run_monthly(json, compact, tz.as_deref())
        }
    }
}
