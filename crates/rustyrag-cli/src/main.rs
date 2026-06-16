use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rustyrag_config::{list_adapters, load_pipeline_config, load_rag_config};
use rustyrag_etl::PipelineRunner;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "rustyrag", about = "Config-first RAG ETL framework")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate pipeline and RAG YAML configs.
    Validate {
        /// Path to a YAML file or directory containing configs.
        path: PathBuf,
    },
    /// Adapter registry commands.
    Adapters {
        #[command(subcommand)]
        command: AdaptersCommands,
    },
    /// Batch ingestion commands.
    Etl {
        #[command(subcommand)]
        command: EtlCommands,
    },
    /// Query API server.
    Serve {
        /// RAG config path.
        #[arg(long, default_value = "configs/rag/default.yaml")]
        config: PathBuf,
        /// Address to bind, e.g. 0.0.0.0:8080
        #[arg(long, default_value = "0.0.0.0:8080")]
        bind: String,
    },
}

#[derive(Subcommand)]
enum AdaptersCommands {
    /// List registered adapters (for GUI / config authoring).
    List {
        /// Filter by stage (source, chunk, embed, store, generation, search_mode).
        #[arg(long)]
        stage: Option<String>,
    },
}
#[derive(Subcommand)]
enum EtlCommands {
    /// Run the full ingest pipeline.
    Run { config: PathBuf },
    /// Parse and chunk without embedding or writing to Qdrant.
    DryRun { config: PathBuf },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load `.env` from the project root before reading configs or secrets.
    rustyrag_config::load_dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("rustyrag=info".parse()?))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Validate { path } => validate_command(path)?,
        Commands::Adapters { command } => match command {
            AdaptersCommands::List { stage } => adapters_command(stage.as_deref())?,
        },
        Commands::Etl { command } => match command {
            EtlCommands::Run { config } => etl_run(config).await?,
            EtlCommands::DryRun { config } => etl_dry_run(config).await?,
        },
        Commands::Serve { config, bind } => {
            rustyrag_api::serve(config, &bind).await?;
        }
    }

    Ok(())
}

fn adapters_command(stage: Option<&str>) -> Result<()> {
    let entries = list_adapters(stage);
    println!("{}", serde_json::to_string_pretty(&entries)?);
    Ok(())
}

fn validate_command(path: PathBuf) -> Result<()> {
    if path.is_dir() {
        for entry in std::fs::read_dir(&path).context("failed to read config directory")? {
            let entry = entry?;
            let file = entry.path();
            if is_yaml(&file) {
                validate_file(&file)?;
            }
        }
    } else {
        validate_file(&path)?;
    }

    println!("validation ok");
    Ok(())
}

fn validate_file(path: &PathBuf) -> Result<()> {
    let file_name = path.to_string_lossy();
    if file_name.contains("pipelines") || file_name.ends_with("pipeline.yaml") {
        let config = load_pipeline_config(path).context("pipeline config invalid")?;
        println!("pipeline `{}` is valid", config.name);
    } else {
        let config = load_rag_config(path).context("rag config invalid")?;
        println!("rag `{}` is valid", config.name);
    }
    Ok(())
}

async fn etl_run(config_path: PathBuf) -> Result<()> {
    let config = load_pipeline_config(&config_path).context("invalid pipeline config")?;
    let runner = PipelineRunner::new(config, config_path);
    let report = runner.run().await?;

    println!("ETL complete");
    println!("  documents seen:    {}", report.documents_seen);
    println!("  documents skipped: {}", report.documents_skipped);
    println!("  documents indexed: {}", report.documents_indexed);
    println!("  chunks written:    {}", report.chunks_written);
    Ok(())
}

async fn etl_dry_run(config_path: PathBuf) -> Result<()> {
    let config = load_pipeline_config(&config_path).context("invalid pipeline config")?;
    let runner = PipelineRunner::new(config, config_path);
    let report = runner.dry_run().await?;

    println!("Dry run complete");
    println!("  documents seen:      {}", report.documents_seen);
    println!("  documents skipped:   {}", report.documents_skipped);
    println!("  documents to index:  {}", report.documents_to_index);
    println!("  chunks (estimated):  {}", report.chunks_total);
    println!(
        "  embed cost (est.):   ${:.6} USD",
        report.estimated_embed_cost_usd
    );
    for warning in &report.warnings {
        println!("  warning: {warning}");
    }
    Ok(())
}

fn is_yaml(path: &std::path::Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("yaml") | Some("yml")
    )
}
