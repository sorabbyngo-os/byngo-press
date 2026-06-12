use serde::{Deserialize, Serialize};

/// The four runtime profiles of ByngoPress
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Profile {
    /// Server-grade: 10,000:1, <1μs hot decompression, AVX-512, 7 stages
    Apex,
    /// Workstation: 1,000:1, <1ms hot decompression, AVX2, 5 stages
    Forge,
    /// Mobile: 100:1, <10ms decompression, ARM NEON, 3 stages
    Swift,
    /// Embedded/IoT/Consumer: 50:1, <100ms, scalar, 2 stages
    Nano,
}

impl Profile {
    pub fn name(&self) -> &'static str {
        match self {
            Profile::Apex  => "apex",
            Profile::Forge => "forge",
            Profile::Swift => "swift",
            Profile::Nano  => "nano",
        }
    }

    pub fn stages(&self) -> &'static [Stage] {
        match self {
            Profile::Apex  => &[
                Stage::SchemaDetection,
                Stage::SemanticDedup,
                Stage::DomainEncoding,
                Stage::DictionarySubstitution,
                Stage::DeltaCompression,
                Stage::ColumnarReorganization,
                Stage::EntropyCoding,
            ],
            Profile::Forge => &[
                Stage::SchemaDetection,
                Stage::SemanticDedup,
                Stage::DomainEncoding,
                Stage::DeltaCompression,
                Stage::DictionarySubstitution,
                Stage::EntropyCoding,
            ],
            Profile::Swift => &[
                Stage::SchemaDetection,
                Stage::SemanticDedup,
                Stage::DomainEncoding,
                Stage::EntropyCoding,
            ],
            Profile::Nano  => &[
                Stage::DomainEncoding,
                Stage::EntropyCoding,
            ],
        }
    }

    pub fn max_memory_bytes(&self) -> usize {
        match self {
            Profile::Apex  => 2 * 1024 * 1024 * 1024, // 2GB
            Profile::Forge => 512 * 1024 * 1024,       // 512MB
            Profile::Swift => 128 * 1024 * 1024,        // 128MB
            Profile::Nano  => 128 * 1024,               // 128KB
        }
    }

    pub fn block_size_bytes(&self) -> usize {
        match self {
            Profile::Apex  => 64 * 1024 * 1024,  // 64MB blocks
            Profile::Forge => 16 * 1024 * 1024,  // 16MB blocks
            Profile::Swift => 4 * 1024 * 1024,   // 4MB blocks
            Profile::Nano  => 64 * 1024,          // 64KB blocks
        }
    }

    pub fn supports_prefetch(&self) -> bool {
        matches!(self, Profile::Apex | Profile::Forge)
    }

    pub fn supports_simd(&self) -> bool {
        !matches!(self, Profile::Nano)
    }

    /// Auto-detect best profile for current hardware
    pub fn detect() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx512f") {
                return Profile::Apex;
            }
            if is_x86_feature_detected!("avx2") {
                return Profile::Forge;
            }
        }
        #[cfg(target_arch = "aarch64")]
        {
            // NEON is mandatory on aarch64
            return Profile::Swift;
        }
        Profile::Nano
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stage {
    SchemaDetection,
    SemanticDedup,
    DomainEncoding,
    DictionarySubstitution,
    DeltaCompression,
    ColumnarReorganization,
    EntropyCoding,
}

impl Stage {
    pub fn name(&self) -> &'static str {
        match self {
            Stage::SchemaDetection       => "schema-detection",
            Stage::SemanticDedup         => "semantic-dedup",
            Stage::DomainEncoding        => "domain-encoding",
            Stage::DictionarySubstitution => "dictionary-substitution",
            Stage::DeltaCompression      => "delta-compression",
            Stage::ColumnarReorganization => "columnar-reorganization",
            Stage::EntropyCoding         => "entropy-coding",
        }
    }
}
