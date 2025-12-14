//! Rip mode argument parsing.
//!
//! Provides the ergonomic rip-style CLI with subcommands for:
//! - File deletion (moving to graveyard)
//! - Decompose (permanent deletion - root only)
//! - Seance (list deleted files)
//! - Unbury (restore deleted files)

use anstyle::{AnsiColor, Color::Ansi, Style};
use clap::builder::styling::Styles;
use clap::{Parser, Subcommand};

use std::io::{Error, ErrorKind};
use std::path::PathBuf;

const CMD_STYLE: Style = Style::new()
    .bold()
    .fg_color(Some(Ansi(AnsiColor::BrightCyan)));
const HEADER_STYLE: Style = Style::new().bold().fg_color(Some(Ansi(AnsiColor::Green)));
const PLACEHOLDER_STYLE: Style = Style::new().fg_color(Some(Ansi(AnsiColor::BrightCyan)));
const STYLES: Styles = Styles::styled()
    .literal(AnsiColor::BrightCyan.on_default().bold())
    .placeholder(AnsiColor::BrightCyan.on_default());

const OPTIONS_PLACEHOLDER: &str = "{options}";
const SUBCOMMANDS_PLACEHOLDER: &str = "{subcommands}";

fn help_template(template: &str) -> String {
    let header = HEADER_STYLE.render();
    let rheader = HEADER_STYLE.render_reset();
    let rip_s = CMD_STYLE.render();
    let rrip_s = CMD_STYLE.render_reset();
    let place = PLACEHOLDER_STYLE.render();
    let rplace = PLACEHOLDER_STYLE.render_reset();

    match template {
        "rip" => format!(
            "\
rip: a safe and ergonomic alternative to rm

{header}Usage{rheader}: {rip_s}rip{rrip_s} [{place}OPTIONS{rplace}] [{place}FILES{rplace}]...
       {rip_s}rip{rrip_s} [{place}SUBCOMMAND{rplace}]

{header}Arguments{rheader}:
    [{place}FILES{rplace}]...  Files or directories to remove

{header}Options{rheader}:
{OPTIONS_PLACEHOLDER}

{header}Subcommands{rheader}:
{SUBCOMMANDS_PLACEHOLDER}
"
        ),
        "completions" => format!(
            "\
Generate the shell completions file

{header}Usage{rheader}: {rip_s}rip completions{rrip_s} <{place}SHELL{rplace}>

{header}Arguments{rheader}:
    <{place}SHELL{rplace}>  The shell to generate completions for (bash, elvish, fish, powershell, zsh, nushell)

{header}Options{rheader}:
{OPTIONS_PLACEHOLDER}
"
        ),
        "graveyard" => format!(
            "\
Print the graveyard path

{header}Usage{rheader}: {rip_s}rip graveyard{rrip_s} [{place}OPTIONS{rplace}]

{header}Options{rheader}:
{OPTIONS_PLACEHOLDER}
"
        ),
        _ => unreachable!(),
    }
}

/// Rip mode CLI arguments.
///
/// Provides an ergonomic interface for safe file deletion with:
/// - Automatic graveyard management
/// - Undo capability via unbury
/// - Inspection before deletion
/// - Subcommands for completions and graveyard info
#[derive(Parser, Debug, Default)]
#[command(
    name = "rip",
    version,
    about,
    long_about = None,
    styles=STYLES,
    help_template = help_template("rip"),
)]
pub struct RipArgs {
    /// Files and directories to remove
    pub targets: Vec<PathBuf>,

    /// Directory where deleted files rest
    #[arg(long)]
    pub graveyard: Option<PathBuf>,

    /// Permanently deletes the graveyard
    #[arg(short, long)]
    pub decompose: bool,

    /// Prints files that were deleted
    /// in the current directory
    #[arg(short, long)]
    pub seance: bool,

    /// Restore the specified
    /// files or the last file
    /// if none are specified
    #[arg(short, long, num_args = 0..)]
    pub unbury: Option<Vec<PathBuf>>,

    /// Print some info about FILES before
    /// burying
    #[arg(short, long)]
    pub inspect: bool,

    /// Non-interactive mode
    #[arg(short, long)]
    pub force: bool,

    #[command(subcommand)]
    pub command: Option<RipCommands>,
}

/// Rip mode subcommands.
#[derive(Subcommand, Debug)]
pub enum RipCommands {
    /// Generate shell completions file
    #[command(styles=STYLES, help_template=help_template("completions"))]
    Completions {
        /// The shell to generate completions for
        #[arg(value_name = "SHELL")]
        shell: String,
    },

    /// Print the graveyard path
    #[command(styles=STYLES, help_template=help_template("graveyard"))]
    Graveyard {
        /// Get the graveyard subdirectory
        /// of the current directory
        #[arg(short, long)]
        seance: bool,
    },
}

/// Helper for checking if arguments are at their default values.
struct IsDefault {
    graveyard: bool,
    decompose: bool,
    seance: bool,
    unbury: bool,
    inspect: bool,
    force: bool,
    completions: bool,
}

impl IsDefault {
    fn new(cli: &RipArgs) -> Self {
        let defaults = RipArgs::default();
        Self {
            graveyard: cli.graveyard == defaults.graveyard,
            decompose: cli.decompose == defaults.decompose,
            seance: cli.seance == defaults.seance,
            unbury: cli.unbury == defaults.unbury,
            inspect: cli.inspect == defaults.inspect,
            force: cli.force == defaults.force,
            completions: cli.command.is_none(),
        }
    }
}

/// Validate rip mode arguments for compatibility.
///
/// Checks:
/// - completions subcommand can only be used by itself
/// - decompose can only be used with --graveyard
/// - force and inspect are mutually exclusive
#[allow(clippy::nonminimal_bool)]
pub fn validate_rip_args(cli: &RipArgs) -> Result<(), Error> {
    let defaults = IsDefault::new(cli);

    // [completions] can only be used by itself
    if !defaults.completions
        && !(defaults.graveyard
            && defaults.decompose
            && defaults.seance
            && defaults.unbury
            && defaults.inspect
            && defaults.force)
    {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "--completions can only be used by itself",
        ));
    }
    if !defaults.decompose && !(defaults.seance && defaults.unbury && defaults.inspect) {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "-d,--decompose can only be used with --graveyard",
        ));
    }

    // Force and inspect are incompatible
    if !defaults.force && !defaults.inspect {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "-f,--force and -i,--inspect cannot be used together",
        ));
    }

    Ok(())
}
