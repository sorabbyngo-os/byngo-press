/// CPU SIMD capability detected at runtime
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SimdCapability {
    /// No SIMD — pure scalar (Nano profile, embedded)
    Scalar,
    /// SSE4.2 — 16 bytes/instruction (older x86)
    Sse42,
    /// AVX2 — 32 bytes/instruction (Forge profile, modern PC)
    Avx2,
    /// AVX-512 — 64 bytes/instruction (Apex profile, servers)
    Avx512,
    /// ARM NEON — 16 bytes/instruction (Swift profile, mobile)
    Neon,
}

impl SimdCapability {
    /// Detect the best available SIMD capability at runtime
    pub fn detect() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx512f") && is_x86_feature_detected!("avx512bw") {
                return Self::Avx512;
            }
            if is_x86_feature_detected!("avx2") {
                return Self::Avx2;
            }
            if is_x86_feature_detected!("sse4.2") {
                return Self::Sse42;
            }
        }
        #[cfg(target_arch = "aarch64")]
        {
            // NEON is mandatory on aarch64
            return Self::Neon;
        }
        Self::Scalar
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Scalar => "scalar",
            Self::Sse42  => "sse4.2",
            Self::Avx2   => "avx2",
            Self::Avx512 => "avx512",
            Self::Neon   => "neon",
        }
    }

    /// Compression level to use with zstd for this capability
    /// Higher capability = higher level (more CPU to spare)
    pub fn zstd_level(&self) -> i32 {
        match self {
            Self::Scalar => 3,
            Self::Sse42  => 9,
            Self::Avx2   => 15,
            Self::Avx512 => 22,
            Self::Neon   => 9,
        }
    }

    /// Number of parallel threads to use
    pub fn thread_count(&self) -> usize {
        match self {
            Self::Scalar => 1,
            Self::Sse42  => 2,
            Self::Avx2   => num_cpus::get().min(8),
            Self::Avx512 => num_cpus::get(),
            Self::Neon   => num_cpus::get().min(4),
        }
    }
}
