# RM Compatibility Mode - Implementation Guide

**Target:** rip2 v0.9.4+
**Reference Spec:** RM_COMPAT_SPEC.md
**Estimated Effort:** 5-8 implementation sessions

---

## Quick Start

This guide provides step-by-step instructions for implementing rm-compatible mode in rip2.

**Prerequisites:**
- Read RM_COMPAT_SPEC.md thoroughly
- Understand current rip2 architecture
- Have GNU coreutils source at `/home/jan/src/coreutils/`

**Testing approach:**
- Test-driven development (TDD)
- Compare behavior against real GNU rm
- Maintain backward compatibility tests

---

## Phase 1: Foundation & Mode Detection

### Step 1.1: Create Mode Detection Module

**File:** `src/mode.rs` (NEW)

```rust
//! Operating mode detection for rip vs rm compatibility

use std::env;
use std::path::Path;

/// Operating mode for the binary
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatingMode {
    /// Normal rip mode with graveyard features
    Rip,
    /// GNU rm-compatible mode
    RmCompat,
}

impl OperatingMode {
    /// Detect operating mode from argv[0] and environment
    ///
    /// Priority:
    /// 1. Environment variable RIP_MODE (for testing)
    /// 2. Binary name (argv[0])
    /// 3. Default to Rip
    pub fn detect() -> Self {
        // Check environment variable first (for testing)
        if let Ok(mode) = env::var("RIP_MODE") {
            return match mode.to_lowercase().as_str() {
                "rm" => OperatingMode::RmCompat,
                "rip" => OperatingMode::Rip,
                _ => {
                    eprintln!("Warning: invalid RIP_MODE='{}', using default", mode);
                    OperatingMode::Rip
                }
            };
        }

        // Check argv[0] binary name
        if let Some(arg0) = env::args().next() {
            let path = Path::new(&arg0);
            if let Some(filename) = path.file_name() {
                let name = filename.to_string_lossy();
                // Check if invoked as 'rm' (handles /usr/bin/rm, ./rm, rm.exe, etc.)
                if name == "rm" || name == "rm.exe" {
                    return OperatingMode::RmCompat;
                }
            }
        }

        // Default to rip mode
        OperatingMode::Rip
    }

    /// Returns true if in rm-compatible mode
    pub fn is_rm_compat(self) -> bool {
        matches!(self, OperatingMode::RmCompat)
    }

    /// Returns true if in rip mode
    pub fn is_rip(self) -> bool {
        matches!(self, OperatingMode::Rip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_detection_env() {
        env::set_var("RIP_MODE", "rm");
        assert_eq!(OperatingMode::detect(), OperatingMode::RmCompat);

        env::set_var("RIP_MODE", "rip");
        assert_eq!(OperatingMode::detect(), OperatingMode::Rip);

        env::remove_var("RIP_MODE");
    }

    // Note: Testing argv[0] detection requires integration tests
}
```

### Step 1.2: Modify main.rs

**File:** `src/main.rs`

```rust
use clap::{Args as _, Command, FromArgMatches as _};
use std::env;
use std::io;
use std::process::ExitCode;

use rip2::args::Commands;
use rip2::mode::OperatingMode;  // NEW
use rip2::{args, args_rm, completions, util};  // NEW: args_rm

fn main() -> ExitCode {
    // Detect operating mode FIRST
    let mode = OperatingMode::detect();

    // Dispatch to appropriate argument parser
    match mode {
        OperatingMode::Rip => run_rip_mode(),
        OperatingMode::RmCompat => run_rm_compat_mode(),
    }
}

fn run_rip_mode() -> ExitCode {
    // Existing rip logic (unchanged)
    let base_cmd = Command::new("rip");
    let cmd = args::Args::augment_args(base_cmd);
    let cli = args::Args::from_arg_matches(&cmd.get_matches()).unwrap();

    match &cli.command {
        Some(Commands::Completions { shell }) => {
            let result = completions::generate_shell_completions(shell, &mut io::stdout());
            if result.is_err() {
                eprintln!("{}", result.unwrap_err());
                return ExitCode::FAILURE;
            }
        }
        Some(Commands::Graveyard { seance }) => {
            let graveyard = rip2::get_graveyard(None);
            if *seance {
                let cwd = env::current_dir().expect("Failed to get current directory");
                let gravepath = util::join_absolute(
                    graveyard,
                    dunce::canonicalize(cwd).expect("Failed to get current directory"),
                );
                print!("{}", gravepath.display());
            } else {
                print!("{}", graveyard.display());
            }
        }
        None => {
            let mut stream = io::stdout();
            let production_mode = util::ProductionMode;

            let result = rip2::run(&cli, production_mode, &mut stream);

            if let Err(ref e) = result {
                println!("Exception: {e}");
                return ExitCode::FAILURE;
            }
        }
    }

    ExitCode::SUCCESS
}

fn run_rm_compat_mode() -> ExitCode {
    // NEW: rm-compatible mode
    let base_cmd = Command::new("rm");
    let cmd = args_rm::RmArgs::augment_args(base_cmd);

    let cli = match args_rm::RmArgs::from_arg_matches(&cmd.get_matches()) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::FAILURE;
        }
    };

    let mut stream = io::stdout();
    let production_mode = util::ProductionMode;

    // Call rm-compatible run function
    let result = rip2::run_rm_compat(&cli, production_mode, &mut stream);

    match result {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("rm: {}", e);
            ExitCode::FAILURE
        }
    }
}
```

### Step 1.3: Create rm argument parser

**File:** `src/args_rm.rs` (NEW)

```rust
//! GNU rm-compatible argument parsing

use clap::Parser;
use std::path::PathBuf;

/// GNU rm-compatible argument structure
#[derive(Parser, Debug)]
#[command(
    name = "rm",
    version,
    about = "remove files or directories",
    long_about = "Remove (unlink) the FILE(s).\n\nThis is a graveyard-enabled version of rm that moves files to a recoverable location instead of permanent deletion."
)]
pub struct RmArgs {
    /// Files or directories to remove
    #[arg(value_name = "FILE")]
    pub targets: Vec<PathBuf>,

    /// Ignore nonexistent files and arguments, never prompt
    #[arg(short = 'f', long = "force")]
    pub force: bool,

    /// Prompt before every removal
    #[arg(short = 'i', conflicts_with_all = ["interactive_once", "interactive"])]
    pub interactive_always: bool,

    /// Prompt once before removing more than three files, or when removing recursively
    #[arg(short = 'I', conflicts_with_all = ["interactive_always", "interactive"])]
    pub interactive_once: bool,

    /// Prompt according to WHEN: never, once (-I), or always (-i)
    #[arg(
        long = "interactive",
        value_name = "WHEN",
        conflicts_with_all = ["interactive_always", "interactive_once"]
    )]
    pub interactive: Option<String>,

    /// Remove directories and their contents recursively
    #[arg(short = 'r', short_alias = 'R', long = "recursive")]
    pub recursive: bool,

    /// Remove empty directories
    #[arg(short = 'd', long = "dir")]
    pub dir: bool,

    /// Explain what is being done
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// When removing a hierarchy recursively, skip any directory on a different filesystem
    #[arg(long = "one-file-system")]
    pub one_file_system: bool,

    /// Do not remove '/' (default); with 'all', reject any command line argument on a separate device
    #[arg(long = "preserve-root", value_name = "all", num_args = 0..=1)]
    pub preserve_root: Option<Option<String>>,

    /// Do not treat '/' specially
    #[arg(long = "no-preserve-root", conflicts_with = "preserve_root")]
    pub no_preserve_root: bool,

    /// Directory where deleted files rest (for testing/compatibility)
    #[arg(long = "graveyard", hide = true)]
    pub graveyard: Option<PathBuf>,

    /// Override all safety checks (DANGEROUS! Use with extreme caution)
    #[arg(long = "yes-i-am-100-percent-certain", hide = true)]
    pub override_safety: bool,
}

/// Interactive mode derived from arguments
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractiveMode {
    /// Never prompt (-f, --interactive=never)
    Never,
    /// Prompt once if >3 files or recursive (-I, --interactive=once)
    Once,
    /// Prompt before each removal (-i, --interactive=always)
    Always,
}

/// Preserve-root mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreserveRoot {
    /// No root protection (--no-preserve-root)
    None,
    /// Protect / only (--preserve-root, default)
    Root,
    /// Protect all mount points (--preserve-root=all)
    All,
}

impl RmArgs {
    /// Get the interactive mode from the parsed arguments
    pub fn get_interactive_mode(&self) -> InteractiveMode {
        // -f takes precedence
        if self.force {
            return InteractiveMode::Never;
        }

        // Check explicit flags
        if self.interactive_always {
            return InteractiveMode::Always;
        }

        if self.interactive_once {
            return InteractiveMode::Once;
        }

        // Check --interactive=WHEN
        if let Some(ref when) = self.interactive {
            return match when.to_lowercase().as_str() {
                "never" | "no" | "none" => InteractiveMode::Never,
                "once" => InteractiveMode::Once,
                "always" | "yes" => InteractiveMode::Always,
                _ => {
                    eprintln!("rm: invalid argument '{}' for '--interactive'", when);
                    eprintln!("Valid arguments are:");
                    eprintln!("  - 'never', 'no', 'none'");
                    eprintln!("  - 'once'");
                    eprintln!("  - 'always', 'yes'");
                    std::process::exit(1);
                }
            };
        }

        // Default: depends on stdin_tty (implement later)
        // For now, default to Never
        InteractiveMode::Never
    }

    /// Get the preserve-root mode from the parsed arguments
    pub fn get_preserve_root(&self) -> PreserveRoot {
        if self.no_preserve_root {
            return PreserveRoot::None;
        }

        // Check --preserve-root[=all]
        if let Some(ref opt) = self.preserve_root {
            return match opt.as_deref() {
                Some("all") => PreserveRoot::All,
                Some(other) => {
                    eprintln!("rm: invalid argument '{}' for '--preserve-root'", other);
                    eprintln!("Valid argument is: 'all'");
                    std::process::exit(1);
                }
                None => PreserveRoot::Root,
            };
        }

        // Default: protect root
        PreserveRoot::Root
    }

    /// Check if we should prompt once before all removals
    pub fn should_prompt_once(&self) -> bool {
        self.get_interactive_mode() == InteractiveMode::Once
            && (self.recursive || self.targets.len() > 3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interactive_mode() {
        // Test -f
        let args = RmArgs::parse_from(&["rm", "-f", "file"]);
        assert_eq!(args.get_interactive_mode(), InteractiveMode::Never);

        // Test -i
        let args = RmArgs::parse_from(&["rm", "-i", "file"]);
        assert_eq!(args.get_interactive_mode(), InteractiveMode::Always);

        // Test -I
        let args = RmArgs::parse_from(&["rm", "-I", "file"]);
        assert_eq!(args.get_interactive_mode(), InteractiveMode::Once);
    }

    #[test]
    fn test_preserve_root() {
        // Default
        let args = RmArgs::parse_from(&["rm", "file"]);
        assert_eq!(args.get_preserve_root(), PreserveRoot::Root);

        // --no-preserve-root
        let args = RmArgs::parse_from(&["rm", "--no-preserve-root", "file"]);
        assert_eq!(args.get_preserve_root(), PreserveRoot::None);

        // --preserve-root=all
        let args = RmArgs::parse_from(&["rm", "--preserve-root=all", "file"]);
        assert_eq!(args.get_preserve_root(), PreserveRoot::All);
    }

    #[test]
    fn test_prompt_once_logic() {
        // Not enough files, no recursive
        let args = RmArgs::parse_from(&["rm", "-I", "f1", "f2"]);
        assert!(!args.should_prompt_once());

        // >3 files
        let args = RmArgs::parse_from(&["rm", "-I", "f1", "f2", "f3", "f4"]);
        assert!(args.should_prompt_once());

        // Recursive
        let args = RmArgs::parse_from(&["rm", "-I", "-r", "dir"]);
        assert!(args.should_prompt_once());
    }
}
```

### Step 1.4: Update lib.rs to export new modules

**File:** `src/lib.rs`

Add at top:
```rust
pub mod mode;      // NEW
pub mod args_rm;   // NEW
pub mod rm_compat; // NEW (will create in Phase 3)
```

Add new function (placeholder for now):
```rust
/// Run in rm-compatible mode
pub fn run_rm_compat(
    _cli: &args_rm::RmArgs,
    _mode: impl util::TestingMode,
    _stream: &mut impl Write,
) -> Result<(), Error> {
    // TODO: Implement in Phase 3
    todo!("rm-compat mode not yet implemented")
}
```

---

## Phase 2: Core rm Functionality

### Step 2.1: Implement rm-compat core logic

**File:** `src/rm_compat.rs` (NEW)

```rust
//! GNU rm-compatible removal logic

use crate::args_rm::{InteractiveMode, PreserveRoot, RmArgs};
use crate::record::{Record, DEFAULT_FILE_LOCK};
use crate::util::{self, TestingMode};
use std::fs;
use std::io::{Error, ErrorKind, Write};
use std::path::{Path, PathBuf};

/// Run rm-compatible removal
pub fn run<const FILE_LOCK: bool>(
    cli: &RmArgs,
    graveyard: &PathBuf,
    mode: &impl TestingMode,
    stream: &mut impl Write,
) -> Result<(), Error> {
    // Handle empty targets
    if cli.targets.is_empty() {
        if cli.force {
            // -f with no arguments: success (GNU rm behavior)
            return Ok(());
        } else {
            return Err(Error::new(ErrorKind::InvalidInput, "missing operand"));
        }
    }

    // Prompt once if needed (before any removal)
    if cli.should_prompt_once() {
        let n_files = cli.targets.len();
        let message = if cli.recursive {
            format!("rm: remove {} arguments recursively? ", n_files)
        } else {
            format!("rm: remove {} arguments? ", n_files)
        };

        if !util::prompt_yes(&message, mode, stream)? {
            // User declined: exit with success (GNU rm behavior)
            return Ok(());
        }
    }

    // Create graveyard if needed
    if !graveyard.exists() {
        fs::create_dir_all(graveyard)?;
    }

    let record = Record::<FILE_LOCK>::new(graveyard);
    let interactive_mode = cli.get_interactive_mode();
    let preserve_root = cli.get_preserve_root();

    // Process each target
    for target in &cli.targets {
        remove_target(
            target,
            graveyard,
            &record,
            cli.recursive,
            cli.dir,
            cli.verbose,
            interactive_mode,
            preserve_root,
            cli.one_file_system,
            mode,
            stream,
        )?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn remove_target<const FILE_LOCK: bool>(
    target: &Path,
    graveyard: &PathBuf,
    record: &Record<FILE_LOCK>,
    recursive: bool,
    allow_empty_dir: bool,
    verbose: bool,
    interactive: InteractiveMode,
    preserve_root: PreserveRoot,
    one_file_system: bool,
    mode: &impl TestingMode,
    stream: &mut impl Write,
) -> Result<(), Error> {
    // TODO: Implement actual removal logic
    // For now, just print what we would do
    if verbose {
        writeln!(stream, "removed '{}'", target.display())?;
    }
    Ok(())
}
```

### Step 2.2: Implement preserve-root protection

**File:** `src/rm_compat.rs`

Add helper functions:
```rust
/// Check if path is the root filesystem
fn is_root_path(path: &Path) -> Result<bool, Error> {
    let canonical = dunce::canonicalize(path)?;

    // Check if path is exactly "/"
    #[cfg(unix)]
    {
        Ok(canonical.to_str() == Some("/"))
    }

    #[cfg(windows)]
    {
        // On Windows, check for drive root like C:\
        if let Some(s) = canonical.to_str() {
            Ok(s.len() == 3 && s.chars().nth(1) == Some(':') && s.chars().nth(2) == Some('\\'))
        } else {
            Ok(false)
        }
    }
}

/// Check home directory depth protection (rip2 safety enhancement)
///
/// Blocks recursive removal of paths fewer than 3 levels deep within /home
/// to prevent catastrophic mistakes like `rm -r ~` or `rm -r /home/user/Documents`
///
/// Examples:
/// - /home/jan              -> Error (depth 2)
/// - /home/jan/Desktop      -> Error (depth 3)
/// - /home/jan/Desktop/proj -> OK (depth 4)
fn check_home_directory_depth(path: &Path, override_safety: bool) -> Result<(), Error> {
    // Skip check if safety override enabled
    if override_safety {
        return Ok(());
    }

    let canonical = dunce::canonicalize(path)?;
    let path_str = canonical.to_string_lossy();

    #[cfg(unix)]
    {
        // Check if path is within /home
        if path_str.starts_with("/home/") {
            // Count directory depth: /home/user/dir1/dir2/dir3
            let parts: Vec<&str> = path_str.split('/').filter(|s| !s.is_empty()).collect();

            // parts[0] = "home"
            // parts[1] = username
            // parts[2] = first level dir
            // parts[3] = second level dir (minimum safe depth)

            if parts.len() < 4 {
                return Err(Error::new(
                    ErrorKind::PermissionDenied,
                    format!(
                        "refusing to recursively remove '{}': path is too shallow (depth protection)\n\
                         Hint: Use --yes-i-am-100-percent-certain to override (DANGEROUS!)",
                        path.display()
                    ),
                ));
            }
        }
    }

    #[cfg(windows)]
    {
        // Check if path is within C:\Users
        if path_str.starts_with("C:\\Users\\") || path_str.starts_with("C:/Users/") {
            // Normalize to forward slashes for easier parsing
            let normalized = path_str.replace('\\', "/");
            let parts: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();

            // parts[0] = "C:"
            // parts[1] = "Users"
            // parts[2] = username
            // parts[3] = first level dir (minimum safe depth)

            if parts.len() < 4 {
                return Err(Error::new(
                    ErrorKind::PermissionDenied,
                    format!(
                        "refusing to recursively remove '{}': path is too shallow (depth protection)\n\
                         Hint: Use --yes-i-am-100-percent-certain to override (DANGEROUS!)",
                        path.display()
                    ),
                ));
            }
        }
    }

    Ok(())
}

/// Get device ID of a path (for --preserve-root=all and --one-file-system)
#[cfg(unix)]
fn get_device_id(path: &Path) -> Result<u64, Error> {
    use std::os::unix::fs::MetadataExt;
    let metadata = fs::metadata(path)?;
    Ok(metadata.dev())
}

#[cfg(windows)]
fn get_device_id(path: &Path) -> Result<u64, Error> {
    // Windows: use volume serial number
    // For now, just return 0 (not implemented)
    Ok(0)
}
```

Update `remove_target` to add protection:
```rust
fn remove_target<const FILE_LOCK: bool>(
    target: &Path,
    graveyard: &PathBuf,
    record: &Record<FILE_LOCK>,
    recursive: bool,
    allow_empty_dir: bool,
    verbose: bool,
    interactive: InteractiveMode,
    preserve_root: PreserveRoot,
    one_file_system: bool,
    override_safety: bool,
    mode: &impl TestingMode,
    stream: &mut impl Write,
) -> Result<(), Error> {
    // FIRST: Check home directory depth protection (rip2 enhancement)
    // This runs BEFORE preserve-root check
    if recursive {
        check_home_directory_depth(target, override_safety)?;
    }

    // SECOND: Check preserve-root protection (GNU rm compatible)
    match preserve_root {
        PreserveRoot::None => {
            // No protection
        }
        PreserveRoot::Root | PreserveRoot::All => {
            if recursive && is_root_path(target)? {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!(
                        "refusing to remove '{}' recursively",
                        target.display()
                    ),
                ));
            }
        }
    }

    // Check if target exists
    let metadata = match fs::symlink_metadata(target) {
        Ok(m) => m,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            if cli.force {
                return Ok(()); // -f ignores missing files
            } else {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    format!("cannot remove '{}': No such file or directory", target.display()),
                ));
            }
        }
        Err(e) => return Err(e),
    };

    // Check directory handling
    if metadata.is_dir() {
        if !recursive && !allow_empty_dir {
            return Err(Error::new(
                ErrorKind::IsADirectory,
                format!("cannot remove '{}': Is a directory", target.display()),
            ));
        }

        if allow_empty_dir && !recursive {
            // -d flag: only remove if empty
            let is_empty = fs::read_dir(target)?.next().is_none();
            if !is_empty {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("cannot remove '{}': Directory not empty", target.display()),
                ));
            }
        }
    }

    // Interactive prompting (if ALWAYS mode)
    if interactive == InteractiveMode::Always {
        let file_type = if metadata.is_dir() {
            "directory"
        } else {
            "file"
        };
        let prompt = format!("rm: remove {} '{}'? ", file_type, target.display());
        if !util::prompt_yes(&prompt, mode, stream)? {
            return Ok(()); // User declined
        }
    }

    // TODO: Actually move to graveyard (use existing rip logic)
    // For now, just verbose output
    if verbose {
        let removed_type = if metadata.is_dir() {
            "directory"
        } else {
            ""
        };
        writeln!(
            stream,
            "removed {}'{}'",
            if removed_type.is_empty() { "" } else { "directory " },
            target.display()
        )?;
    }

    Ok(())
}
```

---

## Phase 3: Integration with Existing Rip Logic

### Step 3.1: Reuse graveyard code

The key insight: rm-compat mode should reuse rip's existing graveyard logic!

**Modify:** `src/lib.rs`

Update `run_rm_compat`:
```rust
pub fn run_rm_compat(
    cli: &args_rm::RmArgs,
    mode: impl util::TestingMode,
    stream: &mut impl Write,
) -> Result<(), Error> {
    let graveyard = get_graveyard(cli.graveyard.clone());

    // Call rm_compat module
    rm_compat::run::<DEFAULT_FILE_LOCK>(
        cli,
        &graveyard,
        &mode,
        stream,
    )
}
```

**In rm_compat.rs:** Reuse `bury_target` logic from lib.rs

Study the existing `bury_target` function to understand how to:
- Move files to graveyard
- Handle conflicts (numbered backups)
- Update the record
- Handle symlinks and special files

---

## Phase 4: Testing Strategy

### Step 4.1: Unit Tests

Create: `tests/rm_compat_unit_tests.rs`

```rust
use rip2::args_rm::{InteractiveMode, PreserveRoot, RmArgs};
use rip2::mode::OperatingMode;

#[test]
fn test_mode_detection_from_binary_name() {
    // This requires mocking argv[0]
    // Use integration tests instead
}

#[test]
fn test_interactive_mode_parsing() {
    let args = RmArgs::parse_from(&["rm", "-f", "file"]);
    assert_eq!(args.get_interactive_mode(), InteractiveMode::Never);

    let args = RmArgs::parse_from(&["rm", "-i", "file"]);
    assert_eq!(args.get_interactive_mode(), InteractiveMode::Always);
}

#[test]
fn test_preserve_root_parsing() {
    let args = RmArgs::parse_from(&["rm", "--no-preserve-root", "/"]);
    assert_eq!(args.get_preserve_root(), PreserveRoot::None);
}
```

### Step 4.2: Integration Tests

Create: `tests/rm_compat_integration_tests.rs`

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_rm_compat_mode_basic() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("test.txt");
    std::fs::write(&file, "content").unwrap();

    // Invoke as 'rm' (need to create symlink or set env)
    std::env::set_var("RIP_MODE", "rm");

    let mut cmd = Command::cargo_bin("rip").unwrap();
    cmd.arg(file.to_str().unwrap());
    cmd.assert().success();

    // File should be in graveyard, not deleted
    assert!(!file.exists());
    // TODO: Check graveyard

    std::env::remove_var("RIP_MODE");
}

#[test]
fn test_rm_directory_without_recursive() {
    let temp = TempDir::new().unwrap();
    let dir = temp.path().join("testdir");
    std::fs::create_dir(&dir).unwrap();

    std::env::set_var("RIP_MODE", "rm");

    let mut cmd = Command::cargo_bin("rip").unwrap();
    cmd.arg(dir.to_str().unwrap());
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Is a directory"));

    std::env::remove_var("RIP_MODE");
}

#[test]
fn test_rm_force_missing_file() {
    std::env::set_var("RIP_MODE", "rm");

    let mut cmd = Command::cargo_bin("rip").unwrap();
    cmd.args(&["-f", "/nonexistent/file"]);
    cmd.assert().success(); // -f ignores missing files

    std::env::remove_var("RIP_MODE");
}
```

### Step 4.3: Comparison Tests vs GNU rm

Create bash script: `tests/compare_with_gnu_rm.sh`

```bash
#!/bin/bash
# Compare rip rm-compat mode with GNU rm

set -euo pipefail

TEMP_RIP=$(mktemp -d)
TEMP_RM=$(mktemp -d)

# Create test files
create_test_files() {
    local dir=$1
    mkdir -p "$dir/testdir"
    echo "test" > "$dir/file1.txt"
    echo "test" > "$dir/file2.txt"
    mkdir -p "$dir/emptydir"
}

# Test 1: Directory without -r
echo "Test 1: Directory without -r"
create_test_files "$TEMP_RIP"
create_test_files "$TEMP_RM"

rip "$TEMP_RIP/testdir" 2>&1 | tee rip_out.txt || true
rm "$TEMP_RM/testdir" 2>&1 | tee rm_out.txt || true

# Compare error messages (should both say "Is a directory")
grep -q "Is a directory" rip_out.txt
grep -q "Is a directory" rm_out.txt

echo "Test 1: PASS"

# More tests...

# Cleanup
rm -rf "$TEMP_RIP" "$TEMP_RM"
rm -f rip_out.txt rm_out.txt

echo "All comparison tests passed!"
```

---

## Phase 5: Documentation & Polish

### Step 5.1: Update README.md

Add new section after installation instructions:

```markdown
## Using rip as rm replacement

rip can operate in GNU rm-compatible mode when invoked as `rm`. This provides
a safer alternative to rm while maintaining 100% command-line compatibility.

### Setup

Create a symlink from `rm` to `rip`:

\`\`\`bash
# Option 1: User-local (recommended)
ln -s $(which rip) ~/.local/bin/rm
# Make sure ~/.local/bin is in your PATH before /usr/bin

# Option 2: System-wide (requires sudo)
sudo ln -s $(which rip) /usr/local/bin/rm
\`\`\`

### Safety Features

When used as `rm`, all files still go to the graveyard:

- **Recover deleted files**: Use `rip -u` or `$(which rip) -u`
- **Root protection**: `--preserve-root` is ON by default
- **Interactive mode**: `-i` and `-I` flags work exactly like GNU rm

### Compatibility

100% compatible with GNU rm v9.4 flags:
- `-f`, `--force`: Never prompt, ignore errors
- `-i`: Interactive (prompt before each removal)
- `-I`: Interactive once (prompt if >3 files or recursive)
- `-r`, `-R`, `--recursive`: Remove directories recursively
- `-v`, `--verbose`: Show each file being removed
- `-d`, `--dir`: Remove empty directories
- `--preserve-root[=all]`: Protect `/` from removal (default: on)
- `--no-preserve-root`: Allow removing `/` (dangerous!)
- `--one-file-system`: Don't cross filesystem boundaries

### Example Usage

\`\`\`bash
rm file.txt                  # Remove file (goes to graveyard)
rm -i *.txt                  # Interactive removal
rm -rf directory/            # Recursive removal
rm -v file1 file2            # Verbose output
rip -u                       # Restore last deleted file
\`\`\`
```

### Step 5.2: Update man page

Generate updated man page from help text.

### Step 5.3: Shell completions

Update completion scripts for both `rip` and `rm` commands.

---

## Implementation Checklist

### Phase 1: Foundation ✓
- [ ] Create `src/mode.rs` with mode detection
- [ ] Create `src/args_rm.rs` with rm argument parsing
- [ ] Modify `src/main.rs` for mode dispatch
- [ ] Add module exports to `src/lib.rs`
- [ ] Write unit tests for mode detection
- [ ] Write unit tests for argument parsing

### Phase 2: Core rm Logic
- [ ] Create `src/rm_compat.rs` skeleton
- [ ] Implement preserve-root protection
- [ ] Implement directory/recursive checks
- [ ] Implement interactive prompting (ALWAYS mode)
- [ ] Implement prompt-once logic (ONCE mode)
- [ ] Implement verbose output
- [ ] Match rm error message format

### Phase 3: Integration
- [ ] Reuse existing graveyard movement logic
- [ ] Handle file conflicts (numbered backups)
- [ ] Update record keeping
- [ ] Handle special files (symlinks, FIFOs, etc.)
- [ ] Implement one-file-system checking
- [ ] Implement empty directory checking (-d flag)

### Phase 4: Testing
- [ ] Write unit tests for all new functions
- [ ] Write integration tests for rm-compat mode
- [ ] Create comparison tests vs GNU rm
- [ ] Test backward compatibility (rip mode still works)
- [ ] Test mode detection (symlink, env var)
- [ ] Test all flag combinations

### Phase 5: Polish
- [ ] Update README.md
- [ ] Update help text
- [ ] Generate shell completions
- [ ] Write migration guide
- [ ] Document known limitations
- [ ] Final code review

---

## Common Pitfalls to Avoid

1. **Don't break existing rip users**: Mode detection must be bulletproof
2. **Match rm behavior exactly**: Use comparison tests extensively
3. **Error messages matter**: Users expect specific rm error formats
4. **Exit codes matter**: Scripts depend on correct exit codes
5. **Preserve-root is critical**: Must be secure and correct
6. **Test with symlinks**: Both as targets and as the binary itself

---

## Performance Considerations

- Mode detection happens once at startup: negligible overhead
- Graveyard operations slightly slower than permanent deletion
- File locking prevents concurrent operation corruption
- Consider adding `--fast` flag for skipping record keeping?

---

## Future Enhancements

1. **Audit logging**: Record all rm operations for security
2. **Policy framework**: System admin can enforce safety rules
3. **Pure deletion mode**: Optional `--rm-pure` for actual rm behavior?
4. **Better Windows support**: Full rm emulation on Windows

---

**End of Implementation Guide**
