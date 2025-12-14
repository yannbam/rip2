use lazy_static::lazy_static;
use rip2::args::{validate_rip_args, RipArgs, RipCommands, RmArgs};
use rip2::safety::MockRootChecker;
use rip2::run_rm;
use rip2::completions;
use rip2::util::{humanize_bytes, TestMode};
use rstest::rstest;
use std::fs;
use std::io::{Cursor, ErrorKind};
use std::path::PathBuf;
use std::process;
use std::sync::{Mutex, MutexGuard, PoisonError};
use tempfile::tempdir;

#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::os::unix::net::UnixListener;

#[cfg(target_os = "windows")]
use std::os::windows::fs::symlink_file as symlink;

#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;

lazy_static! {
    static ref GLOBAL_LOCK: Mutex<()> = Mutex::new(());
}

fn aquire_lock() -> MutexGuard<'static, ()> {
    GLOBAL_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

#[rstest]
fn test_validation() {
    let bad_completions = RipArgs {
        command: Some(RipCommands::Completions {
            shell: "bash".to_string(),
        }),
        decompose: true,
        ..RipArgs::default()
    };
    validate_rip_args(&bad_completions).expect_err("--completions can only be used by itself");

    let bad_decompose = RipArgs {
        decompose: true,
        seance: true,
        ..RipArgs::default()
    };
    validate_rip_args(&bad_decompose).expect_err("-d,--decompose can only be used with --graveyard");
}

#[rstest]
fn test_filetypes(
    #[values("regular", "big", "fifo", "symlink", "socket")] file_type: &str,
    #[values(false, true)] copy: bool,
) {
    if ["big", "socket"].contains(&file_type) && !copy {
        return;
    }

    #[cfg(target_os = "windows")]
    {
        if ["fifo", "socket"].contains(&file_type) {
            return;
        }
    }
    let tmpdir = tempdir().unwrap();
    let path = PathBuf::from(tmpdir.path());
    let source_path = path.join("test_file");
    let dest_path = path.join("test_file_copy");

    match file_type {
        "regular" => {
            fs::File::create(&source_path).unwrap();
        }
        "big" => {
            let file = fs::File::create(&source_path).unwrap();
            let len = rip2::BIG_FILE_THRESHOLD + 1;
            file.set_len(len).unwrap();
        }
        "fifo" => {
            process::Command::new("mkfifo")
                .arg("-m")
                .arg("700")
                .arg(&source_path)
                .status()
                .expect("mkfifo failed");
        }
        "symlink" => {
            let target_path = path.join("symlink_target");
            fs::File::create(&target_path).unwrap();
            symlink(&target_path, &source_path).unwrap();
        }
        "socket" => {
            #[cfg(unix)]
            {
                UnixListener::bind(&source_path).unwrap();
            }
        }
        _ => unreachable!(),
    }

    let mut log = Vec::new();
    let mode = TestMode;

    if copy {
        rip2::copy_file(&source_path, &dest_path, &mode, &mut log, false).unwrap();
    } else {
        rip2::move_target(&source_path, &dest_path, true, &mode, &mut log, false, &[]).unwrap();
    }

    let log_s = String::from_utf8(log).unwrap();

    // Check logs
    match file_type {
        "big" => {
            assert!(log_s.contains("About to copy a big file"));
        }
        "socket" => {
            assert!(log_s.contains("Non-regular file or directory:"));
            assert!(log_s.contains("Permanently delete the file?"));
        }
        _ => {
            assert!(log_s.is_empty());
        }
    }

    // Check graveyard contents and file type
    // let metadata = fs::symlink_metadata(dest_path).unwrap();
    // let ftype = metadata.file_type();
    let ftype = fs::symlink_metadata(&dest_path).map(|m| m.file_type());
    match file_type {
        "regular" => {
            assert!(dest_path.is_file());
            assert!(ftype.unwrap().is_file());
        }
        "big" => {
            assert!(!dest_path.exists());
        }
        "fifo" => {
            #[cfg(unix)]
            {
                // Dest must be a FIFO …
                assert!(dest_path.exists(), "FIFO not created in graveyard");
                let meta = fs::symlink_metadata(&dest_path).unwrap();
                assert!(meta.file_type().is_fifo(), "dest is not a FIFO");

                // … and keep the exact 0o700 mode.
                let mode_bits = meta.permissions().mode() & 0o777;
                assert_eq!(mode_bits, 0o700, "expected mode 0700, got {mode_bits:o}");
            }
        }
        "symlink" => {
            assert!(dest_path.exists());
            assert!(ftype.unwrap().is_symlink());
        }
        "socket" => {
            // Socket files are not copied, so are instead simply deleted
            assert!(!dest_path.exists());
        }
        _ => {}
    }
}

#[rstest]
fn test_prompt_read(#[values("y", "Y", "n", "N", "", "\n", "q", "Q", "k")] key: &str) {
    let input = Cursor::new(key);
    let result = rip2::util::yes_no_quit(input);
    match key {
        "y" | "Y" => assert!(result.unwrap()),
        "n" | "N" | "" | "\n" => assert!(!result.unwrap()),
        "q" | "Q" => {
            let err = result.unwrap_err();
            assert_eq!(err.kind(), ErrorKind::Interrupted);
            assert_eq!(err.to_string(), "User requested to quit");
        }
        "k" => {
            let err = result.unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput);
            assert_eq!(err.to_string(), "Invalid input");
        }
        _ => {}
    }
}

#[rstest]
fn test_completions(
    #[values("bash", "elvish", "fish", "powershell", "zsh", "nushell", "fake")] shell: &str,
) {
    let mut output = Vec::new();
    let result = completions::generate_shell_completions(shell, &mut output);
    let output_s = String::from_utf8(output).unwrap();
    match shell {
        "bash" => {
            assert!(output_s.contains("complete -F"));
        }
        "elvish" => {
            assert!(output_s.contains("set edit:completion:arg-completer[rip]"));
        }
        "fish" => {
            assert!(output_s.contains("complete -c"));
        }
        "powershell" => {
            assert!(output_s.contains("Register-ArgumentCompleter"));
        }
        "zsh" => {
            assert!(output_s.contains("compdef"));
        }
        "nushell" => {
            assert!(output_s.contains("export extern"));
        }
        "fake" => {
            assert!(result.is_err());
            let err_msg = result.unwrap_err().to_string();
            assert!(err_msg.contains("Invalid shell specification: fake"));
            assert!(
                err_msg.contains("Available shells: bash, elvish, fish, powershell, zsh, nushell")
            );
        }
        _ => {}
    }
}

#[rstest]
fn test_graveyard_path() {
    let _env_lock = aquire_lock();

    // Clear env:
    std::env::remove_var("RIP_GRAVEYARD");
    std::env::remove_var("XDG_DATA_HOME");

    // Check default graveyard path (now ~/.graveyard)
    let graveyard = rip2::get_graveyard(None);
    assert_eq!(
        graveyard,
        dirs::home_dir().unwrap().join(".graveyard")
    );
}

#[rstest]
fn test_humanize_bytes() {
    assert_eq!(humanize_bytes(0), "0 B");
    assert_eq!(humanize_bytes(1), "1 B");
    assert_eq!(humanize_bytes(1024), "1.0 KiB");
    assert_eq!(humanize_bytes(1024 * 1024), "1.0 MiB");
    assert_eq!(humanize_bytes(1024 * 1024 * 1024), "1.0 GiB");
    assert_eq!(humanize_bytes(1024 * 1024 * 1024 * 1024), "1.0 TiB");

    assert_eq!(humanize_bytes(1024 * 1024 + 1024 * 512), "1.5 MiB");
}

#[rstest]
fn fail_move_dir() {
    let tmpdir_dest = tempdir().unwrap();
    let tmpdir_target = tempdir().unwrap();
    let path_dest = PathBuf::from(tmpdir_dest.path());
    let path_target = PathBuf::from(tmpdir_target.path());
    let dest = path_dest.join("foo");
    let target = path_target.join("bar");
    let mut log = Vec::new();
    let results = rip2::move_dir(&target, &dest, &TestMode, &mut log, false);
    assert!(results.is_err());
    if let Err(e) = results {
        assert!(e.to_string().contains("Failed to remove dir"));
    }
}

#[rstest]
fn test_directory_size_output() {
    let tmpdir = tempdir().unwrap();
    let path = PathBuf::from(tmpdir.path());
    // Create a directory with some files
    let test_dir = path.join("test_dir");
    fs::create_dir(&test_dir).unwrap();

    // Create a few files with known sizes
    fs::write(test_dir.join("file1"), vec![0; 1024]).unwrap(); // 1 KiB
    fs::write(test_dir.join("file2"), vec![0; 2048]).unwrap(); // 2 KiB

    let mut output = Vec::new();
    // Test the directory size calculation and output
    let result = rip2::testing::testable_should_we_bury_this(
        &PathBuf::from("test_dir"),
        &test_dir,
        &fs::metadata(&test_dir).unwrap(),
        &mut output,
    );

    assert!(result.is_ok());
    let output_str = String::from_utf8(output).unwrap();

    // Should actually show the files in the directory
    assert!(output_str.contains("test_dir"));
    assert!(output_str.contains("file1"));
    assert!(output_str.contains("file2"));

    let re = regex::Regex::new(r"directory, ([\d.]+ KiB)").unwrap();
    let size = re.captures(&output_str).unwrap().get(1).unwrap().as_str();

    // The total size should be at least 3 KiB (can be larger due to filesystem overhead)
    assert!(size.contains("KiB"));
    let numeric_size = size
        .split_whitespace()
        .next()
        .unwrap()
        .parse::<f64>()
        .unwrap();
    assert!(numeric_size >= 3.0);
    assert!(numeric_size < 6.0);
}

// ============================================================================
// Mode Detection Tests
// ============================================================================

use rip2::{detect_mode_from_inputs, ExecutionMode};
use std::ffi::OsStr;

#[rstest]
fn test_mode_detection_default() {
    // No env, binary name is "rip" → default to Rip mode
    assert_eq!(
        detect_mode_from_inputs(None, OsStr::new("rip")),
        ExecutionMode::Rip
    );
}

#[rstest]
fn test_mode_detection_binary_name_rm() {
    // No env, binary name is "rm" → Rm mode
    assert_eq!(
        detect_mode_from_inputs(None, OsStr::new("rm")),
        ExecutionMode::Rm
    );
}

#[rstest]
fn test_mode_detection_binary_name_safe_rm_does_not_match() {
    // "safe-rm" should NOT trigger Rm mode (not exactly "rm")
    assert_eq!(
        detect_mode_from_inputs(None, OsStr::new("safe-rm")),
        ExecutionMode::Rip
    );
}

#[rstest]
fn test_mode_detection_env_rm() {
    // RIP_MODE=rm overrides binary name
    assert_eq!(
        detect_mode_from_inputs(Some("rm"), OsStr::new("rip")),
        ExecutionMode::Rm
    );
}

#[rstest]
fn test_mode_detection_env_rip() {
    // RIP_MODE=rip explicitly sets Rip mode
    assert_eq!(
        detect_mode_from_inputs(Some("rip"), OsStr::new("rm")),
        ExecutionMode::Rip
    );
}

#[rstest]
fn test_mode_detection_env_case_insensitive() {
    // RIP_MODE should be case-insensitive
    assert_eq!(
        detect_mode_from_inputs(Some("RM"), OsStr::new("rip")),
        ExecutionMode::Rm
    );
    assert_eq!(
        detect_mode_from_inputs(Some("Rm"), OsStr::new("rip")),
        ExecutionMode::Rm
    );
    assert_eq!(
        detect_mode_from_inputs(Some("RIP"), OsStr::new("rm")),
        ExecutionMode::Rip
    );
}

#[rstest]
fn test_mode_detection_invalid_env_falls_through() {
    // Invalid RIP_MODE value falls through to binary name detection
    assert_eq!(
        detect_mode_from_inputs(Some("invalid"), OsStr::new("rm")),
        ExecutionMode::Rm
    );
    assert_eq!(
        detect_mode_from_inputs(Some(""), OsStr::new("rip")),
        ExecutionMode::Rip
    );
}

#[rstest]
fn test_mode_detection_empty_binary_name() {
    // Empty binary name defaults to Rip mode
    assert_eq!(
        detect_mode_from_inputs(None, OsStr::new("")),
        ExecutionMode::Rip
    );
}

// ============================================================================
// rm mode tests
// ============================================================================

/// Helper to create RmArgs with defaults
fn rm_args(targets: Vec<PathBuf>) -> RmArgs {
    RmArgs {
        targets,
        force: false,
        recursive: false,
        dir: false,
        verbose: false,
        no_preserve_root: false,
        preserve_root: false,
        one_file_system: false,
    }
}

#[rstest]
fn test_rm_nonexistent_file_without_force() {
    let _guard = aquire_lock();
    let tempdir = tempdir().unwrap();

    // Try to remove nonexistent file without -f
    let cli = rm_args(vec![tempdir.path().join("nonexistent")]);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mode = TestMode;

    let result = run_rm(&cli, mode, &mut stdout, &mut stderr, &MockRootChecker::non_root());

    // Should fail
    assert!(!result.success);

    // Should have error message in rm format
    let stderr_str = String::from_utf8(stderr).unwrap();
    assert!(stderr_str.contains("rm: cannot remove"));
    assert!(stderr_str.contains("No such file or directory"));
}

#[rstest]
fn test_rm_nonexistent_file_with_force() {
    let _guard = aquire_lock();
    let tempdir = tempdir().unwrap();

    // Try to remove nonexistent file with -f → should succeed silently
    let mut cli = rm_args(vec![tempdir.path().join("nonexistent")]);
    cli.force = true;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mode = TestMode;

    let result = run_rm(&cli, mode, &mut stdout, &mut stderr, &MockRootChecker::non_root());

    // Should succeed (force mode silences missing files)
    assert!(result.success);

    // Should have no error output
    let stderr_str = String::from_utf8(stderr).unwrap();
    assert!(stderr_str.is_empty());
}

#[rstest]
fn test_rm_directory_without_recursive() {
    let _guard = aquire_lock();
    let tempdir = tempdir().unwrap();
    let dir_path = tempdir.path().join("testdir");
    fs::create_dir(&dir_path).unwrap();

    // Try to remove directory without -r → should fail
    let cli = rm_args(vec![dir_path.clone()]);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mode = TestMode;

    let result = run_rm(&cli, mode, &mut stdout, &mut stderr, &MockRootChecker::non_root());

    // Should fail
    assert!(!result.success);

    // Should have "Is a directory" error
    let stderr_str = String::from_utf8(stderr).unwrap();
    assert!(stderr_str.contains("Is a directory"));

    // Directory should still exist
    assert!(dir_path.exists());
}

#[rstest]
fn test_rm_directory_with_recursive() {
    let _guard = aquire_lock();
    let tempdir = tempdir().unwrap();
    let dir_path = tempdir.path().join("testdir");
    fs::create_dir(&dir_path).unwrap();
    fs::write(dir_path.join("file.txt"), "content").unwrap();

    // Remove directory with -r → should succeed
    let mut cli = rm_args(vec![dir_path.clone()]);
    cli.recursive = true;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mode = TestMode;

    let result = run_rm(&cli, mode, &mut stdout, &mut stderr, &MockRootChecker::non_root());

    // Should succeed
    assert!(result.success);

    // Directory should be gone (moved to graveyard)
    assert!(!dir_path.exists());
}

#[rstest]
fn test_rm_empty_directory_with_d_flag() {
    let _guard = aquire_lock();
    let tempdir = tempdir().unwrap();
    let dir_path = tempdir.path().join("emptydir");
    fs::create_dir(&dir_path).unwrap();

    // Remove empty directory with -d → should succeed
    let mut cli = rm_args(vec![dir_path.clone()]);
    cli.dir = true;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mode = TestMode;

    let result = run_rm(&cli, mode, &mut stdout, &mut stderr, &MockRootChecker::non_root());

    // Should succeed
    assert!(result.success);

    // Directory should be gone
    assert!(!dir_path.exists());
}

#[rstest]
fn test_rm_nonempty_directory_with_d_flag() {
    let _guard = aquire_lock();
    let tempdir = tempdir().unwrap();
    let dir_path = tempdir.path().join("nonemptydir");
    fs::create_dir(&dir_path).unwrap();
    fs::write(dir_path.join("file.txt"), "content").unwrap();

    // Remove non-empty directory with -d → should fail
    let mut cli = rm_args(vec![dir_path.clone()]);
    cli.dir = true;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mode = TestMode;

    let result = run_rm(&cli, mode, &mut stdout, &mut stderr, &MockRootChecker::non_root());

    // Should fail
    assert!(!result.success);

    // Should have "Directory not empty" error
    let stderr_str = String::from_utf8(stderr).unwrap();
    assert!(stderr_str.contains("Directory not empty"));

    // Directory should still exist
    assert!(dir_path.exists());
}

#[rstest]
fn test_rm_verbose_output() {
    let _guard = aquire_lock();
    let tempdir = tempdir().unwrap();
    let file_path = tempdir.path().join("testfile.txt");
    fs::write(&file_path, "content").unwrap();

    // Remove with -v → should print "removed 'filename'"
    let mut cli = rm_args(vec![file_path.clone()]);
    cli.verbose = true;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mode = TestMode;

    let result = run_rm(&cli, mode, &mut stdout, &mut stderr, &MockRootChecker::non_root());

    // Should succeed
    assert!(result.success);

    // Should have verbose output
    let stdout_str = String::from_utf8(stdout).unwrap();
    assert!(stdout_str.contains("removed '"));

    // File should be gone
    assert!(!file_path.exists());
}

#[rstest]
fn test_rm_multiple_files_partial_failure() {
    let _guard = aquire_lock();
    let tempdir = tempdir().unwrap();
    let file_path = tempdir.path().join("exists.txt");
    fs::write(&file_path, "content").unwrap();

    // Remove existing file + nonexistent file → should process both, report error
    let cli = rm_args(vec![
        file_path.clone(),
        tempdir.path().join("nonexistent"),
    ]);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mode = TestMode;

    let result = run_rm(&cli, mode, &mut stdout, &mut stderr, &MockRootChecker::non_root());

    // Should fail (because one file didn't exist)
    assert!(!result.success);

    // But the existing file should still be removed
    assert!(!file_path.exists());

    // Error should mention the nonexistent file
    let stderr_str = String::from_utf8(stderr).unwrap();
    assert!(stderr_str.contains("nonexistent"));
}

#[rstest]
fn test_rm_preserve_root_default() {
    let _guard = aquire_lock();

    // Attempt to remove "/" with default preserve-root → should fail
    let cli = rm_args(vec![PathBuf::from("/")]);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mode = TestMode;

    let result = run_rm(&cli, mode, &mut stdout, &mut stderr, &MockRootChecker::non_root());

    // Should fail
    assert!(!result.success);

    // Should have preserve-root error message
    let stderr_str = String::from_utf8(stderr).unwrap();
    assert!(stderr_str.contains("dangerous to operate recursively on '/'"));
    assert!(stderr_str.contains("--no-preserve-root"));
}

#[rstest]
fn test_rm_no_preserve_root_requires_root() {
    let _guard = aquire_lock();

    // Attempt to remove "/" with --no-preserve-root as non-root → should fail
    let mut cli = rm_args(vec![PathBuf::from("/")]);
    cli.no_preserve_root = true;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mode = TestMode;

    let result = run_rm(&cli, mode, &mut stdout, &mut stderr, &MockRootChecker::non_root());

    // Should fail (non-root can't use --no-preserve-root)
    assert!(!result.success);

    // Should have permission error
    let stderr_str = String::from_utf8(stderr).unwrap();
    assert!(stderr_str.contains("requires root"));
}

#[rstest]
fn test_rm_home_depth_protection() {
    let _guard = aquire_lock();

    // Get home directory and create a test directory at depth 1
    let home = dirs::home_dir().expect("Should have home dir");
    let shallow_path = home.join("_test_protected_dir_safe_rm");

    // Create the directory so it can be canonicalized
    fs::create_dir_all(&shallow_path).unwrap();

    // Attempt to remove with -r → should fail (depth 1 = protected)
    let mut cli = rm_args(vec![shallow_path.clone()]);
    cli.recursive = true;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mode = TestMode;

    let result = run_rm(&cli, mode, &mut stdout, &mut stderr, &MockRootChecker::non_root());

    // Clean up the test directory (we created it, protection should have blocked removal)
    fs::remove_dir(&shallow_path).ok();

    // Should fail due to home depth protection
    assert!(!result.success);

    // Should have protection error message
    let stderr_str = String::from_utf8(stderr).unwrap();
    assert!(stderr_str.contains("protected home directory location"));
}

#[rstest]
fn test_rm_home_deep_path_allowed() {
    let _guard = aquire_lock();
    let tempdir = tempdir().unwrap();

    // Create a directory outside of home (tempdir is in /tmp)
    let deep_path = tempdir.path().join("deep_test");
    fs::create_dir(&deep_path).unwrap();

    // Paths not under home should be allowed
    let mut cli = rm_args(vec![deep_path.clone()]);
    cli.recursive = true;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mode = TestMode;

    let result = run_rm(&cli, mode, &mut stdout, &mut stderr, &MockRootChecker::non_root());

    // Should succeed (path is not under home)
    assert!(result.success);
    assert!(!deep_path.exists());
}

// ============================================================================
// Edge Case Tests: Symlinks and Path Traversal
// ============================================================================

/// Test that symlinks pointing to / are caught by preserve-root protection.
/// Attack vector: Create symlink pointing to /, try to rm -r the symlink.
#[rstest]
#[cfg(unix)]
fn test_symlink_to_root_blocked() {
    let _guard = aquire_lock();
    let tempdir = tempdir().unwrap();

    // Create a symlink that points to /
    let symlink_path = tempdir.path().join("root_link");
    std::os::unix::fs::symlink("/", &symlink_path).unwrap();

    // Attempt to rm -r the symlink → should be blocked by preserve-root
    let mut cli = rm_args(vec![symlink_path.clone()]);
    cli.recursive = true;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mode = TestMode;

    let result = run_rm(&cli, mode, &mut stdout, &mut stderr, &MockRootChecker::non_root());

    // Should fail due to preserve-root
    assert!(!result.success);
    let stderr_str = String::from_utf8(stderr).unwrap();
    assert!(
        stderr_str.contains("dangerous to operate recursively on '/'"),
        "Should mention dangerous to operate on /: {}",
        stderr_str
    );

    // Symlink should still exist (not deleted)
    assert!(symlink_path.exists());
}

/// Test that path traversal attempts (/../) are caught after canonicalization.
/// Attack vector: Use paths like /tmp/foo/../../ to try to reach /.
#[rstest]
#[cfg(unix)]
fn test_path_traversal_to_root_blocked() {
    let _guard = aquire_lock();
    let tempdir = tempdir().unwrap();

    // Create a path that traverses up to root: /tmp/xxx/../../
    // After canonicalization this becomes /
    let traversal_path = tempdir.path().join("..").join("..").join("..");

    // Attempt to rm -r → should be blocked by preserve-root
    let mut cli = rm_args(vec![traversal_path.clone()]);
    cli.recursive = true;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mode = TestMode;

    let result = run_rm(&cli, mode, &mut stdout, &mut stderr, &MockRootChecker::non_root());

    // Should fail due to preserve-root
    assert!(!result.success);
    let stderr_str = String::from_utf8(stderr).unwrap();
    assert!(
        stderr_str.contains("dangerous to operate recursively on '/'"),
        "Path traversal to / should be blocked: {}",
        stderr_str
    );
}

/// Test that symlinks to protected home directories are caught.
/// Attack vector: Create symlink to ~/Desktop, try to rm -r the symlink.
#[rstest]
#[cfg(unix)]
fn test_symlink_to_home_child_blocked() {
    let _guard = aquire_lock();
    let tempdir = tempdir().unwrap();

    // Get home directory
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return, // Skip if no home dir
    };

    // Create a real directory under home that we'll protect
    let protected_dir = home.join("_test_protected_safe_rm");
    fs::create_dir_all(&protected_dir).unwrap();

    // Create a symlink in tempdir pointing to this protected directory
    let symlink_path = tempdir.path().join("home_link");
    std::os::unix::fs::symlink(&protected_dir, &symlink_path).unwrap();

    // Attempt to rm -r the symlink → should be blocked by home depth protection
    let mut cli = rm_args(vec![symlink_path.clone()]);
    cli.recursive = true;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mode = TestMode;

    let result = run_rm(&cli, mode, &mut stdout, &mut stderr, &MockRootChecker::non_root());

    let stderr_str = String::from_utf8(stderr).unwrap();

    // Should fail due to home depth protection
    assert!(
        !result.success,
        "Expected failure but got success. stderr: {}",
        stderr_str
    );
    assert!(
        stderr_str.contains("protected home directory location"),
        "Symlink to home child should be blocked: {}",
        stderr_str
    );

    // Clean up the protected directory (can do this before assertion now)
    fs::remove_dir(&protected_dir).ok();

    // Symlink should still exist (not deleted)
    // Note: Use symlink_metadata(), not exists(), because exists() follows
    // the symlink and checks if the TARGET exists. We want to check if
    // the symlink itself exists, regardless of its target.
    assert!(
        fs::symlink_metadata(&symlink_path).is_ok(),
        "Symlink should not have been deleted"
    );
}
