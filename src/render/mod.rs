pub mod bare;
pub mod chart;
pub mod json;
pub mod table;

/// Wrap each contiguous run of chars satisfying `predicate` with ANSI escapes
/// (`open` before the run, `close` after) in a single pass. Shared by the
/// dim-border and bar-color post-processors.
pub(crate) fn wrap_char_runs<F: Fn(char) -> bool>(
    s: &str,
    predicate: F,
    open: &str,
    close: &str,
) -> String {
    let mut out = String::with_capacity(s.len() + 32);
    let mut in_run = false;
    for c in s.chars() {
        let is_match = predicate(c);
        match (is_match, in_run) {
            (true, false) => {
                out.push_str(open);
                in_run = true;
            }
            (false, true) => {
                out.push_str(close);
                in_run = false;
            }
            _ => {}
        }
        out.push(c);
    }
    if in_run {
        out.push_str(close);
    }
    out
}
