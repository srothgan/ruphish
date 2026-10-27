use clap::{CommandFactory, Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum Mode {
    All,
    Admin,
    Phish,
}

impl Mode {
    pub(crate) fn admin(self) -> bool {
        matches!(self, Self::All | Self::Admin)
    }
    pub(crate) fn phish(self) -> bool {
        matches!(self, Self::All | Self::Phish)
    }
}

#[derive(Parser)]
#[command(name = "ruphish", version = env!("CARGO_PKG_VERSION"), disable_help_flag = true, disable_version_flag = true, color = clap::ColorChoice::Never)]
pub(crate) struct Arguments {
    /// Read an existing config file; otherwise use/create the OS user configuration.
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,
    /// Run the selected administrative/public listeners.
    #[arg(long, value_enum, default_value = "all")]
    pub(crate) mode: Mode,
    /// Disable the mailer; IMAP remains an independent service.
    #[arg(long)]
    pub(crate) disable_mailer: bool,
    #[arg(
        long = "no-disable-mailer",
        hide = true,
        conflicts_with = "disable_mailer"
    )]
    enable_mailer: bool,
    /// Show command help.
    #[arg(long, short = 'h')]
    help: bool,
    /// Show extended command help.
    #[arg(long, hide = true)]
    help_long: bool,
    /// Show a manual page.
    #[arg(long, hide = true)]
    help_man: bool,
    /// Show application version.
    #[arg(long = "version", action = clap::ArgAction::Version)]
    _version: Option<bool>,
}

pub(crate) fn parse() -> Result<Option<Arguments>, clap::Error> {
    let mut arguments = match Arguments::try_parse() {
        Err(error) if error.kind() == clap::error::ErrorKind::DisplayVersion => {
            eprintln!("Ruphish {}", env!("CARGO_PKG_VERSION"));
            return Ok(None);
        }
        result => result?,
    };
    if arguments.enable_mailer {
        arguments.disable_mailer = false;
    }
    // Preserve legacy stream selection without copying Kingpin's presentation.
    if arguments.help {
        eprintln!("{}", Arguments::command().render_help());
    } else if arguments.help_long {
        println!("{}", Arguments::command().render_long_help());
    } else if arguments.help_man {
        println!(
            ".TH RUPHISH 1 {}\n.SH NAME\nruphish\n.SH OPTIONS\n.nf\n{}\n.fi",
            env!("CARGO_PKG_VERSION"),
            Arguments::command().render_long_help()
        );
    } else {
        return Ok(Some(arguments));
    }
    Ok(None)
}
