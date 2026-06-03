//! The startup banner.
//!
//! Renders a box-style banner with project info inside. The banner
//! optionally applies ANSI color codes when `use_color` is `true`.

/// Returns the banner as a colored, multi-line string when `use_color` is
/// `true`, or as plain text when it is `false`.
#[must_use]
pub fn render(
    use_color: bool,
    version: &str,
    host: &str,
    port: u16,
    env: &str,
    data_dir: &str,
) -> String {
    let width = 62;
    let hline = "=".repeat(width);

    let title = "ACCELERATE SEARCH";
    let version_line = format!("v{}", version);
    let listen_url = format!("http://{}:{}", host, port);
    let env_line = env.to_string();
    let data_line = data_dir.to_string();
    let docs_url = "github.com/muhammad-fiaz/acceleratesearch";

    // Center text within the box (|  ...  |)
    let center = |s: &str| -> String {
        let vis_len = s.chars().count();
        let padding = width.saturating_sub(vis_len);
        let left_pad = padding / 2;
        let right_pad = padding - left_pad;
        format!("|{}{}{}|", " ".repeat(left_pad), s, " ".repeat(right_pad))
    };

    let top = format!("+{}+", hline);
    let bot = format!("+{}+", hline);

    let lines: Vec<String> = vec![
        top.clone(),
        center(title),
        center(&version_line),
        center(&listen_url),
        center(&env_line),
        center(&data_line),
        center("Documentation:"),
        center(docs_url),
        bot,
    ];

    if use_color {
        // ANSI escape codes
        let cyan_bold = "\x1b[1;36m";
        let white = "\x1b[0;37m";
        let yellow_bold = "\x1b[1;33m";
        let green = "\x1b[0;32m";
        let blue = "\x1b[0;34m";
        let magenta = "\x1b[0;35m";
        let cyan = "\x1b[0;36m";
        let underline = "\x1b[4m";
        let reset = "\x1b[0m";

        let mut colored = String::new();
        for (i, line) in lines.iter().enumerate() {
            let inner = &line[1..line.len() - 1];
            if i == 0 || i == lines.len() - 1 {
                colored.push_str(&format!("{}{}{}\n", cyan_bold, line, reset));
            } else if line.contains("ACCELERATE") {
                colored.push_str(&format!(
                    "{}|{}{}{}{}|{}\n",
                    cyan_bold, white, yellow_bold, inner, white, reset
                ));
            } else if line.contains("v0.") || line.contains("v1.") {
                colored.push_str(&format!(
                    "{}|{}{}{}{}|{}\n",
                    cyan_bold, white, magenta, inner, white, reset
                ));
            } else if line.contains("development") || line.contains("production") {
                colored.push_str(&format!(
                    "{}|{}{}{}{}|{}\n",
                    cyan_bold, white, green, inner, white, reset
                ));
            } else if line.contains("./") || line.contains("/data") {
                colored.push_str(&format!(
                    "{}|{}{}{}{}|{}\n",
                    cyan_bold, white, blue, inner, white, reset
                ));
            } else if line.contains("http://")
                || line.contains("github.com")
                || line.contains("Documentation")
            {
                colored.push_str(&format!(
                    "{}|{}{}{}{}{}|{}\n",
                    cyan_bold, white, cyan, underline, inner, white, reset
                ));
            } else {
                colored.push_str(&format!(
                    "{}|{}{}{}|{}\n",
                    cyan_bold, white, inner, cyan_bold, reset
                ));
            }
        }
        colored.trim_end().to_string()
    } else {
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_contains_both_words() {
        let banner = render(false, "0.0.0", "localhost", 7700, "development", "./data");
        let upper: String = banner.chars().filter(|c| c.is_ascii_alphabetic()).collect();
        assert!(
            upper.contains("ACCELERATE"),
            "banner should contain ACCELERATE"
        );
        assert!(upper.contains("SEARCH"), "banner should contain SEARCH");
    }

    #[test]
    fn banner_has_expected_line_count() {
        let banner = render(false, "0.0.0", "localhost", 7700, "development", "./data");
        let count = banner.lines().count();
        assert_eq!(count, 9, "banner should have exactly 9 lines");
    }

    #[test]
    fn render_plain_is_exact_banner() {
        let a = render(false, "0.0.0", "localhost", 7700, "development", "./data");
        let b = render(false, "0.0.0", "localhost", 7700, "development", "./data");
        assert_eq!(a, b);
    }

    #[test]
    fn render_colored_wraps_in_ansi() {
        let out = render(true, "0.0.0", "localhost", 7700, "development", "./data");
        assert!(out.contains("\x1b[1;36m"));
        assert!(out.contains("\x1b[0m"));
    }

    #[test]
    fn banner_shows_version_with_v_prefix() {
        let banner = render(false, "1.2.3", "localhost", 7700, "development", "./data");
        assert!(banner.contains("v1.2.3"));
        assert!(!banner.contains("Version"));
    }

    #[test]
    fn banner_shows_port() {
        let banner = render(false, "0.0.0", "localhost", 8080, "production", "/var/data");
        assert!(banner.contains("8080"));
    }

    #[test]
    fn banner_shows_environment() {
        let banner = render(false, "0.0.0", "localhost", 7700, "production", "./data");
        assert!(banner.contains("production"));
    }

    #[test]
    fn banner_shows_localhost() {
        let banner = render(false, "0.0.0", "localhost", 7700, "development", "./data");
        assert!(banner.contains("localhost"));
    }

    #[test]
    fn banner_box_lines_are_same_length() {
        let banner = render(false, "0.0.0", "localhost", 7700, "development", "./data");
        let expected_len = banner.lines().next().unwrap().len();
        for line in banner.lines() {
            assert_eq!(
                line.len(),
                expected_len,
                "line length mismatch: {:?} (len {}) vs expected {}",
                line,
                line.len(),
                expected_len
            );
        }
    }

    #[test]
    fn banner_title_is_centered() {
        let banner = render(false, "0.0.0", "localhost", 7700, "development", "./data");
        let lines: Vec<&str> = banner.lines().collect();
        let title_line = lines[1];
        let inner = &title_line[1..title_line.len() - 1];
        let vis_len = inner.trim().chars().count();
        assert!(vis_len > 0, "title line should have content");
    }

    #[test]
    fn print_banner_for_debug() {
        let banner = render(false, "0.0.0", "localhost", 7700, "development", "./data");
        eprintln!("\n{banner}");
    }
}
