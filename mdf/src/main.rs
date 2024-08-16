use std::{
    io::{stdout, IsTerminal},
    path::PathBuf,
    time::Instant,
};

use fancy_duration::FancyDuration;
use gedcomfy::parser::{encodings::SupportedEncoding, options::ParseOptions, Parser};
use miette::{Context, IntoDiagnostic};

#[derive(clap::Parser)]
enum MdfArgs {
    Gedcom(GedcomArgs),
}

#[derive(clap::Args)]
struct GedcomArgs {
    #[command(subcommand)]
    command: GedcomCommands,
}

#[derive(clap::Subcommand)]
enum GedcomCommands {
    Parse {
        path: PathBuf,
        #[command(flatten)]
        parse_options: ParseOptionsArgs,
    },
    Validate {
        path: PathBuf,
        #[command(flatten)]
        parse_options: ParseOptionsArgs,
    },
    Kdl {
        path: PathBuf,
        #[command(flatten)]
        parse_options: ParseOptionsArgs,
    },
}

#[derive(clap::Args)]
struct ParseOptionsArgs {
    #[arg(long, rename_all = "kebab-case")]
    force_encoding: Option<Encoding>,
}

impl From<ParseOptionsArgs> for ParseOptions {
    fn from(args: ParseOptionsArgs) -> ParseOptions {
        ParseOptions::default().force_encoding(args.force_encoding.map(Into::into))
    }
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
    let args = <MdfArgs as clap::Parser>::parse();

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
            GedcomCommands::Kdl {
                path,
                parse_options,
            } => {
                let mut parser = Parser::read_file(&path, parse_options.into())
                    .into_diagnostic()
                    .with_context(|| format!("Parsing file {}", path.display()))?;

                let result = parser.parse_kdl()?;
                println!("{}", result);
            }
            GedcomCommands::Parse {
                path,
                parse_options,
            } => {
                let mut parser = Parser::read_file(&path, parse_options.into())
                    .into_diagnostic()
                    .with_context(|| format!("Parsing file {}", path.display()))?;

                let result = parser.parse()?;
                // TODO: print warnings
                println!("{:#?}", result.file);
            }
            GedcomCommands::Validate {
                path,
                parse_options,
            } => {
                let start_time = Instant::now();
                let mut parser = Parser::read_file(&path, parse_options.into())
                    .into_diagnostic()
                    .with_context(|| format!("Parsing file {}", path.display()))?;

                println!("File loaded: {}", path.display());
                println!("Validating file syntax…");

                let result = parser.validate()?;

                println!(
                    "Completed in {}",
                    FancyDuration(start_time.elapsed()).truncate(2)
                );

                println!("{:?}", miette::Report::new(result));
            }
        },
    }

    Ok(())
}
