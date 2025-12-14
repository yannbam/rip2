//! CLI argument parsing for safe-rm.
//!
//! This module provides dual-mode argument parsing:
//! - `RipArgs`: Ergonomic rip-style interface with subcommands
//! - `RmArgs`: POSIX-compatible rm-style interface
//!
//! The execution mode determines which argument parser is used.

mod rip;
mod rm;

// Re-export rip mode types
pub use rip::{RipArgs, RipCommands, validate_rip_args};

// Re-export rm mode types
pub use rm::{RmArgs, validate_rm_args};

/// Execution mode: determines CLI behavior and argument parsing.
///
/// - `Rip`: Ergonomic interface with subcommands (decompose, seance, unbury, etc.)
/// - `Rm`: POSIX-compatible interface with rm-style flags (-r, -f, -d, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Rip mode: ergonomic interface with subcommands
    Rip,
    /// Rm mode: POSIX-compatible interface
    Rm,
}
