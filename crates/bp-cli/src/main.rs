use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber::{fmt, EnvFilter};

use bp_core::pipeline::Pipeline;
use bp_core::profile::Profile;
use bp_core::schema::Schema;
use bp_columnar::stage::ColumnarStage;
use bp_dedup::stage::SemanticDedupStage;
use bp_delta::stage::DeltaCompressionStage;
use bp_dictionary::stage::DictionaryStage;
use bp_domain::stage::DomainEncodingStage;
use bp_simd::stage::EntropyCodingStage;

#[derive(Parser)]
#[command(name="byngo-press", version="0.1.0", about="ByngoPress — 10,000:1 semantic compression engine")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compress a file
    Compress {
        input: PathBuf,
        #[arg(short, long)] output: Option<PathBuf>,
        #[arg(short, long, default_value="auto")] profile: ProfileArg,
        #[arg(short, long)] schema: Option<SchemaArg>,
    },
    /// Decompress a .bp file
    Decompress {
        input: PathBuf,
        #[arg(short, long)] output: Option<PathBuf>,
    },
    /// Show compression stats without writing output
    Stat {
        input: PathBuf,
        #[arg(short, long, default_value="auto")] profile: ProfileArg,
    },
    /// Show detected hardware capabilities
    Info,
}

#[derive(ValueEnum, Clone, Debug)]
enum ProfileArg { Auto, Apex, Forge, Swift, Nano }

#[derive(ValueEnum, Clone, Debug)]
enum SchemaArg { SecurityIntel, AuditLog, NetworkCapture, Medical, Military, Aviation, Json, Binary }

fn resolve_profile(arg: &ProfileArg) -> Profile {
    match arg {
        ProfileArg::Auto  => Profile::detect(),
        ProfileArg::Apex  => Profile::Apex,
        ProfileArg::Forge => Profile::Forge,
        ProfileArg::Swift => Profile::Swift,
        ProfileArg::Nano  => Profile::Nano,
    }
}

fn resolve_schema(arg: Option<&SchemaArg>, data: &[u8]) -> Schema {
    match arg {
        Some(SchemaArg::SecurityIntel)  => Schema::SecurityIntel,
        Some(SchemaArg::AuditLog)       => Schema::AuditLog,
        Some(SchemaArg::NetworkCapture) => Schema::NetworkCapture,
        Some(SchemaArg::Medical)        => Schema::Medical,
        Some(SchemaArg::Military)       => Schema::Military,
        Some(SchemaArg::Aviation)       => Schema::Aviation,
        Some(SchemaArg::Json)           => Schema::GenericJson,
        Some(SchemaArg::Binary)         => Schema::Binary,
        None                            => Schema::detect(data),
    }
}

fn build_pipeline(profile: Profile, schema: Schema) -> Pipeline {
    let mut pipeline = Pipeline::new(profile, schema);
    for stage in profile.stages() {
        match stage {
            bp_core::profile::Stage::DomainEncoding         => pipeline.add_stage(Box::new(DomainEncodingStage)),
            bp_core::profile::Stage::SemanticDedup          => pipeline.add_stage(Box::new(SemanticDedupStage)),
            bp_core::profile::Stage::DictionarySubstitution => pipeline.add_stage(Box::new(DictionaryStage)),
            bp_core::profile::Stage::DeltaCompression       => pipeline.add_stage(Box::new(DeltaCompressionStage)),
            bp_core::profile::Stage::ColumnarReorganization => pipeline.add_stage(Box::new(ColumnarStage)),
            bp_core::profile::Stage::EntropyCoding          => pipeline.add_stage(Box::new(EntropyCodingStage::new())),
            bp_core::profile::Stage::SchemaDetection        => {}
        }
    }
    pipeline
}

fn main() -> Result<()> {
    fmt()
        .with_env_filter(EnvFilter::from_default_env()
            .add_directive("bp_simd=info".parse()?)
            .add_directive("bp_dedup=info".parse()?)
            .add_directive("bp_delta=info".parse()?)
            .add_directive("bp_dictionary=info".parse()?))
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match &cli.command {
        Command::Info => {
            let simd    = bp_simd::capability::SimdCapability::detect();
            let profile = Profile::detect();
            println!("ByngoPress v0.1.0");
            println!("─────────────────────────────");
            println!("SIMD level:    {}", simd.name());
            println!("Auto profile:  {}", profile.name());
            println!("zstd level:    {}", simd.zstd_level());
            println!("Threads:       {}", simd.thread_count());
            println!("Block size:    {} MB", profile.block_size_bytes() / 1024 / 1024);
            println!("Max memory:    {} MB", profile.max_memory_bytes() / 1024 / 1024);
            println!("Prefetch:      {}", profile.supports_prefetch());
        }

        Command::Compress { input, output, profile, schema } => {
            let data    = std::fs::read(input)?;
            let profile = resolve_profile(profile);
            let schema  = resolve_schema(schema.as_ref(), &data);
            println!("ByngoPress compress");
            println!("  Input:   {} ({} bytes)", input.display(), data.len());
            println!("  Profile: {}", profile.name());
            println!("  Schema:  {}", schema.name());

            let pipeline        = build_pipeline(profile, schema);
            let (compressed, stats) = pipeline.compress(&data)?;

            let out_path = output.clone().unwrap_or_else(|| {
                let mut p = input.clone(); p.set_extension("bp"); p
            });
            std::fs::write(&out_path, &compressed)?;

            println!("\nResults:");
            println!("  Original:   {} bytes", stats.original_size);
            println!("  Compressed: {} bytes", stats.compressed_size);
            println!("  Ratio:      {:.1}:1", stats.ratio);
            println!("  Time:       {}ms", stats.duration_ms);
            println!("  Output:     {}", out_path.display());
            println!("\nStage breakdown:");
            for (stage, size) in &stats.stage_sizes {
                println!("  {:<32} {} bytes", stage, size);
            }
        }

        Command::Decompress { input, output } => {
            let data = std::fs::read(input)?;
            // Read profile/schema from the file envelope so we always use
            // the exact stage list that compressed this file — no guessing.
            let (profile, schema, pipeline) =
                if let Some(Ok(env)) = bp_core::pipeline::BpEnvelope::decode(&data) {
                    let p = env.profile;
                    let s = env.schema;
                    (p, s, build_pipeline(p, s))
                } else {
                    // Legacy file — fall back to hardware detection
                    let p = Profile::detect();
                    let s = Schema::GenericJson;
                    (p, s, build_pipeline(p, s))
                };
            let _ = (profile, schema); // used above
            let start = Instant::now();
            let out   = pipeline.decompress(&data)?;
            let out_path = output.clone().unwrap_or_else(|| {
                let mut p = input.clone(); p.set_extension("json"); p
            });
            std::fs::write(&out_path, &out)?;
            println!("Decompressed: {} → {} bytes in {}ms → {}", 
                data.len(), out.len(), start.elapsed().as_millis(), out_path.display());
        }

        Command::Stat { input, profile } => {
            let data    = std::fs::read(input)?;
            let profile = resolve_profile(profile);
            let schema  = Schema::detect(&data);
            let (_, stats) = build_pipeline(profile, schema).compress(&data)?;
            println!("ByngoPress stat — {}", input.display());
            println!("  Original:  {} bytes", stats.original_size);
            println!("  Projected: {} bytes", stats.compressed_size);
            println!("  Ratio:     {:.1}:1", stats.ratio);
            println!("  Profile:   {}", profile.name());
            println!("  Schema:    {}", schema.name());
        }
    }

    Ok(())
}
