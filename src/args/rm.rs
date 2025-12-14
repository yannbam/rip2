//! Rm mode argument parsing.
//!
//! Provides POSIX-compatible rm-style CLI for safe-rm.
//! Files are moved to graveyard instead of permanent deletion,
//! but the interface matches GNU rm for drop-in compatibility.

use anstyle::{AnsiColor, Color::Ansi, Style};
use clap::builder::styling::Styles;
use clap::Parser;
use std::path::PathBuf;

// Styling constants (matching rip mode for consistency)
const CMD_STYLE: Style = Style::new()
    .bold()
    .fg_color(Some(Ansi(AnsiColor::BrightCyan)));
const HEADER_STYLE: Style = Style::new().bold().fg_color(Some(Ansi(AnsiColor::Green)));
const PLACEHOLDER_STYLE: Style = Style::new().fg_color(Some(Ansi(AnsiColor::BrightCyan)));
const STYLES: Styles = Styles::styled()
    .literal(AnsiColor::BrightCyan.on_default().bold())
    .placeholder(AnsiColor::BrightCyan.on_default());

const OPTIONS_PLACEHOLDER: &str = "{options}";

fn help_template() -> String {
    let header = HEADER_STYLE.render();
    let rheader = HEADER_STYLE.render_reset();
    let cmd = CMD_STYLE.render();
    let rcmd = CMD_STYLE.render_reset();
    let place = PLACEHOLDER_STYLE.render();
    let rplace = PLACEHOLDER_STYLE.render_reset();

    format!(
        "\
safe-rm: a safer rm that moves files to graveyard

{header}Usage{rheader}: {cmd}rm{rcmd} [{place}OPTIONS{rplace}] [{place}FILES{rplace}]...

{header}Arguments{rheader}:
    [{place}FILES{rplace}]...  Files or directories to remove

{header}Options{rheader}:
{OPTIONS_PLACEHOLDER}

{header}Note{rheader}:
    Files are moved to ~/.graveyard instead of permanent deletion.
    Use 'rip -u' to restore deleted files.
"
    )
}

/// Rm mode CLI arguments.
///
/// POSIX-compatible interface that matches GNU rm behavior,
/// but moves files to graveyard instead of permanent deletion.
///
/// # Safety
///
/// Permanent deletion (bypassing graveyard) requires root privileges.
/// Non-root users always get recoverable deletion via graveyard.
///
/// # Differences from GNU rm
///
/// - Files are moved to ~/.graveyard, not permanently deleted
/// - `--no-preserve-root` requires root privileges
/// - Home directory depth protection blocks `rm -rf ~/Desktop`
#[derive(Parser, Debug, Default)]
#[command(
    name = "rm",
    version,
    about = "safe-rm: a safer rm that moves files to graveyard",
    long_about = None,
    styles = STYLES,
    help_template = help_template(),
)]
pub struct RmArgs {
    /// Files and directories to remove
    pub targets: Vec<PathBuf>,

    /// Ignore nonexistent files and arguments, never prompt
    #[arg(short = 'f', long)]
    pub force: bool,

    /// Remove directories and their contents recursively
    #[arg(short = 'r', short_alias = 'R', long)]
    pub recursive: bool,

    /// Remove empty directories
    #[arg(short = 'd', long = "dir")]
    pub dir: bool,

    /// Explain what is being done
    #[arg(short = 'v', long)]
    pub verbose: bool,

    /// Do not treat '/' specially (default: preserve root)
    ///
    /// WARNING: Requires root privileges in safe-rm.
    /// Conflicts with --preserve-root.
    #[arg(long, conflicts_with = "preserve_root")]
    pub no_preserve_root: bool,

    /// Do not remove '/' (default behavior)
    ///
    /// This flag is provided for compatibility but is the default.
    /// Use --no-preserve-root to override (requires root).
    #[arg(long)]
    pub preserve_root: bool,

    /// When removing a hierarchy recursively, skip any directory
    /// that is on a different file system
    #[arg(long)]
    pub one_file_system: bool,
}

impl RmArgs {
    /// Check if root directory should be preserved.
    ///
    /// Root is preserved by default. Only --no-preserve-root disables this.
    /// Note: --no-preserve-root requires root privileges in safe-rm.
    pub fn should_preserve_root(&self) -> bool {
        !self.no_preserve_root
    }
}

/// Validate rm mode arguments for compatibility.
///
/// Currently performs no validation since clap handles conflicts_with.
/// Future: Add home directory protection validation here.
pub fn validate_rm_args(_cli: &RmArgs) -> Result<(), std::io::Error> {
    // clap handles --preserve-root / --no-preserve-root conflict
    // Additional validation can be added here as needed
    Ok(())
}
