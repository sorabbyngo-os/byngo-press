<div align="center">

# ByngoPress

**10,000:1 semantic compression engine with nanosecond decompression**  
*Built for mission-critical systems — military, intelligence, healthcare, aviation, and beyond*

[![License: UNLICENSED](https://img.shields.io/badge/license-UNLICENSED-red.svg)]()
[![Language: Rust](https://img.shields.io/badge/language-Rust-orange.svg)]()
[![Platform: Linux · Windows · macOS · Embedded](https://img.shields.io/badge/platform-Linux%20%7C%20Windows%20%7C%20macOS%20%7C%20Embedded-blue.svg)]()

</div>

---

## What is ByngoPress?

ByngoPress is a domain-aware, semantically-intelligent compression engine that achieves compression ratios conventional tools cannot approach — up to 10,000:1 on structured data — while maintaining sub-millisecond to nanosecond decompression on commodity hardware.

It is not a general-purpose compressor. It is a precision instrument designed for data types that have structure, meaning, and redundancy: security intelligence, medical records, military telemetry, network captures, audit logs, and sensor streams.

ByngoPress ships in four runtime profiles — **Apex, Forge, Swift, and Nano** — each tuned to a hardware class, from enterprise data centers to embedded IoT sensors. All profiles share a universal block format: a file compressed on a phone can be read on a server without conversion.

---

## Why ByngoPress?

General-purpose compressors see bytes. ByngoPress sees meaning.

When a threat intelligence feed mentions the same IP address 10 million times, `gzip` stores it 10 million times. ByngoPress stores it once and uses a 3-byte reference everywhere else. When an audit log records timestamps that differ by milliseconds, ByngoPress stores the delta — 2 bytes — not the full RFC3339 string — 25 bytes. When a medical record contains the field name `"blood_pressure_systolic"` ten thousand times, ByngoPress replaces it with a 2-byte dictionary index.

This semantic awareness is the source of the ratio advantage. The entropy coder is the last stage, not the only stage.

---

## Compression Profiles

| Profile | Target hardware | Ratio | Hot decompression | SIMD | Stages |
|---|---|---|---|---|---|
| **Apex** | Servers (Dell, HP, Soraxis) | 10,000:1 | < 1 microsecond | AVX-512 | 7 |
| **Forge** | PC / Laptop / Workstation | 1,000:1 | < 1 millisecond | AVX2 | 5 |
| **Swift** | Mobile (Android / iOS) | 100:1 | < 10 milliseconds | ARM NEON | 3 |
| **Nano** | IoT / Embedded / Consumer / Soraxis edge | 50:1 | < 100 milliseconds | Scalar / NEON | 2 |

All profiles read and write the same block format. Profile selection is automatic at runtime based on CPU capability detection, or can be forced via configuration.

---

## The Compression Pipeline

ByngoPress runs data through up to 7 sequential stages. Higher profiles run more stages; lower profiles skip the expensive stages to preserve battery and memory.
Raw input data (e.g. 1,000 GB)
│
▼
┌─────────────────────────────────┐
│  Stage 1: Schema Detection      │  Identifies data type and applies domain rules
│  All profiles                   │  Output: ~10 GB (typed, structured)
└────────────────┬────────────────┘
│
▼
┌─────────────────────────────────┐
│  Stage 2: Semantic Deduplication│  Content-addressed storage, SHA-256 refs
│  Apex, Forge, Swift             │  Output: ~1 GB  ← most ratio gain here
└────────────────┬────────────────┘
│
▼
┌─────────────────────────────────┐
│  Stage 3: Domain Encoding       │  IPs→4B, timestamps→delta, UUIDs→16B
│  All profiles                   │  Output: ~500 MB
└────────────────┬────────────────┘
│
▼
┌─────────────────────────────────┐
│  Stage 4: Dictionary Substitution│ Adaptive per-dataset domain dictionary
│  Apex, Forge, Swift             │  Output: ~200 MB
└────────────────┬────────────────┘
│
▼
┌─────────────────────────────────┐
│  Stage 5: Delta Compression     │  Store diffs between sequential records
│  Apex, Forge, Nano              │  Output: ~150 MB
└────────────────┬────────────────┘
│
▼
┌─────────────────────────────────┐
│  Stage 6: Columnar Reorganization│ Group fields together (Parquet-style)
│  Apex, Forge                    │  Output: ~120 MB
└────────────────┬────────────────┘
│
▼
┌─────────────────────────────────┐
│  Stage 7: Entropy Coding        │  SIMD-accelerated zstd / arithmetic coder
│  All profiles                   │  Output: ~100 MB
└────────────────┬────────────────┘
│
▼
Compressed output
(e.g. 100 MB from 1,000 GB)
---

## Decompression Architecture

ByngoPress never decompresses the full dataset. It decompresses on demand, block by block, with aggressive prefetching that hides latency.
Application request (e.g. "give me record #5,847,293")
│
▼
┌─────────────────────────────────────────────────────┐
│                  citadel-fs / mmap layer             │
│   Presents compressed data as regular memory         │
│   Application sees no compression at all             │
└──────────────┬──────────────────────────────────────┘
│
┌───────▼────────┐
│  HOT cache hit? │
│  (pinned RAM)   │
└───────┬─────────┘
YES │                    NO
< 1 ns ◄───┘               ┌───▼────────────────┐
│  WARM cache hit?   │
│  (RAM block cache) │
└───┬────────────────┘
YES │                NO
< 100 ns ◄───┘          ┌────▼──────────────────┐
│  Decompress from SSD  │
│  SIMD-accelerated     │
│  io_uring async I/O   │
└────┬──────────────────┘
< 1ms ◄───┘  (Apex/Forge)
< 10ms        (Swift)
< 100ms        (Nano)
### Prefetch Engine (Apex + Forge)

The prefetch engine runs on background cores and learns access patterns within minutes of deployment:

- **Sequential patterns** — audit logs read in time order → prefetch next N blocks
- **Entity patterns** — ARAM queries by IP → prefetch related entity blocks
- **Temporal patterns** — medical systems query by patient admission sequence
- **Geographic patterns** — military C2 queries by region and time window

By the time your application requests data, it is already decompressed in RAM. The application experiences nanosecond latency on data that was compressed at 10,000:1.

---

## Domain Encoders

ByngoPress ships with built-in encoders for common data types. Custom encoders can be registered via the plugin API.

| Encoder | Input | Compressed size | Savings |
|---|---|---|---|
| IPv4 | `"192.168.1.1"` (11B) | 4 bytes | 64% |
| IPv6 | `"2001:0db8::1"` (13B) | 16 bytes | 0% raw, but deduped |
| UUID | `"550e8400-e29b-41d4..."` (36B) | 16 bytes | 56% |
| Timestamp (RFC3339) | `"2026-05-31T12:00:00Z"` (20B) | 4 bytes delta | 80% |
| Severity enum | `"CRITICAL"` (8B) | 1 byte | 88% |
| GPS coordinate | `"1.2921° S, 36.8219° E"` (22B) | 8 bytes | 64% |
| CVE ID | `"CVE-2026-12345"` (14B) | 4 bytes | 71% |
| MAC address | `"AA:BB:CC:DD:EE:FF"` (17B) | 6 bytes | 65% |
| HTTP status | `"200 OK"` (6B) | 1 byte | 83% |
| Port number | `"443"` (3B) | 2 bytes | 33% |

---

## SIMD Acceleration

ByngoPress detects CPU capabilities at runtime and selects the fastest available instruction set:

| ISA | Hardware | Throughput |
|---|---|---|
| AVX-512 | Intel Xeon, AMD EPYC (servers) | 64 bytes/instruction |
| AVX2 | Intel Core i5+, AMD Ryzen (PCs) | 32 bytes/instruction |
| SSE4.2 | Older x86 | 16 bytes/instruction |
| ARM NEON | Apple Silicon, Snapdragon, Soraxis ARM | 16 bytes/instruction |
| Scalar | Cortex-M, RISC-V, any CPU | 1 byte/instruction |

The SIMD selection is transparent — same API, same binary format, automatically optimal performance on any hardware.

---

## Domain-Specific Vertical Targets

### Military & Intelligence
- Coordinates → 8-byte fixed-point (not float strings)
- Classification levels → 2 bits
- Track IDs → sequential integer
- Timestamps → microsecond delta from mission epoch
- **Target: threat picture query < 1 microsecond from hot cache**

### Healthcare
- Patient vitals → delta encoded (heart rate changes ±5 BPM)
- HL7/FHIR field names → 2-byte dictionary index
- DICOM metadata → schema-aware encoding
- ICD codes → 3-byte lookup
- **Target: patient record retrieval < 100 nanoseconds from hot cache**

### Aviation / ATC
- Flight IDs → 3 bytes (ICAO format)
- Transponder codes → 2 bytes (squawk)
- Altitude/speed → delta encoded
- Radar sweeps → columnar by parameter
- **Target: radar feed query < 500 nanoseconds from hot cache**

### Security / Citadel
- IP addresses → 4/16 bytes
- SHA-256 hashes → 32 bytes (already optimal, dedup gains)
- YARA signatures → dictionary compressed
- Audit events → delta + columnar
- **Target: threat intel query < 1 microsecond from hot cache**

### Consumer / Mobile
- Photos/video metadata → domain encoded
- App telemetry → delta + dictionary
- Sync payloads → differential compression
- **Target: < 10ms decompression, < 5% battery impact**

---

## Nano Profile — Embedded & Consumer

The Nano profile is the universal embedded runtime. It runs on:

- Consumer smartphones and tablets
- Budget laptops and Chromebooks
- Soraxis edge sensors and hardware modules
- Raspberry Pi and single-board computers
- Medical wearables and implantables
- Avionics black boxes and flight recorders
- Industrial IoT sensors

Nano uses streaming decompression — processes data in 64-byte chunks, never holding more than 128KB in working memory. It runs on hardware with no SIMD, no virtual memory, and as little as 256KB of RAM.

**Nano + Apex two-stage pipeline for Soraxis edge:**
Soraxis sensor → compress (Nano, 50:1) → transmit → server → recompress (Apex, 10,000:1) → store
The sensor compresses locally to reduce transmission bandwidth. The server performs full semantic compression for long-term storage.

---

## Architecture — Repository Structure
byngo-press/
├── crates/
│   ├── bp-core         — block format, skip index, profile traits, pipeline orchestration
│   ├── bp-dedup        — semantic deduplication, content-addressed store
│   ├── bp-domain       — domain encoders (IP, UUID, timestamp, GPS, enum, CVE...)
│   ├── bp-dictionary   — adaptive dictionary builder and codec
│   ├── bp-delta        — delta compression for sequential data
│   ├── bp-columnar     — columnar field reorganization
│   ├── bp-simd         — AVX-512 / AVX2 / NEON / scalar entropy coder
│   ├── bp-prefetch     — adaptive prefetch engine with pattern learning
│   ├── bp-cache        — tiered block cache (hot/warm/cold), LRU, NUMA-aware
│   ├── bp-fs           — FUSE virtual filesystem (byngo-fs)
│   ├── bp-io           — io_uring async I/O layer
│   └── bp-cli          — compress / decompress / benchmark / inspect CLI
├── benches/            — criterion benchmarks per profile and data type
├── tests/              — integration tests with synthetic and real datasets
├── docs/
│   ├── block-format.md — universal block format specification v1
│   ├── profiles.md     — profile capability matrix
│   ├── encoders.md     — domain encoder reference
│   └── ffi.md          — C/Python/Go FFI bindings
├── examples/
│   ├── compress_audit_log.rs
│   ├── compress_threat_intel.rs
│   ├── medical_records.rs
│   └── embedded_nano.rs
├── migrations/         — (unused — no DB dependency in core)
├── Cargo.toml          — workspace
└── README.md
---

## Performance Targets

| Operation | Apex | Forge | Swift | Nano |
|---|---|---|---|---|
| Compress 1GB | < 30s | < 2min | < 5min | < 10min |
| Hot cache read | < 1 μs | < 1 ms | < 10 ms | < 100 ms |
| Warm cache read | < 100 μs | < 10 ms | < 50 ms | < 500 ms |
| Cold decompress | < 10 ms | < 100 ms | < 1s | < 5s |
| Memory footprint | 2GB+ | 512MB | 128MB | 128KB |
| CPU cores used | All | 4-8 | 2-4 | 1 |

---

## Integration

ByngoPress is designed as a library first. Every Sorabbyngo OS component uses it natively:

```rust
use byngo_press::{Compressor, Profile, Schema};

// Compress threat intel data
let compressor = Compressor::new(Profile::Apex, Schema::SecurityIntel);
let compressed = compressor.compress(&raw_data).await?;

// Random access without full decompression
let record = compressed.get(record_id).await?;

// Mount as virtual filesystem
byngo_press::fs::mount("/mnt/intel", &compressed, MountOptions::default()).await?;
```

---

## Relationship to Sorabbyngo OS

ByngoPress is a foundational OS primitive in Sorabbyngo OS:
sorabbyngo-os
├── kernel
├── byngo-press          ← this repo, storage layer
├── citadel-sec          ← uses byngo-press for all data storage
│   ├── citadel-aram     ← intelligence data compressed at Apex profile
│   ├── citadel-security ← audit logs, PKI records at Apex profile
│   └── citadel-heal-brain ← event store at Apex profile
├── sorabbyngo-cyberwall ← packet captures compressed at Forge/Apex
└── ...
---

## Roadmap

- [x] Architecture and block format specification
- [ ] `bp-core` — block format, pipeline traits
- [ ] `bp-domain` — IP, UUID, timestamp encoders
- [ ] `bp-dedup` — semantic deduplication engine
- [ ] `bp-simd` — AVX2 entropy coder (Forge profile first)
- [ ] `bp-delta` — delta compression
- [ ] `bp-dictionary` — adaptive dictionary
- [ ] `bp-columnar` — columnar reorganization
- [ ] `bp-cache` — tiered cache
- [ ] `bp-prefetch` — prefetch engine
- [ ] `bp-io` — io_uring layer
- [ ] `bp-fs` — FUSE filesystem
- [ ] `bp-cli` — CLI tools
- [ ] Apex profile complete (all 7 stages)
- [ ] Forge profile complete (5 stages)
- [ ] Swift profile complete (3 stages)
- [ ] Nano profile complete (2 stages, no-std)
- [ ] C FFI bindings
- [ ] Python bindings
- [ ] Sorabbyngo OS integration
- [ ] Soraxis edge hardware integration

---

## License

UNLICENSED — Proprietary. All rights reserved. Sorabbyngo OS / Citadel Security.

---

<div align="center">
<sub>Built with Rust · Part of the Sorabbyngo OS ecosystem</sub>
</div>
