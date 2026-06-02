use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use crate::error::DomainError;
use crate::types::{Classification, Severity};

// ── IPv4 ──────────────────────────────────────────────────────────────────────

pub fn encode_ipv4(ip: &str) -> Result<[u8; 4], DomainError> {
    Ipv4Addr::from_str(ip)
        .map(|a| a.octets())
        .map_err(|e| DomainError::EncodeError {
            kind:   "IPv4".into(),
            reason: e.to_string(),
        })
}

pub fn decode_ipv4(bytes: &[u8; 4]) -> String {
    Ipv4Addr::from(*bytes).to_string()
}

// ── IPv6 ──────────────────────────────────────────────────────────────────────

pub fn encode_ipv6(ip: &str) -> Result<[u8; 16], DomainError> {
    Ipv6Addr::from_str(ip)
        .map(|a| a.octets())
        .map_err(|e| DomainError::EncodeError {
            kind:   "IPv6".into(),
            reason: e.to_string(),
        })
}

pub fn decode_ipv6(bytes: &[u8; 16]) -> String {
    Ipv6Addr::from(*bytes).to_string()
}

// ── UUID ──────────────────────────────────────────────────────────────────────

pub fn encode_uuid(s: &str) -> Result<[u8; 16], DomainError> {
    let s = s.replace('-', "");
    if s.len() != 32 {
        return Err(DomainError::InvalidLength { expected: 32, actual: s.len() });
    }
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = u8::from_str_radix(&s[i*2..i*2+2], 16)
            .map_err(|e| DomainError::EncodeError {
                kind:   "UUID".into(),
                reason: e.to_string(),
            })?;
    }
    Ok(out)
}

pub fn decode_uuid(bytes: &[u8; 16]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],  bytes[1],  bytes[2],  bytes[3],
        bytes[4],  bytes[5],
        bytes[6],  bytes[7],
        bytes[8],  bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

// ── Timestamp (Unix seconds, delta from base epoch) ───────────────────────────

const BASE_EPOCH: i64 = 1_700_000_000; // Nov 2023 — reduces delta size

pub fn encode_timestamp_sec(ts: i64) -> [u8; 4] {
    let delta = (ts - BASE_EPOCH) as i32;
    delta.to_le_bytes()
}

pub fn decode_timestamp_sec(bytes: &[u8; 4]) -> i64 {
    let delta = i32::from_le_bytes(*bytes) as i64;
    BASE_EPOCH + delta
}

pub fn encode_timestamp_ms(ts_ms: i64) -> [u8; 6] {
    // Store as 48-bit little-endian
    let mut out = [0u8; 6];
    let b = ts_ms.to_le_bytes();
    out.copy_from_slice(&b[..6]);
    out
}

pub fn decode_timestamp_ms(bytes: &[u8; 6]) -> i64 {
    let mut buf = [0u8; 8];
    buf[..6].copy_from_slice(bytes);
    i64::from_le_bytes(buf)
}

// ── GPS Coordinates ───────────────────────────────────────────────────────────
// lat: -90.0 to 90.0, lon: -180.0 to 180.0
// Encode as fixed-point i32 with 6 decimal places (±0.000001° ≈ 0.11m precision)

pub fn encode_gps(lat: f64, lon: f64) -> [u8; 8] {
    let lat_fixed = (lat * 1_000_000.0) as i32;
    let lon_fixed = (lon * 1_000_000.0) as i32;
    let mut out = [0u8; 8];
    out[..4].copy_from_slice(&lat_fixed.to_le_bytes());
    out[4..].copy_from_slice(&lon_fixed.to_le_bytes());
    out
}

pub fn decode_gps(bytes: &[u8; 8]) -> (f64, f64) {
    let lat_fixed = i32::from_le_bytes(bytes[..4].try_into().unwrap());
    let lon_fixed = i32::from_le_bytes(bytes[4..].try_into().unwrap());
    (lat_fixed as f64 / 1_000_000.0, lon_fixed as f64 / 1_000_000.0)
}

// ── Severity ──────────────────────────────────────────────────────────────────

pub fn encode_severity(s: &str) -> Option<u8> {
    Severity::from_str(s).map(|v| v as u8)
}

pub fn decode_severity(b: u8) -> &'static str {
    match b {
        0 => "INFO",
        1 => "LOW",
        2 => "MEDIUM",
        3 => "HIGH",
        4 => "CRITICAL",
        _ => "UNKNOWN",
    }
}

// ── MAC Address ───────────────────────────────────────────────────────────────

pub fn encode_mac(mac: &str) -> Result<[u8; 6], DomainError> {
    let parts: Vec<&str> = mac.split([':', '-']).collect();
    if parts.len() != 6 {
        return Err(DomainError::InvalidLength { expected: 6, actual: parts.len() });
    }
    let mut out = [0u8; 6];
    for (i, p) in parts.iter().enumerate() {
        out[i] = u8::from_str_radix(p, 16)
            .map_err(|e| DomainError::EncodeError {
                kind:   "MAC".into(),
                reason: e.to_string(),
            })?;
    }
    Ok(out)
}

pub fn decode_mac(bytes: &[u8; 6]) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5]
    )
}

// ── CVE ID ────────────────────────────────────────────────────────────────────
// Format: CVE-YYYY-NNNNN → year (2B) + number (2B)

pub fn encode_cve(cve: &str) -> Result<[u8; 4], DomainError> {
    let parts: Vec<&str> = cve.split('-').collect();
    if parts.len() < 3 || parts[0] != "CVE" {
        return Err(DomainError::EncodeError {
            kind:   "CVE".into(),
            reason: format!("invalid format: {}", cve),
        });
    }
    let year: u16 = parts[1].parse()
        .map_err(|e: std::num::ParseIntError| DomainError::EncodeError {
            kind: "CVE".into(), reason: e.to_string()
        })?;
    let num: u16 = parts[2].parse::<u32>()
        .map(|n| n.min(65535) as u16)
        .map_err(|e| DomainError::EncodeError {
            kind: "CVE".into(), reason: e.to_string()
        })?;
    let mut out = [0u8; 4];
    out[..2].copy_from_slice(&year.to_le_bytes());
    out[2..].copy_from_slice(&num.to_le_bytes());
    Ok(out)
}

pub fn decode_cve(bytes: &[u8; 4]) -> String {
    let year = u16::from_le_bytes(bytes[..2].try_into().unwrap());
    let num  = u16::from_le_bytes(bytes[2..].try_into().unwrap());
    format!("CVE-{}-{}", year, num)
}

// ── Classification ────────────────────────────────────────────────────────────

pub fn encode_classification(s: &str) -> Option<u8> {
    Classification::from_str(s).map(|v| v as u8)
}

pub fn decode_classification(b: u8) -> &'static str {
    match b {
        0 => "UNCLASSIFIED",
        1 => "RESTRICTED",
        2 => "CONFIDENTIAL",
        3 => "SECRET",
        4 => "TOP SECRET",
        5 => "TOP SECRET//SCI",
        _ => "UNKNOWN",
    }
}

// ── Port number ───────────────────────────────────────────────────────────────

pub fn encode_port(port: u16) -> [u8; 2] {
    port.to_le_bytes()
}

pub fn decode_port(bytes: &[u8; 2]) -> u16 {
    u16::from_le_bytes(*bytes)
}

// ── HTTP status ───────────────────────────────────────────────────────────────

pub fn encode_http_status(status: u16) -> [u8; 2] {
    status.to_le_bytes()
}

pub fn decode_http_status(bytes: &[u8; 2]) -> u16 {
    u16::from_le_bytes(*bytes)
}
