use anyhow::Result;
use log::{debug, info};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub struct Scanner;

impl Scanner {
    /// Probe a single IP and port with a short timeout.
    pub fn probe_tcp_port(ip: &str, port: u16, timeout: Duration) -> bool {
        if let Ok(ip_addr) = ip.parse::<IpAddr>() {
            let socket_addr = SocketAddr::new(ip_addr, port);
            TcpStream::connect_timeout(&socket_addr, timeout).is_ok()
        } else {
            false
        }
    }

    /// Auto-detect the local IPv4 address and /24 subnet base (e.g. "192.168.1").
    pub fn detect_local_subnet_base() -> Option<(String, Ipv4Addr)> {
        // Try `ip -4 -o addr show scope global`
        if let Ok(output) = Command::new("ip")
            .args(["-4", "-o", "addr", "show", "scope", "global"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    // Line format: "2: wlo1    inet 192.168.1.38/24 brd ..."
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if let Some(pos) = parts.iter().position(|&p| p == "inet") {
                        if let Some(cidr) = parts.get(pos + 1) {
                            if let Some((ip_str, _mask)) = cidr.split_once('/') {
                                if let Ok(ipv4) = ip_str.parse::<Ipv4Addr>() {
                                    let octets = ipv4.octets();
                                    // Skip link-local or loopback
                                    if !ipv4.is_loopback() && !ipv4.is_link_local() {
                                        let base = format!("{}.{}.{}", octets[0], octets[1], octets[2]);
                                        return Some((base, ipv4));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Fallback: try connecting a dummy UDP socket to find the routing source IP
        if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
            if socket.connect("1.1.1.1:80").is_ok() {
                if let Ok(local_addr) = socket.local_addr() {
                    if let IpAddr::V4(ipv4) = local_addr.ip() {
                        let octets = ipv4.octets();
                        let base = format!("{}.{}.{}", octets[0], octets[1], octets[2]);
                        return Some((base, ipv4));
                    }
                }
            }
        }

        None
    }

    /// Scan a /24 subnet for open ADB ports (default 5555).
    /// If `quick_check_ip` is provided, tests it first.
    pub fn scan_subnet_for_adb(
        port: u16,
        subnet_override: Option<&str>,
        quick_check_ip: Option<&str>,
    ) -> Result<Vec<String>> {
        let probe_timeout = Duration::from_millis(300);

        // 1. Quick check the last known IP or specific target first
        if let Some(ip) = quick_check_ip {
            if !ip.is_empty() && ip != "auto" {
                debug!("Quick-checking target IP {}:{}", ip, port);
                if Self::probe_tcp_port(ip, port, Duration::from_millis(400)) {
                    info!("Target IP {}:{} responded immediately!", ip, port);
                    return Ok(vec![ip.to_string()]);
                }
            }
        }

        // 2. Determine the list of IPs to scan
        let (subnet_base, my_ip) = if let Some(override_cidr) = subnet_override {
            let base = if let Some((ip_part, _)) = override_cidr.split_once('/') {
                let octets: Vec<&str> = ip_part.split('.').collect();
                if octets.len() >= 3 {
                    format!("{}.{}.{}", octets[0], octets[1], octets[2])
                } else {
                    "192.168.1".to_string()
                }
            } else {
                "192.168.1".to_string()
            };
            (base, None)
        } else {
            match Self::detect_local_subnet_base() {
                Some((base, ip)) => (base, Some(ip)),
                None => {
                    info!("Could not detect local subnet, defaulting to 192.168.1.0/24");
                    ("192.168.1".to_string(), None)
                }
            }
        };

        info!("Scanning subnet {}.1-254 on port {}...", subnet_base, port);

        let mut candidate_ips = Vec::new();
        for i in 1..=254 {
            let ip_str = format!("{}.{}", subnet_base, i);
            if let Some(local_ip) = my_ip {
                if ip_str == local_ip.to_string() {
                    continue; // Skip scanning self
                }
            }
            candidate_ips.push(ip_str);
        }

        // 3. Multithreaded port probe across candidate IPs
        let (tx, rx) = mpsc::channel();
        let chunk_size = (candidate_ips.len() + 15) / 16;

        thread::scope(|s| {
            for chunk in candidate_ips.chunks(chunk_size) {
                let tx = tx.clone();
                s.spawn(move || {
                    for ip in chunk {
                        if Self::probe_tcp_port(ip, port, probe_timeout) {
                            let _ = tx.send(ip.clone());
                        }
                    }
                });
            }
        });
        drop(tx);

        let mut discovered = Vec::new();
        while let Ok(ip) = rx.recv() {
            info!("Discovered device with ADB port {} open at {}", port, ip);
            discovered.push(ip);
        }

        discovered.sort();
        Ok(discovered)
    }
}
