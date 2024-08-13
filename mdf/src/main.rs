use std::{
    io::{stdout, IsTerminal},
    path::PathBuf,
    time::Instant,
};

use clap::{Parser, Subcommand};
use fancy_duration::FancyDuration;
use gedcomfy::{
    parse_file,
    parser::{encodings::SupportedEncoding, options::ParseOptions},
};
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
    Parse {
        path: PathBuf,
        #[arg(long, rename_all = "kebab-case")]
        force_encoding: Option<Encoding>,
    },
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
            Encoding::UTF_8 => SupportedEncoding::Utf8,
            Encoding::Windows_1252 => SupportedEncoding::Windows1252,
        }
    }
}

fn main() -> miette::Result<()> {
    let args = MdfArgs::parse();

    if stdout().is_terminal() {
        // TODO
    } else {
        // non-interactive, format log messages using default `fmt`
    }
    tracing_subscriber::fmt::init();

    miette::set_hook(Box::new(|_| {
        Box::new(
            miette::MietteHandlerOpts::default()
                .with_syntax_highlighting(gedcomfy::highlighting::GEDCOMHighlighter {})
                .build(),
        )
    }))?;

    match args {
        MdfArgs::Gedcom(args) => match args.command {
            GedcomCommands::Parse {
                path,
                force_encoding,
            } => {
                let parse_options =
                    ParseOptions::default().force_encoding(force_encoding.map(Into::into));

                let result = parse_file(&path, parse_options)?;
                println!("{:#?}", result);
            }
            GedcomCommands::Validate {
                path,
                force_encoding,
            } => {
                let parse_options =
                    ParseOptions::default().force_encoding(force_encoding.map(Into::into));

                let start_time = Instant::now();
                let data = std::fs::read(&path)
                    .into_diagnostic()
                    .with_context(|| format!("Loading file {}", path.display()))?;

                let mut buffer = String::new();

                println!("File loaded: {}", path.display());
                println!("Validating file syntax…");

                let validation_result =
                    gedcomfy::validate_syntax_opt(&data, &mut buffer, parse_options);
                println!(
                    "Completed in {}",
                    FancyDuration(start_time.elapsed()).truncate(2)
                );

                match validation_result {
                    Ok(count) => {
                        println!("File syntax validation succeeded: {count} lines");
                        tracing::info!(
                            record_count = count,
                            path = %path.display(),
                            "file is (syntactically) valid",
                        );
                    }
                    Err(e) => {
                        println!("File syntax validation failed");
                        return Err(miette::Report::new(e)
                            .context(format!("Validating {}", path.display()))
                            .with_source_code(
                                NamedSource::new(path.to_string_lossy(), data)
                                    .with_language("GEDCOM"),
                            ));
                    }
                }
            }
        },
    }

    Ok(())
}
