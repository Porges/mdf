mod cliclack_layer;

use std::{
    io::{stdout, IsTerminal},
    path::PathBuf,
    time::Instant,
};

use clap::{Parser, Subcommand};
use miette::{Context, IntoDiagnostic, NamedSource};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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
    if stdout().is_terminal() {
        // HACK: override console wants_emoji detection
        // https://github.com/console-rs/console/blob/de2f15a31a8fef0b0e65ef4bdf92cd03c3061dac/src/windows_term/mod.rs#L505
        std::env::set_var("WT_SESSION", "1");

        // interactive, format log messages using nice `cliclack`
        tracing_subscriber::Registry::default()
            .with(cliclack_layer::CliclackLayer::new())
            .init();

        cliclack::intro("Welcome to MDF!").into_diagnostic()?;
    } else {
        // non-interactive, format log messages using default `fmt`
        tracing_subscriber::fmt::init();
    }

    miette::set_hook(Box::new(|_| {
        Box::new(
            miette::MietteHandlerOpts::default()
                .with_syntax_highlighting(gedcomfy::highlighting::GEDCOMHighlighter {})
                .build(),
        )
    }))?;

    let args = MdfArgs::parse();
    match args {
        MdfArgs::Gedcom(args) => match args.command {
            GedcomCommands::Validate { path } => {
                let start_time = Instant::now();

                let data = std::fs::read(&path)
                    .into_diagnostic()
                    .with_context(|| format!("Loading file {}", path.display()))?;

                let mut buffer = String::new();

                let count = gedcomfy::validate_syntax(&data, &mut buffer)
                    .with_context(|| format!("validating {}", path.display()))
                    .map_err(|e| {
                        e.with_source_code(
                            NamedSource::new(path.to_string_lossy(), data).with_language("GEDCOM"),
                        )
                    })?;

                let elapsed = start_time.elapsed();

                tracing::info!(
                    record_count = count,
                    elapsed = ?elapsed,
                    path = %path.display(),
                    "file is (syntactically) valid",
                );
            }
        },
    }

    if stdout().is_terminal() {
        cliclack::outro("Goodbye!").into_diagnostic()?;
    }

    Ok(())
}
