use std::io::{self, IsTerminal};
use std::sync::OnceLock;

static ENABLED: OnceLock<bool> = OnceLock::new();

pub fn enabled() -> bool {
    *ENABLED.get_or_init(|| {
        if std::env::var_os("NO_COLOR").is_some() {
            return false;
        }
        if let Some(v) = std::env::var_os("CLICOLOR_FORCE") {
            if v != "0" {
                return true;
            }
        }
        io::stdout().is_terminal()
    })
}

fn wrap(code: &str, text: &str) -> String {
    if enabled() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

pub fn turquoise(text: &str) -> String { wrap("38;5;44", text) }
pub fn bold_turquoise(text: &str) -> String { wrap("1;38;5;44", text) }
pub fn crab(text: &str) -> String { wrap("38;5;208", text) }
pub fn bold_crab(text: &str) -> String { wrap("1;38;5;208", text) }
pub fn green(text: &str) -> String { wrap("38;5;76", text) }
pub fn bold_green(text: &str) -> String { wrap("1;38;5;76", text) }
pub fn purple(text: &str) -> String { wrap("38;5;141", text) }
pub fn bold_purple(text: &str) -> String { wrap("1;38;5;141", text) }
pub fn dim(text: &str) -> String { wrap("2", text) }
pub fn bold(text: &str) -> String { wrap("1", text) }

pub fn ok(text: &str) -> String {
    format!("{} {text}", green("[+]"))
}
pub fn info(text: &str) -> String {
    format!("{} {text}", turquoise("[>]"))
}
pub fn note(text: &str) -> String {
    format!("{} {text}", purple("[*]"))
}
pub fn warn(text: &str) -> String {
    format!("{} {text}", crab("[!]"))
}
pub fn neg(text: &str) -> String {
    format!("{} {text}", dim("[-]"))
}

pub fn banner() {
    let title = "privcrab";
    let subtitle = "Network privacy hardening toolkit";
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    let link = "github.com/tc4dy";
    let line = "─".repeat(46);
    println!("{}", dim(&line));
    println!("   {}", bold_turquoise(title));
    println!("   {}", crab(subtitle));
    println!("{}", dim(&line));
    println!("   {} {} {}", bold_purple(&version), dim("|"), turquoise(link));
    println!();
}