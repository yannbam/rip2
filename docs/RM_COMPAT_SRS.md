# Software Requirements Specification: rm-Compatibility Layer for rip2

**Document Version:** 1.0
**Date:** 2025-01-22
**Project:** rip2 - https://github.com/MilesCranmer/rip2
**Target Version:** 0.10.0

---

## 1. Introduction

### 1.1 Purpose

This document specifies requirements for adding a GNU rm-compatible interface to rip2. The implementation shall allow rip2 to be used as a drop-in replacement for the GNU `rm` command while preserving rip2's graveyard-based deletion mechanism.

### 1.2 Scope

This specification covers:
- Mode detection mechanism
- Command-line argument compatibility
- Behavioral requirements
- Safety features (including rip2-specific enhancements)
- Error handling
- Exit codes

### 1.3 References

- GNU coreutils rm: https://www.gnu.org/software/coreutils/rm
- Target compatibility: GNU rm version 9.4
- rip2 repository: https://github.com/MilesCranmer/rip2

### 1.4 Definitions

- **rip mode**: Normal rip2 operation with graveyard features
- **rm-compat mode**: GNU rm-compatible operation
- **graveyard**: Directory where "deleted" files are moved for potential recovery

---

## 2. System Overview

### 2.1 Current System

rip2 is a Rust-based safe alternative to rm that moves files to a graveyard instead of permanently deleting them. Current rip2 flags include:
- `-i, --inspect`: Show file info before deletion
- `-f, --force`: Skip prompts for large files
- `-d, --decompose`: Permanently delete graveyard
- `-s, --seance`: List deleted files
- `-u, --unbury`: Restore deleted files
- `--graveyard`: Specify graveyard location

### 2.2 Proposed Enhancement

Add an rm-compatible interface that:
1. Accepts GNU rm command-line arguments
2. Maintains 100% behavioral compatibility with GNU rm
3. Uses rip2's graveyard mechanism (files are recoverable)
4. Adds enhanced safety features beyond GNU rm

---

## 3. Functional Requirements

### 3.1 Mode Detection

**REQ-MODE-001**: The system SHALL detect operating mode based on the invoked binary name.
- If invoked as `rm` (or `rm.exe` on Windows): activate rm-compat mode
- Otherwise: activate rip mode

**REQ-MODE-002**: The system SHALL support an environment variable `RIP_MODE` to override mode detection.
- `RIP_MODE=rm`: Force rm-compat mode
- `RIP_MODE=rip`: Force rip mode

**REQ-MODE-003**: Environment variable SHALL take precedence over binary name detection.

### 3.2 Command-Line Arguments (rm-compat mode)

**REQ-ARG-001**: The system SHALL accept the following short flags:
- `-f`: Force mode (never prompt, ignore missing files)
- `-i`: Interactive mode (prompt before each removal)
- `-I`: Interactive once (prompt before bulk/recursive operations)
- `-r`: Recursive removal
- `-R`: Recursive removal (alias for -r)
- `-d`: Remove empty directories
- `-v`: Verbose output

**REQ-ARG-002**: The system SHALL accept the following long flags:
- `--force`: Same as -f
- `--interactive[=WHEN]`: Control prompting (WHEN: never, once, always)
- `--recursive`: Same as -r
- `--dir`: Same as -d
- `--verbose`: Same as -v
- `--one-file-system`: Do not cross filesystem boundaries
- `--preserve-root`: Protect root filesystem (default: enabled)
- `--preserve-root=all`: Protect all mount points
- `--no-preserve-root`: Disable root protection
- `--help`: Display help
- `--version`: Display version

**REQ-ARG-003**: The system SHALL accept positional arguments as files/directories to remove.

**REQ-ARG-004**: Flag `-i` SHALL conflict with rip mode's `-i` (inspect). In rm-compat mode, `-i` means interactive prompting.

**REQ-ARG-005**: Flag `-d` SHALL conflict with rip mode's `-d` (decompose). In rm-compat mode, `-d` means remove empty directories.

**REQ-ARG-006**: Rip-specific flags (`--seance`, `--unbury`, `--decompose`) SHALL NOT be available in rm-compat mode.

**REQ-ARG-007**: The `--graveyard` flag SHALL remain available in rm-compat mode for testing purposes.

### 3.3 Interactive Modes

**REQ-INT-001**: Interactive mode NEVER (`-f` or `--interactive=never`):
- Never prompt the user
- Ignore nonexistent files without error

**REQ-INT-002**: Interactive mode ALWAYS (`-i` or `--interactive=always`):
- Prompt before each file/directory removal
- If user declines: skip that file, continue with others

**REQ-INT-003**: Interactive mode ONCE (`-I` or `--interactive=once`):
- Prompt once before starting, IF:
  - More than 3 files are being removed, OR
  - Recursive mode is active
- If user declines: exit immediately with success status
- If conditions not met: proceed without prompting

**REQ-INT-004**: `--interactive` without argument SHALL behave as `--interactive=always`.

**REQ-INT-005**: Later flags SHALL override earlier flags (e.g., `-i -f` results in force mode).

### 3.4 Directory Handling

**REQ-DIR-001**: Attempting to remove a directory without `-r` or `-d` SHALL fail with error.

**REQ-DIR-002**: With `-r` or `-R`: remove directories and all contents recursively.

**REQ-DIR-003**: With `-d`: remove only empty directories. Fail if directory is not empty.

**REQ-DIR-004**: Non-directory files SHALL be removable without any flags.

### 3.5 Verbose Output

**REQ-VERB-001**: With `-v` or `--verbose`, print each removed item.

**REQ-VERB-002**: Output format: `removed 'FILENAME'` for files.

**REQ-VERB-003**: Output format: `removed directory 'DIRNAME'` for directories.

### 3.6 One-File-System

**REQ-OFS-001**: With `--one-file-system` during recursive removal:
- Skip any directory that resides on a different filesystem than the starting directory
- Continue with other files/directories

### 3.7 Deletion Mechanism

**REQ-DEL-001**: All deletions SHALL use rip2's graveyard mechanism.
- Files are moved to graveyard, not permanently deleted
- Files remain recoverable via rip's unbury functionality

**REQ-DEL-002**: The graveyard location SHALL follow rip2's existing logic:
- `$RIP_GRAVEYARD` if set
- `$XDG_DATA_HOME/graveyard` if XDG_DATA_HOME is set
- `$TMPDIR/graveyard-$USER` otherwise

---

## 4. Safety Requirements

### 4.1 Root Protection (GNU rm Compatible)

**REQ-SAFE-001**: By default, `--preserve-root` SHALL be enabled.

**REQ-SAFE-002**: With `--preserve-root` (default):
- Refuse to recursively remove the root filesystem (`/` on Unix, drive roots on Windows)
- Error message: `refusing to remove '/' recursively`

**REQ-SAFE-003**: With `--preserve-root=all`:
- Additionally refuse to remove any path that is a mount point on a separate device

**REQ-SAFE-004**: With `--no-preserve-root`:
- Disable root protection
- This flag MUST be spelled exactly; abbreviations SHALL NOT be accepted

### 4.2 Home Directory Depth Protection (rip2 Enhancement)

**REQ-SAFE-010**: The system SHALL implement depth protection for home directories.

**REQ-SAFE-011**: For paths under `/home/` (Unix) or `C:\Users\` (Windows):
- Block recursive removal if path depth is fewer than 4 components
- Components counted from root: `/home/user/dir1/dir2` = 4 components

**REQ-SAFE-012**: Blocked paths (examples):
- `/home` (2 components)
- `/home/username` (3 components)
- `/home/username/Desktop` (3 components)
- `C:\Users\username` (3 components)
- `C:\Users\username\Documents` (3 components)

**REQ-SAFE-013**: Allowed paths (examples):
- `/home/username/Desktop/project` (4 components)
- `/home/username/Documents/work` (4 components)
- `C:\Users\username\Documents\project` (4 components)

**REQ-SAFE-014**: The system SHALL provide an override flag: `--yes-i-am-100-percent-certain`
- This flag bypasses depth protection
- This flag SHALL be hidden from help output

**REQ-SAFE-015**: Depth protection SHALL be independent of `--preserve-root`.
- `--no-preserve-root` does NOT disable depth protection
- Only `--yes-i-am-100-percent-certain` disables depth protection

**REQ-SAFE-016**: Depth protection error message SHALL include:
- The blocked path
- Indication that depth protection triggered
- Hint about the override flag

**REQ-SAFE-017**: Path resolution SHALL use canonical/absolute paths for depth counting.

---

## 5. Error Handling

### 5.1 Error Message Format

**REQ-ERR-001**: Error messages SHALL follow GNU rm format: `rm: cannot remove 'PATH': REASON`

**REQ-ERR-002**: Common error reasons:
- `No such file or directory` - target does not exist
- `Is a directory` - attempting to remove directory without -r
- `Directory not empty` - using -d on non-empty directory
- `Permission denied` - insufficient permissions
- `refusing to remove '/' recursively` - preserve-root triggered

### 5.2 Missing Files

**REQ-ERR-010**: Without `-f`: missing files SHALL cause an error.

**REQ-ERR-011**: With `-f`: missing files SHALL be silently ignored.

### 5.3 Partial Failures

**REQ-ERR-020**: If some files fail and others succeed:
- Continue processing remaining files
- Report each failure
- Exit with failure status

---

## 6. Exit Codes

**REQ-EXIT-001**: Exit code 0 (success):
- All specified files were successfully removed
- User declined `-I` prompt (no error, just user choice)
- `-f` with no arguments (nothing to do, not an error)

**REQ-EXIT-002**: Exit code 1 (failure):
- One or more files could not be removed
- Invalid arguments provided
- No arguments provided without `-f`

---

## 7. Backward Compatibility

### 7.1 Rip Mode Preservation

**REQ-COMPAT-001**: When invoked as `rip`, all existing functionality SHALL remain unchanged.

**REQ-COMPAT-002**: Existing rip flags SHALL retain their current meanings in rip mode:
- `-i` = inspect (NOT interactive)
- `-d` = decompose (NOT empty directories)
- `-s` = seance
- `-u` = unbury
- `-f` = force

**REQ-COMPAT-003**: No breaking changes to rip mode behavior.

### 7.2 Recovery

**REQ-COMPAT-010**: Files deleted via rm-compat mode SHALL be recoverable.

**REQ-COMPAT-011**: Recovery SHALL be possible via:
- Invoking rip directly: `rip -u`
- Using full path to rip binary

---

## 8. Platform Requirements

### 8.1 Unix/Linux

**REQ-PLAT-001**: Full functionality on Linux and macOS.

**REQ-PLAT-002**: Home directory protection for `/home/` paths.

### 8.2 Windows

**REQ-PLAT-010**: Core functionality on Windows.

**REQ-PLAT-011**: Home directory protection for `C:\Users\` paths.

**REQ-PLAT-012**: Binary name detection SHALL handle `rm.exe`.

---

## 9. Testing Requirements

### 9.1 Compatibility Testing

**REQ-TEST-001**: Behavior SHALL match GNU rm 9.4 for all documented flags.

**REQ-TEST-002**: Comparison testing against real GNU rm installation.

### 9.2 Safety Testing

**REQ-TEST-010**: Root protection SHALL be verified.

**REQ-TEST-011**: Home directory depth protection SHALL be verified at all boundary conditions.

**REQ-TEST-012**: Override flags SHALL be verified to work correctly.

### 9.3 Backward Compatibility Testing

**REQ-TEST-020**: All existing rip tests SHALL continue to pass.

**REQ-TEST-021**: Rip mode behavior SHALL be verified unchanged.

---

## 10. Non-Requirements

The following are explicitly NOT in scope:

- Permanent deletion mode (all deletions use graveyard)
- XDG trash specification compliance
- Desktop integration (trash icons, etc.)
- Network filesystem special handling
- ACL or extended attribute preservation
- Shred/secure deletion

---

## 11. Acceptance Criteria

The implementation SHALL be considered complete when:

1. All REQ-* requirements are satisfied
2. GNU rm flag compatibility is verified
3. All safety features are tested and working
4. Backward compatibility is maintained
5. Documentation is updated

---

## Appendix A: Flag Quick Reference

### rm-compat Mode Flags

| Flag | Long Form | Description |
|------|-----------|-------------|
| `-f` | `--force` | Never prompt, ignore missing |
| `-i` | — | Prompt before each removal |
| `-I` | — | Prompt once for bulk operations |
| — | `--interactive[=WHEN]` | Control prompting |
| `-r` | `--recursive` | Remove directories recursively |
| `-R` | — | Alias for -r |
| `-d` | `--dir` | Remove empty directories |
| `-v` | `--verbose` | Print each removal |
| — | `--one-file-system` | Don't cross filesystems |
| — | `--preserve-root` | Protect / (default) |
| — | `--preserve-root=all` | Protect all mounts |
| — | `--no-preserve-root` | Allow removing / |
| — | `--yes-i-am-100-percent-certain` | Override depth protection (hidden) |
| — | `--graveyard` | Set graveyard path (hidden) |

### Interactive Mode Values

| Value | Aliases | Behavior |
|-------|---------|----------|
| `never` | `no`, `none` | Never prompt |
| `once` | — | Prompt for bulk/recursive |
| `always` | `yes` | Prompt for each file |

---

## Appendix B: Depth Protection Examples

### Unix

| Path | Components | Allowed? |
|------|------------|----------|
| `/home` | 2 | ❌ No |
| `/home/jan` | 3 | ❌ No |
| `/home/jan/Desktop` | 3 | ❌ No |
| `/home/jan/Desktop/foo` | 4 | ✅ Yes |
| `/home/jan/src/project/build` | 5 | ✅ Yes |

### Windows

| Path | Components | Allowed? |
|------|------------|----------|
| `C:\Users` | 2 | ❌ No |
| `C:\Users\jan` | 3 | ❌ No |
| `C:\Users\jan\Desktop` | 3 | ❌ No |
| `C:\Users\jan\Desktop\foo` | 4 | ✅ Yes |

---

**End of Document**
