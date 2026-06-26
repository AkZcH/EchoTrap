// src/display.rs
//
// HERALD-style terminal output for EchoTrap.
//
// Color scheme:
//   cyan    — addresses, ports, values, key names
//   green   — success (✓)
//   yellow  — warnings / alerts (!)
//   red     — errors / critical (⚡)
//   dim     — labels, secondary info, separators
//   white   — primary message text
//   bold    — prefixes, header

use owo_colors::OwoColorize;

// ── Header block ──────────────────────────────────────────────────────────────

pub fn print_header(version: &str) {
    println!();
    println!("  {}", format!("[EchoTrap v{version}]").cyan().bold());
    println!("  {}", "self-rebuilding TCP honeypot".dimmed());
    println!();
}

/// Print an init field with cyan value: "  label        value"
pub fn print_field(label: &str, value: &str) {
    println!("  {:<12} {}", label.dimmed(), value.cyan().bold());
}

pub fn separator() {
    println!("  {}", "─".repeat(48).dimmed());
}

// ── Core event lines ──────────────────────────────────────────────────────────

/// ✓ green prefix — full message in white, with cyan highlights applied by caller
pub fn ok(msg: &str) {
    println!("  {} {}", "✓".green().bold(), colorize_message(msg));
}

/// · dim prefix — secondary/informational
pub fn info(msg: &str) {
    println!("  {} {}", "·".dimmed(), colorize_message(msg));
}

/// ! yellow — alert, full line yellow
pub fn warn(msg: &str) {
    println!("  {} {}", "!".yellow().bold(), colorize_message_warn(msg));
}

/// ⚡ red — error, full line red
pub fn error(msg: &str) {
    println!("  {} {}", "⚡".red().bold(), msg.red());
}

// ── Key=value inline pairs ────────────────────────────────────────────────────

/// Print a key=value pair inline with cyan value: "key=value"
/// Used for structured event details like "latency=31ms  port=9000"
#[allow(dead_code)]
pub fn kv(key: &str, value: &str) -> String {
    format!("{}={}", key.dimmed(), value.cyan().bold())
}

/// Print an indented detail line (secondary info under an event)
#[allow(dead_code)]
pub fn detail(msg: &str) {
    println!("    {} {}", "·".dimmed(), msg.dimmed());
}

/// Print a block label — used to group related output
#[allow(dead_code)]
pub fn block_label(msg: &str) {
    println!();
    println!("  {}", msg.dimmed());
}

// ── Semantic colorizer ────────────────────────────────────────────────────────
//
// Parses common EchoTrap message patterns and applies cyan to:
//   - Port numbers (:9000, :21629)
//   - IP:port addresses (127.0.0.1:50844)
//   - Key=value pairs embedded in messages
//   - Persona names
//   - Durations (30s, 10s)

pub fn colorize_message(msg: &str) -> String {
    colorize_inner(msg, false)
}

pub fn colorize_message_warn(msg: &str) -> String {
    colorize_inner(msg, true)
}

fn colorize_inner(msg: &str, yellow_base: bool) -> String {
    // We do a token-by-token pass. Tokens that look like addresses or
    // key=value pairs get cyan; everything else gets the base color.
    let mut result = String::with_capacity(msg.len() * 2);

    for token in msg.split_inclusive(' ') {
        let trimmed = token.trim_end_matches(' ');
        let trailing_space = if token.ends_with(' ') { " " } else { "" };

        let colored = if is_address(trimmed) || is_port(trimmed) || is_persona(trimmed) {
            format!("{}{}", trimmed.cyan().bold(), trailing_space)
        } else if let Some((k, v)) = trimmed.split_once('=') {
            // key=value pair — dim key, cyan value
            if !k.is_empty() && !v.is_empty() && k.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
                format!("{}={}{}", k.dimmed(), v.cyan().bold(), trailing_space)
            } else if yellow_base {
                format!("{}{}", trimmed.yellow(), trailing_space)
            } else {
                format!("{}{}", trimmed.white(), trailing_space)
            }
        } else if yellow_base {
            format!("{}{}", trimmed.yellow(), trailing_space)
        } else {
            format!("{}{}", trimmed.white(), trailing_space)
        };

        result.push_str(&colored);
    }

    result
}

/// Looks like an IP:port or 0.0.0.0:port
fn is_address(s: &str) -> bool {
    let s = s.trim_matches(&[',', '.', ')', '(', '[', ']'][..]);
    if let Some(colon) = s.rfind(':') {
        let port_part = &s[colon + 1..];
        let host_part = &s[..colon];
        return port_part.parse::<u16>().is_ok()
            && (host_part.contains('.') || host_part == "localhost");
    }
    false
}

/// Looks like :9000 (bare port reference)
fn is_port(s: &str) -> bool {
    let s = s.trim_matches(&[',', '.'][..]);
    s.starts_with(':') && s[1..].parse::<u16>().is_ok()
}

/// Known persona names
fn is_persona(s: &str) -> bool {
    matches!(
        s.trim_matches(&[',', '.', '[', ']'][..]),
        "ssh" | "http" | "redis" | "raw" | "SSH" | "HTTP" | "Redis" | "Raw"
    )
}