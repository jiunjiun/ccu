//! 256-color ANSI escapes shared by table borders, chart bars, top-cost
//! highlight, and the version-check banner. Pure constants; callers decide
//! when to apply (typically only when the target stream is a TTY and
//! `NO_COLOR` is unset).

/// Borders + banner: cool grey #6c6c6c, dimmer than ANSI [2m.
pub const DIM_GREY: &str = "\x1b[38;5;242m";
/// Chart bars: cool azure #5f87ff, readable on dark and light backgrounds.
pub const BAR_AZURE: &str = "\x1b[38;5;69m";
/// Top-cost cell: bold amber #ffaf00.
pub const HIGHLIGHT_AMBER: &str = "\x1b[1;38;5;214m";

/// Reset only the foreground color (preserves other attrs).
pub const RESET_FG: &str = "\x1b[39m";
/// Reset bold + foreground in one go (paired with `HIGHLIGHT_AMBER`).
pub const RESET_BOLD_FG: &str = "\x1b[22;39m";
