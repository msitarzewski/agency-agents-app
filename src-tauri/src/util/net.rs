//! Network host SSRF guard — the public-host / public-IP allowlist.
//!
//! Consumed by `github::url`, whose GitHub-URL validator reuses
//! `is_public_host` so the SSRF defense the rest of the app relies on
//! stays in one canonical, test-pinned place. Used before any outbound fetch to
//! refuse loopback / RFC1918 / link-local / CGNAT / metadata-service
//! hosts.

use std::net::IpAddr;

/// Returns `true` when `host` (a hostname or IP literal) is a
/// public-routable destination. Rejects loopback, RFC1918, link-local
/// (incl. the cloud metadata 169.254.0.0/16 range), CGNAT, ULA IPv6,
/// and the `localhost` / `.local` / `.internal` magic names.
pub(crate) fn is_public_host(host: &str) -> bool {
    // IPv6 literals are wrapped in `[...]` in URLs but our parser strips
    // those before calling here — we still tolerate either form.
    let trimmed = host
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(host);

    if let Ok(ip) = trimmed.parse::<IpAddr>() {
        return is_public_ip(&ip);
    }

    // Hostnames: reject case-insensitively. `.local` is mDNS,
    // `.internal` is the de-facto private TLD, `localhost` is the magic
    // hostname that resolves to loopback.
    let lower = host.to_ascii_lowercase();
    if lower == "localhost" {
        return false;
    }
    if lower.ends_with(".local") || lower.ends_with(".internal") {
        return false;
    }
    // Reject the empty-label edge case — purely defensive.
    if lower.is_empty() || lower == "." {
        return false;
    }
    true
}

/// Classify a parsed IP literal as public-routable or not.
fn is_public_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            if v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_documentation()
            {
                return false;
            }
            // CGNAT 100.64.0.0/10 is not flagged by `is_private`.
            let octets = v4.octets();
            if octets[0] == 100 && (octets[1] & 0xC0) == 0x40 {
                return false;
            }
            // Benchmarking / 198.18.0.0/15 — RFC 2544.
            if octets[0] == 198 && (octets[1] == 18 || octets[1] == 19) {
                return false;
            }
            true
        }
        IpAddr::V6(v6) => {
            if v6.is_loopback() || v6.is_unspecified() || v6.is_multicast() {
                return false;
            }
            let segments = v6.segments();
            // ULA fc00::/7
            if segments[0] & 0xfe00 == 0xfc00 {
                return false;
            }
            // Link-local fe80::/10
            if segments[0] & 0xffc0 == 0xfe80 {
                return false;
            }
            // IPv4-mapped IPv6 (::ffff:0:0/96) — recurse on the
            // embedded IPv4 so private-IPv4 ranges are caught even
            // when expressed in IPv6 form.
            if segments[0] == 0
                && segments[1] == 0
                && segments[2] == 0
                && segments[3] == 0
                && segments[4] == 0
                && segments[5] == 0xffff
            {
                let v4 = std::net::Ipv4Addr::new(
                    (segments[6] >> 8) as u8,
                    (segments[6] & 0xff) as u8,
                    (segments[7] >> 8) as u8,
                    (segments[7] & 0xff) as u8,
                );
                return is_public_ip(&IpAddr::V4(v4));
            }
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_public_host_rejects_loopback_ipv4() {
        assert!(!is_public_host("127.0.0.1"));
        assert!(!is_public_host("127.255.255.255"));
    }

    #[test]
    fn is_public_host_rejects_link_local_ipv4_including_imds() {
        // 169.254.169.254 is AWS Instance Metadata Service — the canonical
        // SSRF target for cloud creds.
        assert!(!is_public_host("169.254.169.254"));
        assert!(!is_public_host("169.254.0.1"));
    }

    #[test]
    fn is_public_host_rejects_rfc1918_ipv4() {
        for addr in &[
            "10.0.0.1",
            "10.255.255.255",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.0.1",
            "192.168.255.255",
        ] {
            assert!(!is_public_host(addr), "expected {} to be rejected", addr);
        }
    }

    #[test]
    fn is_public_host_rejects_cgnat_ipv4() {
        // 100.64.0.0/10 — Carrier-Grade NAT, RFC 6598.
        assert!(!is_public_host("100.64.0.1"));
        assert!(!is_public_host("100.127.255.255"));
        // 100.0.0.0/16 is NOT CGNAT — must still be accepted.
        assert!(is_public_host("100.63.255.255"));
        assert!(is_public_host("100.128.0.1"));
    }

    #[test]
    fn is_public_host_rejects_unspecified_broadcast_multicast() {
        assert!(!is_public_host("0.0.0.0"));
        assert!(!is_public_host("255.255.255.255"));
        assert!(!is_public_host("224.0.0.1"));
        assert!(!is_public_host("239.255.255.255"));
    }

    #[test]
    fn is_public_host_rejects_loopback_ipv6() {
        assert!(!is_public_host("::1"));
        assert!(!is_public_host("[::1]"));
    }

    #[test]
    fn is_public_host_rejects_unique_local_ipv6() {
        // fc00::/7
        assert!(!is_public_host("fc00::1"));
        assert!(!is_public_host("fd12:3456:789a::1"));
    }

    #[test]
    fn is_public_host_rejects_link_local_ipv6() {
        // fe80::/10
        assert!(!is_public_host("fe80::1"));
        assert!(!is_public_host("[fe80::1]"));
    }

    #[test]
    fn is_public_host_rejects_ipv4_mapped_private_ipv6() {
        // ::ffff:10.0.0.1 — IPv4-mapped form of an RFC1918 address.
        assert!(!is_public_host("::ffff:10.0.0.1"));
        assert!(!is_public_host("::ffff:127.0.0.1"));
    }

    #[test]
    fn is_public_host_rejects_internal_tlds() {
        assert!(!is_public_host("localhost"));
        assert!(!is_public_host("LOCALHOST"));
        assert!(!is_public_host("printer.local"));
        assert!(!is_public_host("Printer.Local"));
        assert!(!is_public_host("metadata.google.internal"));
        assert!(!is_public_host("foo.INTERNAL"));
    }

    #[test]
    fn is_public_host_accepts_public_hosts() {
        assert!(is_public_host("example.com"));
        assert!(is_public_host("api.github.com"));
        assert!(is_public_host("8.8.8.8"));
        assert!(is_public_host("1.1.1.1"));
        assert!(is_public_host("github.com"));
    }
}
