use std::{
    io::{stdout, IsTerminal},
    path::PathBuf,
    time::Instant,
};

use clap::{Parser, Subcommand};
use gedcomfy::parser::{encodings::SupportedEncoding, options::ParseOptions};
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
    Validate {
        path: PathBuf,
        #[arg(long, rename_all = "kebab-case")]
        force_encoding: Option<Encoding>,
    },
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
#[allow(non_camel_case_types)] // want hyphens in these
pub enum Encoding {
    UTF_8,
    Windows_1252,
}

impl From<Encoding> for SupportedEncoding {
    fn from(value: Encoding) -> SupportedEncoding {
        match value {
            Encoding::UTF_8 => SupportedEncoding::UTF8,
            Encoding::Windows_1252 => SupportedEncoding::Windows1252,
        }
    }
}

fn main() -> miette::Result<()> {
    let args = MdfArgs::parse();

    let mut log = paris::Logger::new();
    if stdout().is_terminal() {
        // TODO
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

    match args {
        MdfArgs::Gedcom(args) => match args.command {
            GedcomCommands::Validate {
                path,
                force_encoding,
            } => {
                let parse_options = ParseOptions {
                    force_encoding: force_encoding.map(Into::into),
                };

                let start_time = Instant::now();
                let data = std::fs::read(&path)
                    .into_diagnostic()
                    .with_context(|| format!("Loading file {}", path.display()))?;

                let mut buffer = String::new();

                log.info(format!("File loaded: {}", path.display()));

                log.loading("Validating file syntax…");

                match gedcomfy::validate_syntax_opt(&data, &mut buffer, parse_options) {
                    Ok(count) => {
                        let elapsed = start_time.elapsed();
                        log.done()
                            .success("File syntax validation <bold>succeeded</>: {count} lines")
                            .indent(1)
                            .info(format!("Completed in {}s", elapsed.as_secs_f64()));

                        tracing::info!(
                            record_count = count,
                            path = %path.display(),
                            "file is (syntactically) valid",
                        );
                    }
                    Err(e) => {
                        let elapsed = start_time.elapsed();
                        log.done()
                            .warn("File syntax validation <bold>failed</>")
                            .indent(1)
                            .info(format!("Completed in {}s", elapsed.as_secs_f64()));

                        return Err(miette::Report::new(e).with_source_code(
                            NamedSource::new(path.to_string_lossy(), data).with_language("GEDCOM"),
                        ));
                    }
                }
            }
        },
    }

    Ok(())
}
