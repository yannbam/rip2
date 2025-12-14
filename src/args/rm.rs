//! Rm mode argument parsing.
//!
//! Provides POSIX-compatible rm-style CLI for safe-rm.
//! Files are moved to graveyard instead of permanent deletion,
//! but the interface matches GNU rm for drop-in compatibility.
//!
//! TODO(Phase 4): Implement full RmArgs with:
//! - `-f, --force`: Ignore nonexistent files, never prompt
//! - `-r, -R, --recursive`: Remove directories and their contents
//! - `-d, --dir`: Remove empty directories
//! - `-v, --verbose`: Explain what is being done
//! - `--preserve-root` / `--no-preserve-root`: Root protection
//! - `--one-file-system`: Stay on one filesystem

use std::path::PathBuf;

/// Rm mode CLI arguments.
///
/// POSIX-compatible interface that matches GNU rm behavior,
/// but moves files to graveyard instead of permanent deletion.
///
/// # Safety
///
/// Permanent deletion (bypassing graveyard) requires root privileges.
/// Non-root users always get recoverable deletion via graveyard.
#[derive(Debug, Default)]
pub struct RmArgs {
    /// Files and directories to remove
    pub targets: Vec<PathBuf>,

    // TODO(Phase 4): Add rm-compatible flags
    // pub force: bool,
    // pub recursive: bool,
    // pub dir: bool,
    // pub verbose: bool,
    // pub preserve_root: bool,
    // pub one_file_system: bool,
}
