use serde::{Deserialize, Serialize};

/// Type tag for domain-encoded values
/// Written as a single byte before each encoded value
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeTag {
    /// Raw unencoded bytes
    Raw          = 0x00,
    /// IPv4 address — 4 bytes
    Ipv4         = 0x01,
    /// IPv6 address — 16 bytes
    Ipv6         = 0x02,
    /// UUID — 16 bytes
    Uuid         = 0x03,
    /// Unix timestamp (seconds) — 4 bytes delta from epoch
    TimestampSec = 0x04,
    /// Unix timestamp (milliseconds) — 6 bytes delta
    TimestampMs  = 0x05,
    /// GPS coordinate pair (lat, lon) — 8 bytes fixed-point
    GpsCoord     = 0x06,
    /// Severity enum — 1 byte
    Severity     = 0x07,
    /// HTTP status code — 2 bytes
    HttpStatus   = 0x08,
    /// Port number — 2 bytes
    Port         = 0x09,
    /// MAC address — 6 bytes
    MacAddress   = 0x0A,
    /// CVE ID — 4 bytes (year 2B + number 2B)
    CveId        = 0x0B,
    /// ICAO flight ID — 4 bytes
    FlightId     = 0x0C,
    /// Squawk code — 2 bytes
    Squawk       = 0x0D,
    /// Classification level — 1 byte
    Classification = 0x0E,
    /// ICD-10 code — 3 bytes
    Icd10        = 0x0F,
    /// Boolean — 1 byte
    Bool         = 0x10,
    /// u8 integer
    U8           = 0x11,
    /// u16 integer
    U16          = 0x12,
    /// u32 integer
    U32          = 0x13,
    /// u64 integer
    U64          = 0x14,
    /// f32 float
    F32          = 0x15,
    /// f64 float
    F64          = 0x16,
    /// Short string (≤ 255 bytes) — 1B length prefix
    ShortString  = 0x17,
    /// Long string (≤ 65535 bytes) — 2B length prefix
    LongString   = 0x18,
}

impl TypeTag {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x00 => Some(Self::Raw),
            0x01 => Some(Self::Ipv4),
            0x02 => Some(Self::Ipv6),
            0x03 => Some(Self::Uuid),
            0x04 => Some(Self::TimestampSec),
            0x05 => Some(Self::TimestampMs),
            0x06 => Some(Self::GpsCoord),
            0x07 => Some(Self::Severity),
            0x08 => Some(Self::HttpStatus),
            0x09 => Some(Self::Port),
            0x0A => Some(Self::MacAddress),
            0x0B => Some(Self::CveId),
            0x0C => Some(Self::FlightId),
            0x0D => Some(Self::Squawk),
            0x0E => Some(Self::Classification),
            0x0F => Some(Self::Icd10),
            0x10 => Some(Self::Bool),
            0x11 => Some(Self::U8),
            0x12 => Some(Self::U16),
            0x13 => Some(Self::U32),
            0x14 => Some(Self::U64),
            0x15 => Some(Self::F32),
            0x16 => Some(Self::F64),
            0x17 => Some(Self::ShortString),
            0x18 => Some(Self::LongString),
            _    => None,
        }
    }
}

/// Severity levels — encoded as 1 byte
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info     = 0,
    Low      = 1,
    Medium   = 2,
    High     = 3,
    Critical = 4,
}

impl Severity {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "INFO"     => Some(Self::Info),
            "LOW"      => Some(Self::Low),
            "MEDIUM"   => Some(Self::Medium),
            "HIGH"     => Some(Self::High),
            "CRITICAL" => Some(Self::Critical),
            _          => None,
        }
    }

    pub fn to_str(self) -> &'static str {
        match self {
            Self::Info     => "INFO",
            Self::Low      => "LOW",
            Self::Medium   => "MEDIUM",
            Self::High     => "HIGH",
            Self::Critical => "CRITICAL",
        }
    }
}

/// Military classification levels — 1 byte
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classification {
    Unclassified       = 0,
    Restricted         = 1,
    Confidential       = 2,
    Secret             = 3,
    TopSecret          = 4,
    TopSecretSci       = 5,
}

impl Classification {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "UNCLASSIFIED"        => Some(Self::Unclassified),
            "RESTRICTED"          => Some(Self::Restricted),
            "CONFIDENTIAL"        => Some(Self::Confidential),
            "SECRET"              => Some(Self::Secret),
            "TOP SECRET"          => Some(Self::TopSecret),
            "TOP SECRET//SCI"     => Some(Self::TopSecretSci),
            _                     => None,
        }
    }
}
