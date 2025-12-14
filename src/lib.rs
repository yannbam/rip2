use clap::CommandFactory;
use fs_extra::dir::get_size;
use std::fs::Metadata;
use std::io::{BufRead, BufReader, Error, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::{env, fs};
use walkdir::WalkDir;

/// Information needed to create a directory with specific permissions
#[derive(Debug, Clone)]
pub struct DirToCreate {
    pub path: PathBuf,
    pub permissions: Option<fs::Permissions>,
}

// Platform-specific imports
#[cfg(unix)]
use nix::libc;
#[cfg(unix)]
use nix::sys::stat::Mode;
#[cfg(unix)]
use nix::unistd::mkfifo;
#[cfg(unix)]
use std::os::unix::fs::{symlink, FileTypeExt, PermissionsExt};

#[cfg(target_os = "windows")]
use std::os::windows::fs::symlink_file as symlink;

pub mod args;
pub mod completions;
pub mod record;
pub mod safety;
pub mod util;

use std::ffi::OsStr;

// Re-export ExecutionMode from args module
pub use args::ExecutionMode;

/// Detect execution mode from given inputs (pure function for testability)
///
/// # Arguments
/// * `env_mode` - Value of RIP_MODE environment variable, if set
/// * `binary_name` - The filename portion of argv[0]
///
/// # Priority
/// 1. RIP_MODE env ("rm" or "rip") — highest priority, for testing
/// 2. Binary name — if exactly "rm", use Rm mode
/// 3. Default to Rip mode
pub fn detect_mode_from_inputs(env_mode: Option<&str>, binary_name: &OsStr) -> ExecutionMode {
    // Check environment variable first (highest priority, for testing)
    if let Some(mode_str) = env_mode {
        match mode_str.to_lowercase().as_str() {
            "rm" => return ExecutionMode::Rm,
            "rip" => return ExecutionMode::Rip,
            // Invalid values fall through to binary name detection
            _ => {}
        }
    }

    // Check if binary name is exactly "rm"
    // This handles: "rm", "./rm", "/usr/bin/rm" (after Path::file_name extraction)
    // Does NOT match: "safe-rm", "rip", etc.
    if binary_name == "rm" {
        return ExecutionMode::Rm;
    }

    // Default to Rip mode
    ExecutionMode::Rip
}

// ============================================================================

use args::RipArgs;
use record::{Record, RecordItem, DEFAULT_FILE_LOCK};
use safety::{require_root_for_permanent_deletion, RootChecker};

const LINES_TO_INSPECT: usize = 6;
const FILES_TO_INSPECT: usize = 6;
pub const BIG_FILE_THRESHOLD: u64 = 500_000_000; // 500 MB

pub fn run(
    cli: &RipArgs,
    mode: impl util::TestingMode,
    stream: &mut impl Write,
    root_checker: &impl RootChecker,
) -> Result<(), Error> {
    args::validate_rip_args(cli)?;
    let graveyard: &PathBuf = &get_graveyard(cli.graveyard.clone());

    if !graveyard.exists() {
        fs::create_dir_all(graveyard)?;

        #[cfg(unix)]
        {
            fs::set_permissions(graveyard, fs::Permissions::from_mode(0o700))?;
        }
    }

    // Stores the deleted files
    let record = Record::<DEFAULT_FILE_LOCK>::new(graveyard);
    let cwd = &env::current_dir()?;

    // If the user wishes to restore everything
    if cli.decompose {
        // In force mode, skip the prompt to decompose
        if cli.force || util::prompt_yes("Really unlink the entire graveyard?", &mode, stream)? {
            // Require root privileges for permanent deletion
            require_root_for_permanent_deletion(root_checker)?;
            fs::remove_dir_all(graveyard)?;
        }
    } else if let Some(ref mut graves_to_exhume) = cli.unbury.clone() {
        // Vector to hold the grave path of items we want to unbury.
        // This will be used to determine which items to remove from the
        // record following the unbury.
        // Initialize it with the targets passed to -r

        // If -s is also passed, push all files found by seance onto
        // the graves_to_exhume.
        if cli.seance && record.open().is_ok() {
            let gravepath = util::join_absolute(graveyard, dunce::canonicalize(cwd)?);
            for grave in record.seance(&gravepath)? {
                graves_to_exhume.push(grave.dest);
            }
        }

        // Otherwise, add the last deleted file
        if graves_to_exhume.is_empty() {
            if let Ok(s) = record.get_last_bury() {
                graves_to_exhume.push(s);
            }
        }

        let allow_rename = util::allow_rename();

        // Go through the graveyard and exhume all the graves
        for line in record.lines_of_graves(graves_to_exhume) {
            let entry = RecordItem::new(&line);
            let orig: PathBuf = if util::symlink_exists(&entry.orig) {
                util::rename_grave(&entry.orig)
            } else {
                PathBuf::from(&entry.orig)
            };
            let dirs_to_create = build_dirs_to_create_from_graveyard(&entry.dest, &orig);

            move_target(
                &entry.dest,
                &orig,
                allow_rename,
                &mode,
                stream,
                cli.force,
                &dirs_to_create,
                root_checker,
            )
            .map_err(|e| {
                Error::new(
                    e.kind(),
                    format!(
                        "Unbury failed: couldn't copy files from {} to {}",
                        entry.dest.display(),
                        orig.display()
                    ),
                )
            })?;
            writeln!(
                stream,
                "Returned {} to {}",
                entry.dest.display(),
                orig.display()
            )?;
        }
        record.log_exhumed_graves(graves_to_exhume)?;
    } else if cli.seance {
        let gravepath = util::join_absolute(graveyard, dunce::canonicalize(cwd)?);
        writeln!(stream, "{: <19}\tpath", "deletion_time")?;
        for grave in record.seance(&gravepath)? {
            let formatted_time = grave.format_time_for_display()?;
            writeln!(stream, "{}\t{}", formatted_time, grave.dest.display())?;
        }
    } else if cli.targets.is_empty() {
        RipArgs::command().print_help()?;
    } else {
        let allow_rename = util::allow_rename();
        for target in &cli.targets {
            bury_target(
                target,
                graveyard,
                &record,
                cwd,
                cli.inspect,
                allow_rename,
                &mode,
                stream,
                cli.force,
                root_checker,
            )?;
        }
    }

    Ok(())
}

/// Result from rm mode execution.
/// Contains success flag and any error messages printed.
pub struct RmResult {
    /// True if all operations succeeded
    pub success: bool,
}

/// Run in rm mode with POSIX-compatible behavior.
///
/// Unlike `run()`, this handles rm-specific semantics:
/// - Each target is processed independently (errors don't stop processing)
/// - `-f` silences "no such file" errors
/// - Directories require `-r` or `-d` flag
/// - Verbose output with `-v`
///
/// Returns `RmResult` indicating overall success.
pub fn run_rm(
    cli: &args::RmArgs,
    mode: impl util::TestingMode,
    stream: &mut impl Write,
    error_stream: &mut impl Write,
    root_checker: &impl safety::RootChecker,
) -> RmResult {
    use record::{Record, DEFAULT_FILE_LOCK};

    let graveyard: PathBuf = get_graveyard(None);

    // Ensure graveyard exists
    if !graveyard.exists() {
        if let Err(e) = fs::create_dir_all(&graveyard) {
            writeln!(error_stream, "rm: cannot create graveyard: {}", e).ok();
            return RmResult { success: false };
        }

        #[cfg(unix)]
        {
            if let Err(e) = fs::set_permissions(&graveyard, fs::Permissions::from_mode(0o700)) {
                writeln!(error_stream, "rm: cannot set graveyard permissions: {}", e).ok();
                return RmResult { success: false };
            }
        }
    }

    let record = Record::<DEFAULT_FILE_LOCK>::new(&graveyard);
    let cwd = match env::current_dir() {
        Ok(cwd) => cwd,
        Err(e) => {
            writeln!(error_stream, "rm: cannot get current directory: {}", e).ok();
            return RmResult { success: false };
        }
    };

    let allow_rename = util::allow_rename();
    let mut had_error = false;

    // Process each target independently (rm doesn't stop on first error)
    for target in &cli.targets {
        // Check preserve-root protection
        // Canonicalize to catch things like "/../" or symlinks to /
        let canonical = dunce::canonicalize(target).ok();
        let is_root = canonical.as_ref().map(|p| p == Path::new("/")).unwrap_or(false)
            || target.as_os_str() == "/";

        if is_root {
            if cli.should_preserve_root() {
                writeln!(
                    error_stream,
                    "rm: it is dangerous to operate recursively on '/'",
                )
                .ok();
                writeln!(
                    error_stream,
                    "rm: use --no-preserve-root to override this failsafe",
                )
                .ok();
                had_error = true;
                continue;
            } else {
                // --no-preserve-root requires root privileges
                if !root_checker.is_root() {
                    writeln!(
                        error_stream,
                        "rm: cannot remove '/': Operation not permitted (requires root)",
                    )
                    .ok();
                    had_error = true;
                    continue;
                }
                // Root user with --no-preserve-root: allow the operation
            }
        }

        // Check home directory depth protection (only for recursive operations)
        if cli.recursive {
            if let Some(home) = dirs::home_dir() {
                if let Some(ref canon) = canonical {
                    // Check if path is under home directory
                    if let Ok(relative) = canon.strip_prefix(&home) {
                        // Count depth: ~/foo = 1, ~/foo/bar = 2, etc.
                        let depth = relative.components().count();
                        if depth <= 1 {
                            // Block direct children of home: ~/Desktop, ~/Documents, etc.
                            writeln!(
                                error_stream,
                                "rm: refusing to remove '{}': protected home directory location",
                                target.display()
                            )
                            .ok();
                            writeln!(
                                error_stream,
                                "rm: use 'rip' command if you really want to move this to graveyard",
                            )
                            .ok();
                            had_error = true;
                            continue;
                        }
                    }
                }
            }
        }

        // Check if target exists
        let metadata = match fs::symlink_metadata(target) {
            Ok(m) => m,
            Err(_) => {
                // File doesn't exist
                if cli.force {
                    // -f: silently ignore nonexistent files
                    continue;
                } else {
                    writeln!(
                        error_stream,
                        "rm: cannot remove '{}': No such file or directory",
                        target.display()
                    )
                    .ok();
                    had_error = true;
                    continue;
                }
            }
        };

        // Check directory handling
        if metadata.is_dir() {
            if !cli.recursive && !cli.dir {
                // Need -r or -d to remove directories
                writeln!(
                    error_stream,
                    "rm: cannot remove '{}': Is a directory",
                    target.display()
                )
                .ok();
                had_error = true;
                continue;
            }

            // -d only works on empty directories
            if cli.dir && !cli.recursive {
                // Check if directory is empty
                match fs::read_dir(target) {
                    Ok(mut entries) => {
                        if entries.next().is_some() {
                            writeln!(
                                error_stream,
                                "rm: cannot remove '{}': Directory not empty",
                                target.display()
                            )
                            .ok();
                            had_error = true;
                            continue;
                        }
                    }
                    Err(e) => {
                        writeln!(
                            error_stream,
                            "rm: cannot remove '{}': {}",
                            target.display(),
                            e
                        )
                        .ok();
                        had_error = true;
                        continue;
                    }
                }
            }
        }

        // Perform the burial (move to graveyard)
        match bury_target(
            target,
            &graveyard,
            &record,
            &cwd,
            false, // inspect: rm mode never prompts for inspection
            allow_rename,
            &mode,
            stream,
            cli.force,
            root_checker,
        ) {
            Ok(()) => {
                // Verbose output
                if cli.verbose {
                    writeln!(stream, "removed '{}'", target.display()).ok();
                }
            }
            Err(e) => {
                // Format error in rm style
                let reason = match e.kind() {
                    ErrorKind::PermissionDenied => "Permission denied".to_string(),
                    ErrorKind::NotFound => "No such file or directory".to_string(),
                    _ => e.to_string(),
                };
                writeln!(
                    error_stream,
                    "rm: cannot remove '{}': {}",
                    target.display(),
                    reason
                )
                .ok();
                had_error = true;
            }
        }
    }

    RmResult { success: !had_error }
}

#[allow(clippy::too_many_arguments)]
fn bury_target<const FILE_LOCK: bool>(
    target: &PathBuf,
    graveyard: &PathBuf,
    record: &Record<FILE_LOCK>,
    cwd: &Path,
    inspect: bool,
    allow_rename: bool,
    mode: &impl util::TestingMode,
    stream: &mut impl Write,
    force: bool,
    root_checker: &impl RootChecker,
) -> Result<(), Error> {
    // Check if source exists
    let metadata = &fs::symlink_metadata(target).map_err(|_| {
        Error::new(
            ErrorKind::NotFound,
            format!(
                "Cannot remove {}: no such file or directory",
                target.to_str().unwrap()
            ),
        )
    })?;
    // Canonicalize the path unless it's a symlink
    let source = &if metadata.file_type().is_symlink() {
        cwd.join(target)
    } else {
        dunce::canonicalize(cwd.join(target))
            .map_err(|e| Error::new(e.kind(), "Failed to canonicalize path"))?
    };

    if inspect && !should_we_bury_this(target, source, metadata, mode, stream)? {
        // User chose to not bury the file
    } else if source.starts_with(
        dunce::canonicalize(graveyard)
            .map_err(|e| Error::new(e.kind(), "Failed to canonicalize graveyard path"))?,
    ) {
        // If rip is called on a file already in the graveyard, prompt
        // to permanently delete it instead.
        if force
            || util::prompt_yes(
                format!(
                    "{} is already in the graveyard.\nPermanently unlink it?",
                    source.display()
                ),
                mode,
                stream,
            )?
        {
            // Require root privileges for permanent deletion
            require_root_for_permanent_deletion(root_checker)?;
            if fs::remove_dir_all(source).is_err() {
                fs::remove_file(source).map_err(|e| {
                    Error::new(e.kind(), format!("Couldn't unlink {}", source.display()))
                })?;
            }
        } else {
            writeln!(stream, "Skipping {}", source.display())?;
            // TODO: In the original code, this was a hard return from the entire
            // method (i.e., `run`). I think it should just be a return from the bury
            // (meaning a `continue` in the original code's loop). But I'm not sure.
        }
    } else {
        let (dest, dirs_to_create) = build_graveyard_dest(graveyard, source);
        let dest: &Path = &{
            // Resolve a name conflict if necessary
            if util::symlink_exists(&dest) {
                util::rename_grave(dest)
            } else {
                dest
            }
        };

        let moved = move_target(
            source,
            dest,
            allow_rename,
            mode,
            stream,
            force,
            &dirs_to_create,
            root_checker,
        )
        .map_err(|e| {
            fs::remove_dir_all(dest).ok();
            Error::new(e.kind(), "Failed to bury file")
        })?;

        if moved {
            // Clean up any partial buries due to permission error
            record.write_log(source, dest)?;
        }
    }

    Ok(())
}

fn should_we_bury_this(
    target: &Path,
    source: &PathBuf,
    metadata: &Metadata,
    mode: &impl util::TestingMode,
    stream: &mut impl Write,
) -> Result<bool, Error> {
    if metadata.is_dir() {
        // Get the size of the directory and all its contents
        {
            let num_bytes = get_size(source).map_err(|_| {
                Error::other(format!(
                    "Failed to get size of directory: {}",
                    source.display()
                ))
            })?;
            writeln!(
                stream,
                "{}: directory, {} including:",
                target.to_str().unwrap(),
                util::humanize_bytes(num_bytes)
            )?;
        }

        // Print the first few top-level files in the directory
        for entry in WalkDir::new(source)
            .sort_by(|a, b| a.file_name().cmp(b.file_name()))
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(Result::ok)
            .take(FILES_TO_INSPECT)
        {
            writeln!(stream, "{}", entry.path().display())?;
        }
    } else {
        writeln!(
            stream,
            "{}: file, {}",
            &target.to_str().unwrap(),
            util::humanize_bytes(metadata.len())
        )?;
        // Read the file and print the first few lines
        if let Ok(source_file) = fs::File::open(source) {
            for line in BufReader::new(source_file)
                .lines()
                .take(LINES_TO_INSPECT)
                .filter_map(Result::ok)
            {
                writeln!(stream, "> {line}")?;
            }
        } else {
            writeln!(stream, "Error reading {}", source.display())?;
        }
    }
    util::prompt_yes(
        format!("Send {} to the graveyard?", target.to_str().unwrap()),
        mode,
        stream,
    )
}

/// Plan graveyard directory structure and permissions
fn build_graveyard_dest(graveyard: &Path, source: &Path) -> (PathBuf, Vec<DirToCreate>) {
    let mut dest = graveyard.to_path_buf();
    let mut dirs_to_create = Vec::new();
    let mut cumulative_source = PathBuf::new();

    for component in source.components() {
        // Build cumulative source path
        cumulative_source.push(component.as_os_str());

        // Process component for destination using shared logic
        if util::push_component_to_dest(&mut dest, &component) {
            // Only add directories to the list (skip the final file component)
            if cumulative_source.is_dir() {
                let permissions = fs::metadata(&cumulative_source)
                    .map(|m| m.permissions())
                    .ok();
                dirs_to_create.push(DirToCreate {
                    path: dest.clone(),
                    permissions,
                });
            }
        }
    }

    (dest, dirs_to_create)
}

/// Plan source directory structure and permissions
fn build_dirs_to_create_from_graveyard(
    graveyard_path: &Path,
    orig_path: &Path,
) -> Vec<DirToCreate> {
    let mut dirs_to_create = Vec::new();

    // Walk from file to root and collect permissions to propagate
    let mut graveyard_current = graveyard_path.parent();
    let mut orig_current = orig_path.parent();

    while let (Some(g), Some(o)) = (graveyard_current, orig_current) {
        let permissions = fs::metadata(g).map(|m| m.permissions()).ok();
        dirs_to_create.push(DirToCreate {
            path: o.to_path_buf(),
            permissions,
        });

        // Move up one level
        graveyard_current = g.parent();
        orig_current = o.parent();
    }
    dirs_to_create.reverse();
    dirs_to_create
}

/// Create directories with specified permissions
fn create_dirs_with_permissions(dirs_to_create: &[DirToCreate]) -> Result<(), Error> {
    // Create directories one by one in order (parent to child)
    // This assumes dirs_to_create is ordered from root to leaf
    for dir in dirs_to_create {
        if !dir.path.exists() {
            // Create just this directory (parent should already exist)
            fs::create_dir(&dir.path).map_err(|e| {
                Error::new(
                    e.kind(),
                    format!("Failed to create directory {}: {}", dir.path.display(), e),
                )
            })?;

            // Set permissions if we have them
            if let Some(perms) = &dir.permissions {
                fs::set_permissions(&dir.path, perms.clone()).map_err(|e| {
                    Error::new(
                        e.kind(),
                        format!("Failed to set permissions on {}: {}", dir.path.display(), e),
                    )
                })?;
            }
        }
    }

    Ok(())
}

/// Move a target to a given destination, copying if necessary.
/// Returns true if the target was moved, false if it was not (due to
/// user input)
pub fn move_target(
    target: &Path,
    dest: &Path,
    allow_rename: bool,
    mode: &impl util::TestingMode,
    stream: &mut impl Write,
    force: bool,
    dirs_to_create: &[DirToCreate],
    root_checker: &impl RootChecker,
) -> Result<bool, Error> {
    // Try a simple rename, which will only work within the same mount point.
    // Trying to rename across filesystems will throw errno 18.
    if allow_rename && fs::rename(target, dest).is_ok() {
        return Ok(true);
    }

    // If that didn't work, then we need to copy and rm.
    create_dirs_with_permissions(dirs_to_create)?;

    if fs::symlink_metadata(target)?.is_dir() {
        move_dir(target, dest, mode, stream, force, root_checker)
    } else {
        let moved = copy_file(target, dest, mode, stream, force, root_checker).map_err(|e| {
            Error::new(
                e.kind(),
                format!(
                    "Failed to copy file from {} to {}",
                    target.display(),
                    dest.display()
                ),
            )
        })?;
        fs::remove_file(target).map_err(|e| {
            Error::new(
                e.kind(),
                format!("Failed to remove file: {}", target.display()),
            )
        })?;
        Ok(moved)
    }
}

/// Move a target which is a directory to a given destination, copying if necessary.
/// Returns true *always*, as the creation of the directory is enough to mark it as successful.
pub fn move_dir(
    target: &Path,
    dest: &Path,
    mode: &impl util::TestingMode,
    stream: &mut impl Write,
    force: bool,
    root_checker: &impl RootChecker,
) -> Result<bool, Error> {
    // Walk the source, creating directories and copying files as needed
    for entry in WalkDir::new(target).into_iter().filter_map(Result::ok) {
        // Path without the top-level directory
        let orphan = entry
            .path()
            .strip_prefix(target)
            .map_err(|_| Error::other("Parent directory isn't a prefix of child directories?"))?;

        if entry.file_type().is_dir() {
            let dest_dir = dest.join(orphan);
            fs::create_dir_all(&dest_dir).map_err(|e| {
                Error::new(
                    e.kind(),
                    format!(
                        "Failed to create dir: {} in {}",
                        entry.path().display(),
                        dest_dir.display()
                    ),
                )
            })?;

            // Preserve directory permissions
            let source_metadata = fs::metadata(entry.path()).map_err(|e| {
                Error::new(
                    e.kind(),
                    format!("Failed to get metadata for: {}", entry.path().display()),
                )
            })?;
            let source_perms = source_metadata.permissions();
            fs::set_permissions(&dest_dir, source_perms).map_err(|e| {
                Error::new(
                    e.kind(),
                    format!("Failed to set permissions on: {}", dest_dir.display()),
                )
            })?;
        } else {
            copy_file(entry.path(), &dest.join(orphan), mode, stream, force, root_checker).map_err(|e| {
                Error::new(
                    e.kind(),
                    format!(
                        "Failed to copy file from {} to {}",
                        entry.path().display(),
                        dest.join(orphan).display()
                    ),
                )
            })?;
        }
    }
    fs::remove_dir_all(target).map_err(|e| {
        Error::new(
            e.kind(),
            format!("Failed to remove dir: {}", target.display()),
        )
    })?;

    Ok(true)
}

pub fn copy_file(
    source: &Path,
    dest: &Path,
    mode: &impl util::TestingMode,
    stream: &mut impl Write,
    force: bool,
    root_checker: &impl RootChecker,
) -> Result<bool, Error> {
    let metadata = fs::symlink_metadata(source)?;
    let filetype = metadata.file_type();

    // Big file handling: offer permanent deletion option ONLY to root users
    // Safety invariant: non-root users can NEVER permanently delete files
    if metadata.len() > BIG_FILE_THRESHOLD {
        if root_checker.is_root() {
            // Root user: offer choice between copying and permanent deletion
            if !force
                && util::prompt_yes(
                    format!(
                        "About to copy a big file ({} is {})\nPermanently delete this file instead?",
                        source.display(),
                        util::humanize_bytes(metadata.len())
                    ),
                    mode,
                    stream,
                )?
            {
                return Ok(false);
            }
        }
        // Non-root users: always copy to graveyard (no permanent deletion option)
    }

    if filetype.is_file() {
        fs::copy(source, dest)?;
        return Ok(true);
    }

    #[cfg(unix)]
    if filetype.is_fifo() {
        let perm: libc::mode_t = (metadata.permissions().mode() & 0o777) as libc::mode_t;
        let mode = Mode::from_bits_truncate(perm);

        mkfifo(dest, mode)?;
        return Ok(true);
    }

    if filetype.is_symlink() {
        let target = fs::read_link(source)?;
        symlink(target, dest)?;
        return Ok(true);
    }

    match fs::copy(source, dest) {
        Err(e) => {
            // Special file: Try copying it as normal, but this probably won't work
            // Safety invariant: only root can permanently delete files
            if root_checker.is_root()
                && !force
                && util::prompt_yes(
                    format!(
                        "Non-regular file or directory: {}\nPermanently delete the file?",
                        source.display()
                    ),
                    mode,
                    stream,
                )?
            {
                Ok(false)
            } else {
                // Non-root users: error on special files that can't be copied
                // (they cannot choose permanent deletion)
                Err(e)
            }
        }
        Ok(_) => Ok(true),
    }
}

pub fn get_graveyard(graveyard: Option<PathBuf>) -> PathBuf {
    graveyard.map_or_else(
        || {
            if let Ok(env_graveyard) = env::var("RIP_GRAVEYARD") {
                PathBuf::from(env_graveyard)
            } else if let Ok(mut env_graveyard) = env::var("XDG_DATA_HOME") {
                if !env_graveyard.ends_with(std::path::MAIN_SEPARATOR) {
                    env_graveyard.push(std::path::MAIN_SEPARATOR);
                }
                env_graveyard.push_str("graveyard");
                PathBuf::from(env_graveyard)
            } else if let Some(home) = dirs::home_dir() {
                // Default: ~/.graveyard
                home.join(".graveyard")
            } else {
                // Fallback if home directory cannot be determined
                let user = util::get_user();
                PathBuf::from("/var/tmp").join(format!("graveyard-{user}"))
            }
        },
        |flag| flag,
    )
}

/// Testing module for exposing internal functions to unit tests.
/// This module is only used for testing purposes and should not be used in production code.
pub mod testing {
    use super::{should_we_bury_this, util, Error, Metadata, Path, PathBuf, Write};

    pub fn testable_should_we_bury_this(
        target: &Path,
        source: &PathBuf,
        metadata: &Metadata,
        stream: &mut impl Write,
    ) -> Result<bool, Error> {
        should_we_bury_this(target, source, metadata, &util::TestMode, stream)
    }
}
