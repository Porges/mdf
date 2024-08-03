use std::path::PathBuf;

use clap::{Parser, Subcommand};
use miette::{Context, IntoDiagnostic, NamedSource};

#[derive(Parser)]
enum MdfArgs {
    Gedcom(GedcomArgs),
}

#[derive(Debug, clap::Args)]
struct GedcomArgs {
    #[command(subcommand)]
    command: GedcomCommands,
}

#[derive(Debug, Subcommand)]
enum GedcomCommands {
    Validate { path: PathBuf },
}

fn main() -> miette::Result<()> {
    tracing_subscriber::fmt::init();

    miette::set_hook(Box::new(|_| {
        Box::new(
            miette::MietteHandlerOpts::default()
                .with_syntax_highlighting(gedcom::highlighting::GEDCOMHighlighter {})
                .build(),
        )
    }))?;

    let args = MdfArgs::parse();
    match args {
        MdfArgs::Gedcom(args) => match args.command {
            GedcomCommands::Validate { path } => {
                let data = std::fs::read(&path)
                    .into_diagnostic()
                    .with_context(|| format!("Loading file {}", path.display()))?;

                let count = gedcom::validate(&data)
                    .with_context(|| format!("validating {}", path.display()))
                    .map_err(|e| {
                        e.with_source_code(
                            NamedSource::new(path.to_string_lossy(), data).with_language("GEDCOM"),
                        )
                    })?;

                tracing::info!(
                    record_count = count,
                    path = %path.display(),
                    "file is valid",
                );

                Ok(())
            }
        },
    }
}
