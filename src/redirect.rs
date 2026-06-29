// src/redirect.rs
//
// Zero-downtime port migration via nftables REDIRECT.
//
// When EchoTrap migrates from port A to port B:
//   1. Add nftables REDIRECT rule: tcp dport A redirect to :B
//   2. Kernel transparently forwards all connections on A to B
//   3. In-flight connections complete normally
//   4. After REDIRECT_DURATION, remove the rule
//
// This eliminates the 200ms race window in the existing decoy approach.
//
// Requirements: Linux kernel 5.8+, CAP_NET_ADMIN (or root), nft in PATH.
// On non-Linux or when nft is unavailable, all functions are no-ops and
// the caller falls back to the existing decoy mechanism transparently.

use std::time::Duration;
use tokio::process::Command;
use tracing::{info, warn};

const REDIRECT_DURATION_SECS: u64 = 30;
const NFT_TABLE: &str = "echotrap";
const NFT_CHAIN: &str = "prerouting";

/// Enable loopback NAT — required for nftables REDIRECT to work on 127.0.0.1.
/// Sets net.ipv4.conf.lo.route_localnet=1 via sysctl.
/// No-op on non-Linux or if sysctl fails (logged as warning).
pub async fn enable_loopback_nat() {
    #[cfg(target_os = "linux")]
    {
        let result = Command::new("sysctl")
            .args(["-w", "net.ipv4.conf.lo.route_localnet=1"])
            .output()
            .await;

        match result {
            Ok(out) if out.status.success() => {
                info!("[nft] Enabled net.ipv4.conf.lo.route_localnet=1 for loopback NAT");
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                warn!("[nft] Failed to set route_localnet: {stderr}");
            }
            Err(e) => {
                warn!("[nft] sysctl not available: {e} — REDIRECT on loopback may not work");
            }
        }
    }
}

/// Add a REDIRECT rule: TCP traffic on `from_port` is redirected to `to_port`.
pub async fn add_redirect(from_port: u16, to_port: u16) -> std::io::Result<()> {
    if !nft_available().await {
        return Ok(());
    }

    // Ensure loopback NAT is enabled — required for REDIRECT on 127.0.0.1.
    enable_loopback_nat().await;

    // Create table (idempotent).
    nft(&["add", "table", "ip", NFT_TABLE]).await?;

    // Create NAT prerouting chain (idempotent).
    // Each argument is passed separately — no shell splitting of braces/semicolons.
    nft(&[
        "add",
        "chain",
        "ip",
        NFT_TABLE,
        NFT_CHAIN,
        "{ type nat hook prerouting priority dstnat ; }",
    ])
    .await?;

    // Add redirect rule.
    nft(&[
        "add",
        "rule",
        "ip",
        NFT_TABLE,
        NFT_CHAIN,
        "tcp",
        "dport",
        &from_port.to_string(),
        "redirect",
        "to",
        &format!(":{to_port}"),
    ])
    .await?;

    info!("[nft] REDIRECT :{from_port} → :{to_port} active");
    Ok(())
}

/// Remove all EchoTrap redirect rules by flushing the table.
pub async fn remove_redirect(from_port: u16) -> std::io::Result<()> {
    if !nft_available().await {
        return Ok(());
    }

    nft(&["flush", "table", "ip", NFT_TABLE]).await?;
    info!("[nft] REDIRECT for :{from_port} removed");
    Ok(())
}

/// Spawn a task: add REDIRECT, wait REDIRECT_DURATION, remove it.
/// Falls back to decoy-only if nft fails.
pub fn spawn_redirect_then_decoy(
    from_port: u16,
    to_port: u16,
    decoy_banner: &'static str,
    decoy_duration: Duration,
) {
    tokio::spawn(async move {
        if let Err(e) = add_redirect(from_port, to_port).await {
            warn!("[nft] Failed to add REDIRECT :{from_port} → :{to_port}: {e}");
            // Fall back to decoy-only with 200ms settle window.
            tokio::time::sleep(Duration::from_millis(200)).await;
            crate::migration::spawn_decoy(from_port, decoy_banner, decoy_duration);
            return;
        }

        tokio::time::sleep(Duration::from_secs(REDIRECT_DURATION_SECS)).await;

        if let Err(e) = remove_redirect(from_port).await {
            warn!("[nft] Failed to remove REDIRECT for :{from_port}: {e}");
        }

        info!("[nft] REDIRECT for :{from_port} expired");
    });
}

/// Run `nft` with explicit separate arguments (no shell splitting).
async fn nft(args: &[&str]) -> std::io::Result<()> {
    let output = Command::new("nft").args(args).output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // "File exists" / "already exists" means table/chain already present — ok.
        if stderr.contains("File exists") || stderr.contains("already exists") {
            return Ok(());
        }
        return Err(std::io::Error::other(format!(
            "nft {}: {stderr}",
            args.join(" ")
        )));
    }

    Ok(())
}

/// Check whether `nft` is in PATH and executable.
async fn nft_available() -> bool {
    #[cfg(not(target_os = "linux"))]
    return false;

    #[cfg(target_os = "linux")]
    Command::new("nft")
        .arg("--version")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}
