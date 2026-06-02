use serde::{Deserialize, Serialize};

/// Data schema — tells the pipeline which domain encoders to apply
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Schema {
    /// Security intelligence: IPs, hashes, CVEs, IOCs, YARA rules
    SecurityIntel,
    /// Audit logs: timestamps, actor, action, hash chains
    AuditLog,
    /// Network captures: packets, headers, payloads
    NetworkCapture,
    /// Medical records: vitals, ICD codes, HL7/FHIR
    Medical,
    /// Military telemetry: coordinates, track IDs, classifications
    Military,
    /// Aviation: flight IDs, transponder, radar sweeps
    Aviation,
    /// Generic structured JSON — best-effort domain encoding
    GenericJson,
    /// Raw binary — entropy coding only
    Binary,
}

impl Schema {
    pub fn detect(sample: &[u8]) -> Self {
        // Heuristic detection from first 4KB sample
        let s = std::str::from_utf8(&sample[..sample.len().min(4096)])
            .unwrap_or("");

        if s.contains("\"cve\"") || s.contains("\"ioc\"") || s.contains("\"signature\"") {
            return Schema::SecurityIntel;
        }
        if s.contains("\"audit\"") || s.contains("\"prev_hash\"") || s.contains("\"actor\"") {
            return Schema::AuditLog;
        }
        if s.contains("\"src_ip\"") || s.contains("\"dst_ip\"") || s.contains("\"payload\"") {
            return Schema::NetworkCapture;
        }
        if s.contains("\"patient\"") || s.contains("\"vitals\"") || s.contains("\"icd\"") {
            return Schema::Medical;
        }
        if s.contains("\"track_id\"") || s.contains("\"classification\"") || s.contains("\"coord\"") {
            return Schema::Military;
        }
        if s.contains("\"squawk\"") || s.contains("\"flight_id\"") || s.contains("\"altitude\"") {
            return Schema::Aviation;
        }
        if s.starts_with('{') || s.starts_with('[') {
            return Schema::GenericJson;
        }
        Schema::Binary
    }

    pub fn name(&self) -> &'static str {
        match self {
            Schema::SecurityIntel  => "security-intel",
            Schema::AuditLog       => "audit-log",
            Schema::NetworkCapture => "network-capture",
            Schema::Medical        => "medical",
            Schema::Military       => "military",
            Schema::Aviation       => "aviation",
            Schema::GenericJson    => "generic-json",
            Schema::Binary         => "binary",
        }
    }
}
