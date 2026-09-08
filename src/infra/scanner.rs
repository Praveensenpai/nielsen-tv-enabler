//! Network subnet port scanner for discovering Android TV ADB endpoints.

use anyhow::Result;
use log::info;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Local network scanner for finding open ADB ports.
pub struct Scanner;

impl Scanner {
    /// Probes a single IP and port with a short timeout.
    #[must_use]
    pub fn probe_tcp_port(ip: &str, port: u16, timeout: Duration) -> bool {
        let Ok(ip_addr) = ip.parse::<IpAddr>() else {
            return false;
        };
        let socket_addr = SocketAddr::new(ip_addr, port);
        TcpStream::connect_timeout(&socket_addr, timeout).is_ok()
    }

    /// Auto-detects the local IPv4 address and /24 subnet base (e.g. "192.168.1").
    #[must_use]
    pub fn detect_local_subnet_base() -> Option<(String, Ipv4Addr)> {
        if let Some(res) = detect_via_ip_command() {
            return Some(res);
        }
        detect_via_udp_probe()
    }

    /// Scans a /24 subnet for open ADB ports (default 5555).
    ///
    /// # Errors
    /// Returns an error if scanning threads fail.
    pub fn scan_subnet_for_adb(
        port: u16,
        subnet_override: Option<&str>,
        quick_check_ip: Option<&str>,
    ) -> Result<Vec<String>> {
        // 1. Quick check target IP first
        if let Some(ip) = quick_check_ip
            && !ip.is_empty()
            && ip != "auto"
            && Self::probe_tcp_port(ip, port, Duration::from_millis(1000))
        {
            info!("Target IP {ip}:{port} responded immediately!");
            return Ok(vec![ip.to_string()]);
        }

        let (subnet_base, my_ip) = resolve_subnet_range(subnet_override);
        info!("Scanning subnet {subnet_base}.1-254 on port {port}...");

        let candidate_ips = generate_candidate_ips(&subnet_base, my_ip);
        let discovered = probe_ips_multithreaded(&candidate_ips, port);
        Ok(discovered)
    }
}

/// Parses the output of `ip -4 -o addr show scope global`.
fn detect_via_ip_command() -> Option<(String, Ipv4Addr)> {
    let output = Command::new("ip")
        .args(["-4", "-o", "addr", "show", "scope", "global"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.lines().find_map(parse_ip_line)
}

/// Parses a single line from `ip -4 -o addr`.
fn parse_ip_line(line: &str) -> Option<(String, Ipv4Addr)> {
    let mut parts = line.split_whitespace();
    parts.position(|p| p == "inet")?;
    let cidr = parts.next()?;
    let (ip_str, _) = cidr.split_once('/')?;
    let ipv4 = ip_str.parse::<Ipv4Addr>().ok()?;

    if ipv4.is_loopback() || ipv4.is_link_local() {
        return None;
    }

    let [a, b, c, _] = ipv4.octets();
    Some((format!("{a}.{b}.{c}"), ipv4))
}

/// Fallback detection by connecting a dummy UDP socket towards public DNS.
fn detect_via_udp_probe() -> Option<(String, Ipv4Addr)> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:80").ok()?;
    let local_addr = socket.local_addr().ok()?;

    let IpAddr::V4(ipv4) = local_addr.ip() else {
        return None;
    };
    let [a, b, c, _] = ipv4.octets();
    Some((format!("{a}.{b}.{c}"), ipv4))
}

/// Resolves the subnet base string and own IP address.
fn resolve_subnet_range(subnet_override: Option<&str>) -> (String, Option<Ipv4Addr>) {
    if let Some(override_cidr) = subnet_override {
        if let Some((ip_part, _)) = override_cidr.split_once('/') {
            let octets: Vec<&str> = ip_part.split('.').collect();
            if octets.len() >= 3 {
                return (format!("{}.{}.{}", octets[0], octets[1], octets[2]), None);
            }
        }
        return ("192.168.1".to_string(), None);
    }

    if let Some((base, ip)) = Scanner::detect_local_subnet_base() {
        (base, Some(ip))
    } else {
        info!("Could not detect local subnet, defaulting to 192.168.1.0/24");
        ("192.168.1".to_string(), None)
    }
}

/// Generates a list of candidate IPs for a /24 subnet, skipping self.
fn generate_candidate_ips(subnet_base: &str, my_ip: Option<Ipv4Addr>) -> Vec<String> {
    let my_ip_str = my_ip.map(|ip| ip.to_string());
    let mut candidate_ips = Vec::with_capacity(254);

    for i in 1..=254 {
        let ip_str = format!("{subnet_base}.{i}");
        if my_ip_str.as_deref() == Some(&ip_str) {
            continue;
        }
        candidate_ips.push(ip_str);
    }
    candidate_ips
}

/// Multithreaded TCP port probe across candidate IPs.
fn probe_ips_multithreaded(candidate_ips: &[String], port: u16) -> Vec<String> {
    let probe_timeout = Duration::from_millis(300);
    let (tx, rx) = mpsc::channel();
    let chunk_size = candidate_ips.len().div_ceil(16);

    thread::scope(|s| {
        for chunk in candidate_ips.chunks(chunk_size) {
            let tx = tx.clone();
            s.spawn(move || {
                for ip in chunk {
                    if Scanner::probe_tcp_port(ip, port, probe_timeout) {
                        let _ = tx.send(ip.clone());
                    }
                }
            });
        }
    });
    drop(tx);

    let mut discovered = Vec::new();
    while let Ok(ip) = rx.recv() {
        info!("Discovered device with ADB port {port} open at {ip}");
        discovered.push(ip);
    }
    discovered.sort();
    discovered
}
