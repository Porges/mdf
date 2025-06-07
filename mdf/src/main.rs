use std::{
    io::{stdout, IsTerminal},
    path::PathBuf,
    time::Instant,
};

use fancy_duration::FancyDuration;
use gedcomfy::{
    reader::{encodings::Encoding, options::ParseOptions, Reader},
    versions::KnownVersion,
};

mod components;

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
#[clap(rename_all = "kebab-case")]
struct ParseOptionsArgs {
    #[arg(long)]
    force_encoding: Option<ForcedEncoding>,

    #[arg(long)]
    force_version: Option<ForcedVersion>,
}

impl From<ParseOptionsArgs> for ParseOptions {
    fn from(args: ParseOptionsArgs) -> ParseOptions {
        ParseOptions::default()
            .force_encoding(args.force_encoding.map(Into::into))
            .force_version(args.force_version.map(Into::into))
    }
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
#[allow(non_camel_case_types)] // want hyphens in these
pub enum ForcedEncoding {
    UTF_8,
    Windows_1252,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum ForcedVersion {
    #[clap(name = "5.5")]
    V55,
    #[clap(name = "5.5.1")]
    V551,
    #[clap(name = "7.0", alias = "7")]
    V7,
}

impl From<ForcedEncoding> for Encoding {
    fn from(value: ForcedEncoding) -> Encoding {
        match value {
            ForcedEncoding::UTF_8 => Encoding::Utf8,
            ForcedEncoding::Windows_1252 => Encoding::Windows1252,
        }
    }
}

impl From<ForcedVersion> for KnownVersion {
    fn from(value: ForcedVersion) -> KnownVersion {
        match value {
            ForcedVersion::V55 => KnownVersion::V5_5,
            ForcedVersion::V551 => KnownVersion::V5_5_1,
            ForcedVersion::V7 => KnownVersion::V7_0,
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
                let reader = Reader::with_options(parse_options.into());
                let input = reader.decode_file(path)?;
                let result = reader.parse_kdl(&input)?;
                println!("{result}");
            }
            GedcomCommands::Parse {
                path,
                parse_options,
            } => {
                let reader = Reader::with_options(parse_options.into());
                let input = reader.decode_file(path)?;
                let result = reader.parse(&input)?;
                // TODO: print warnings
                println!("{:#?}", result.file);
            }
            GedcomCommands::Validate {
                path,
                parse_options,
            } => {
                let start_time = Instant::now();
                let reader = Reader::with_options(parse_options.into());
                let input = reader.decode_file(&path)?;

                println!("File loaded: {}", path.display());
                println!("Validating file syntax…");

                let result = reader.validate(&input)?;

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
