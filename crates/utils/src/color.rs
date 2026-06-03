//! Tiny ANSI color helpers for terminal output.
//!
//! The helpers in this module are deliberately minimal. They wrap a string
//! in the appropriate escape sequences when colors are enabled, and return
//! the input unchanged when they are not. This makes it cheap to call them
//! unconditionally on hot paths.
//!
//! Colors can be disabled at the process level by setting the
//! `NO_COLOR` environment variable to a non-empty value, or by piping
//! stdout (detected via [`std::io::IsTerminal`]).

/// ANSI escape sequence that resets all attributes.
pub const RESET: &str = "\x1b[0m";

/// Bright cyan prefix (`\x1b[1;36m`).
pub const BOLD_CYAN: &str = "\x1b[1;36m";
/// Bright green prefix (`\x1b[1;32m`).
pub const BOLD_GREEN: &str = "\x1b[1;32m";
/// Bright yellow prefix (`\x1b[1;33m`).
pub const BOLD_YELLOW: &str = "\x1b[1;33m";
/// Bright red prefix (`\x1b[1;31m`).
pub const BOLD_RED: &str = "\x1b[1;31m";
/// Bright magenta prefix (`\x1b[1;35m`).
pub const BOLD_MAGENTA: &str = "\x1b[1;35m";
/// Dim grey prefix (`\x1b[2;37m`).
pub const DIM: &str = "\x1b[2;37m";
/// Bold prefix (`\x1b[1m`).
pub const BOLD: &str = "\x1b[1m";

/// Returns `true` if ANSI color escapes should be emitted on stdout.
///
/// Honors the standard `NO_COLOR` convention: if the environment variable
/// is set to a non-empty value, color is disabled. Otherwise, color is
/// enabled when stdout is a terminal.
#[must_use]
pub fn stdout_is_colored() -> bool {
    if let Ok(v) = std::env::var("NO_COLOR")
        && !v.is_empty()
    {
        return false;
    }
    std::io::IsTerminal::is_terminal(&std::io::stdout())
}

/// Wraps `s` in bold cyan when `enabled`, otherwise returns it unchanged.
#[must_use]
pub fn cyan(s: impl AsRef<str>, enabled: bool) -> String {
    wrap(s, BOLD_CYAN, enabled)
}

/// Wraps `s` in bold green when `enabled`.
#[must_use]
pub fn green(s: impl AsRef<str>, enabled: bool) -> String {
    wrap(s, BOLD_GREEN, enabled)
}

/// Wraps `s` in bold yellow when `enabled`.
#[must_use]
pub fn yellow(s: impl AsRef<str>, enabled: bool) -> String {
    wrap(s, BOLD_YELLOW, enabled)
}

/// Wraps `s` in bold red when `enabled`.
#[must_use]
pub fn red(s: impl AsRef<str>, enabled: bool) -> String {
    wrap(s, BOLD_RED, enabled)
}

/// Wraps `s` in bold magenta when `enabled`.
#[must_use]
pub fn magenta(s: impl AsRef<str>, enabled: bool) -> String {
    wrap(s, BOLD_MAGENTA, enabled)
}

/// Wraps `s` in dim grey when `enabled`.
#[must_use]
pub fn dim(s: impl AsRef<str>, enabled: bool) -> String {
    wrap(s, DIM, enabled)
}

/// Wraps `s` in bold when `enabled`.
#[must_use]
pub fn bold(s: impl AsRef<str>, enabled: bool) -> String {
    wrap(s, BOLD, enabled)
}

fn wrap(s: impl AsRef<str>, code: &str, enabled: bool) -> String {
    if enabled {
        format!("{code}{}{RESET}", s.as_ref())
    } else {
        s.as_ref().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_disabled_is_plain() {
        assert_eq!(red("boom", false), "boom");
        assert_eq!(green("ok", false), "ok");
    }

    #[test]
    fn wrap_enabled_adds_ansi() {
        let out = red("boom", true);
        assert!(out.starts_with(BOLD_RED));
        assert!(out.ends_with(RESET));
        assert!(out.contains("boom"));
    }

    #[test]
    fn all_helpers_round_trip() {
        let s = "x";
        assert!(cyan(s, true).contains(s));
        assert!(green(s, true).contains(s));
        assert!(yellow(s, true).contains(s));
        assert!(red(s, true).contains(s));
        assert!(magenta(s, true).contains(s));
        assert!(dim(s, true).contains(s));
        assert!(bold(s, true).contains(s));
    }

    #[test]
    #[allow(unsafe_code)]
    fn no_color_env_disables_color() {
        // SAFETY: tests in this module run in a single thread; setting
        // `NO_COLOR` only affects this process. We also save the prior
        // value to restore it after the assertion.
        let prior = std::env::var("NO_COLOR").ok();
        unsafe {
            std::env::set_var("NO_COLOR", "1");
        }
        assert!(!stdout_is_colored());
        unsafe {
            match prior {
                Some(v) => std::env::set_var("NO_COLOR", v),
                None => std::env::remove_var("NO_COLOR"),
            }
        }
    }
}
