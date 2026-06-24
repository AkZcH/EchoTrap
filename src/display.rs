// src/display.rs
//
// HERALD-style terminal output for EchoTrap.
// All output goes through these functions — never println! directly.
//
// Color scheme:
//   cyan    — header, version, labels
//   green   — success / ok (✓)
//   yellow  — warnings / alerts (!)
//   red     — errors / critical (⚡)
//   dim     — secondary info (·)

use owo_colors::OwoColorize;

// ── Header block ──────────────────────────────────────────────────────────────

/// Print the startup header block, matching the HERALD [TOOL vX.Y.Z] style.
pub fn print_header(version: &str) {
    println!();
    println!("  {}", format!("[EchoTrap v{version}]").cyan().bold());
    println!("  {}", "self-rebuilding TCP honeypot".dimmed());
    println!();
}

/// Print a single init field: "  label   value"
pub fn print_field(label: &str, value: &str) {
    println!(
        "  {:<12} {}",
        label.dimmed(),
        value.white().bold()
    );
}

// ── Event lines ───────────────────────────────────────────────────────────────

/// ✓ green — successful bind, migration complete, decoy active, shutdown clean
pub fn ok(msg: &str) {
    println!("  {} {}", "✓".green().bold(), msg.white());
}

/// · dim — informational, accepted connection, secondary status
pub fn info(msg: &str) {
    println!("  {} {}", "·".dimmed(), msg.dimmed());
}

/// ! yellow — alert, scan suspected, migration triggered
pub fn warn(msg: &str) {
    println!("  {} {}", "!".yellow().bold(), msg.yellow());
}

/// ⚡ red — bind failure, error, forced exit
pub fn error(msg: &str) {
    println!("  {} {}", "⚡".red().bold(), msg.red());
}

/// Separator line — used between sections
pub fn separator() {
    println!("  {}", "─".repeat(48).dimmed());
}