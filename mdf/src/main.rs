use std::{
    io::{stdout, IsTerminal},
    path::PathBuf,
    time::Instant,
};

use clap::{Parser, Subcommand};
use fancy_duration::FancyDuration;
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

                let mut parser = gedcomfy::parser::Parser::read_file(&path, parse_options)
                    .into_diagnostic()
                    .with_context(|| format!("Parsing file {}", path.display()))?;

                let result = parser.parse()?;
                // TODO: print warnings
                println!("{:#?}", result.file);
            }
            GedcomCommands::Validate {
                path,
                force_encoding,
            } => {
                let parse_options =
                    ParseOptions::default().force_encoding(force_encoding.map(Into::into));

                let start_time = Instant::now();
                let mut parser = gedcomfy::parser::Parser::read_file(&path, parse_options)
                    .into_diagnostic()
                    .with_context(|| format!("Parsing file {}", path.display()))?;

                println!("File loaded: {}", path.display());
                println!("Validating file syntax…");

                let result = parser.validate()?;

                println!(
                    "Completed in {}",
                    FancyDuration(start_time.elapsed()).truncate(2)
                );

                println!("GEDCOM validation result: {}", result.validity);

                println!("{} messages produced", result.errors.len());
                for error in result.errors {
                    println!("{}", error);
                }
            }
        },
    }

    Ok(())
}
