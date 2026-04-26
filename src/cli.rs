use clap::{ArgAction, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "ccu",
    version,
    about = "Claude Code usage/cost tracker aligned with ccusage",
    disable_version_flag = true,
    disable_help_subcommand = true
)]
// Unit-typed field below is a clap trick to rewire `--version` from the default
// short `-V` to lowercase `-v`; not a non-exhaustive pattern.
#[allow(clippy::manual_non_exhaustive)]
pub struct Cli {
    /// Print version information.
    #[arg(short = 'v', long = "version", action = ArgAction::Version)]
    _version: (),

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Print the total cost for a single day (default: today, local TZ).
    Today {
        /// Date in YYYY-MM-DD. Defaults to today in local timezone.
        date: Option<String>,
    },
    /// Print the total cost for a single month (default: current month, local TZ).
    Month {
        /// Month in YYYY-MM. Defaults to this month in local timezone.
        month: Option<String>,
    },
    /// Print a daily usage table across all known days.
    Daily {
        /// Emit ccusage-compatible JSON instead of a table.
        #[arg(long)]
        json: bool,
        /// Five-column table (Date, Models, Input, Output, Cost). Drops cache columns. Ignored when --json is set.
        #[arg(long)]
        compact: bool,
        /// IANA timezone (e.g., UTC, Asia/Taipei). Defaults to system local TZ.
        #[arg(long)]
        tz: Option<String>,
    },
    /// Print a monthly usage table across all known months.
    Monthly {
        /// Emit ccusage-compatible JSON instead of a table.
        #[arg(long)]
        json: bool,
        /// Five-column table (Month, Models, Input, Output, Cost). Drops cache columns. Ignored when --json is set.
        #[arg(long)]
        compact: bool,
        /// IANA timezone (e.g., UTC, Asia/Taipei). Defaults to system local TZ.
        #[arg(long)]
        tz: Option<String>,
    },
}
