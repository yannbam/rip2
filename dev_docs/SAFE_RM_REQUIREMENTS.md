# safe-rm: Making File Deletion Safe by Design

## Vision

Replace the standard Unix `rm` command with safe-rm, making it **structurally impossible** for non-root users to permanently delete files via `rm`. Every deletion becomes a move to the graveyard. Recovery is always possible. Only root holds the power of true deletion.

This is not about "rm compatibility for those who want it" - it's about **making the rm command safe by design**.

## What is safe-rm?

safe-rm is a fork of rip2, a Rust-based safe alternative to `rm`. Instead of permanently deleting files, it moves them to a "graveyard" directory (`~/.graveyard` by default) where they can be recovered.

**Core principle:** For non-root users, permanent deletion is not a code path that exists.

## Current rip2 Commands (Reference)

```bash
rip file.txt          # Move file to graveyard
rip -s                # List deleted files from current directory (seance)
rip -u                # Restore last deleted file (unbury)
rip -d                # Permanently empty the graveyard (decompose) - WILL BE ROOT-ONLY
rip -i file.txt       # Inspect file before deleting
rip -f                # Force mode (no prompts)
```

---

## Requirements

### 1. Root-Only Permanent Deletion

**This is the core safety feature.**

The following operations require root (euid == 0):

| Operation | Non-root behavior | Root behavior |
|-----------|-------------------|---------------|
| `rip -d` / `--decompose` | Error: "Only root can permanently delete files" | Empties graveyard |
| Delete file already in graveyard | Error: "Only root can permanently delete files" | Permanently deletes |

Implementation:
```rust
fn can_permanently_delete() -> bool {
    unsafe { libc::geteuid() == 0 }
}
```

When a non-root user attempts permanent deletion, safe-rm must:
- Print clear error message: `safe-rm: Only root can permanently delete files`
- Exit with non-zero status
- **Never** silently succeed or pretend the operation worked

### 2. Persistent Graveyard Location

**Default:** `~/.graveyard`

The graveyard must persist across reboots. The current default (`/tmp/graveyard-$USER`) is unsuitable because `/tmp` is cleared on reboot.

**Priority order for graveyard location:**
1. `--graveyard <PATH>` CLI flag
2. `$RIP_GRAVEYARD` environment variable
3. `$XDG_DATA_HOME/graveyard` (if `$XDG_DATA_HOME` is set)
4. `~/.graveyard` (new default)

### 3. Mode Detection

The binary detects how it was invoked:

| Invocation | Mode | Behavior |
|------------|------|----------|
| `rip` | rip mode | Original rip2 interface |
| `rm` | rm mode | rm-compatible interface |
| `RIP_MODE=rm rip` | rm mode | For testing |

When invoked as `rm`, use rm-compatible flags and output format.

### 4. rm-Compatible Flags (Simplified)

**No interactive features required.** This simplifies implementation significantly.

**Supported flags:**

| Flag | Description |
|------|-------------|
| `-f, --force` | Never prompt, ignore nonexistent files |
| `-r, -R, --recursive` | Remove directories and their contents |
| `-d, --dir` | Remove empty directories |
| `-v, --verbose` | Explain what is being done |
| `--preserve-root` | Do not remove `/` (default: enabled) |
| `--no-preserve-root` | Do not treat `/` specially |
| `--one-file-system` | Stay within single filesystem when recursive |
| `-h, --help` | Display help |
| `-V, --version` | Display version |

**NOT implemented (interactive features):**
- `-i` (prompt before every removal)
- `-I` (prompt once before removing >3 files or recursively)
- `--interactive=WHEN`

### 5. Flag Conflicts Between Modes

Some flags have different meanings in rip mode vs rm mode:

| Flag | rip mode | rm mode |
|------|----------|---------|
| `-i` | Inspect file before deleting | *Not supported* (would be interactive) |
| `-d` | Decompose (empty graveyard) | Remove empty directories |

The binary must use the correct interpretation based on active mode.

### 6. rm-Compatible Behavior

When in rm mode:

**Error message format:**
```
rm: cannot remove 'file': No such file or directory
rm: cannot remove 'dir': Is a directory
```

**Exit codes:**
- `0` - Success
- `1` - Error occurred

**Special behaviors:**
- `rm -f` with no arguments: exit 0 (not an error)
- `rm nonexistent`: exit 1 with error message
- `rm -f nonexistent`: exit 0 silently

### 7. Safety Features

**Root protection (--preserve-root):**
- Enabled by default
- Refuse to `rm -r /`
- Can be disabled with `--no-preserve-root` (for root only)

**Home directory protection:**
Block recursive removal of shallow home paths:
- `/home/username`
- `/home/username/Desktop`
- `~/Documents`

Allow when path is deeper:
- `/home/username/Desktop/project`
- `~/Documents/old-files`

Override with hidden flag: `--yes-i-am-100-percent-certain`

### 8. Backward Compatibility

**Critical:** When invoked as `rip`, behavior must remain unchanged from original rip2. Existing users must not be affected.

### 9. Graveyard Maintenance

Graveyard cleanup is performed manually by root:

```bash
sudo rip -d              # Empty entire graveyard (with prompt)
sudo rip -df             # Empty entire graveyard (no prompt)
sudo rip ~/.graveyard/path/to/file   # Delete specific item from graveyard
```

No automatic cleanup. Root manages disk space as needed.

---

## System Deployment

To make safe-rm the system's `rm`:

1. Install safe-rm binary: `sudo cp safe-rm /usr/local/bin/safe-rm`
2. Move original rm: `sudo mv /usr/bin/rm /usr/sbin/rm-real`
3. Create symlinks:
   ```bash
   sudo ln -s /usr/local/bin/safe-rm /usr/bin/rm       # rm mode
   sudo ln -s /usr/local/bin/safe-rm /usr/local/bin/rip  # rip mode
   ```
4. Ensure `/usr/sbin` is only in root's PATH

Result:
- `rm file` → safe-rm in rm mode (rm-compatible interface)
- `rip file` → safe-rm in rip mode (rip-style interface with -s, -u, etc.)
- `rm-real` → original rm (root only, for emergencies)

---

## What This Does NOT Protect Against

This is about making `rm` safe, not about preventing all file deletion:

- Direct syscalls (`unlink()`, `rmdir()`)
- Other tools (`find -delete`, Python's `os.unlink()`, etc.)
- Moving files to `/tmp` (which gets cleared)
- Overwriting file contents

The goal is protecting against **accidental deletion via the rm command**, which covers the vast majority of "oops" moments.

---

## Technical Notes

- Written in Rust
- Uses `clap` for argument parsing
- Cross-platform: Linux, macOS, Windows
- Root detection via `libc::geteuid()`

## Reference Materials

- `dev_docs/rm-reference/` - GNU coreutils rm source code for reference:
  - `rm.c` - Main rm implementation
  - `remove.c` - File removal logic
  - `remove.h` - Headers and data structures
- GNU rm documentation: https://www.gnu.org/software/coreutils/rm

---

## Deliverables

1. Root-only permanent deletion enforcement
2. New default graveyard location (`~/.graveyard`)
3. Mode detection (rip vs rm based on binary name)
4. rm-compatible argument parsing (non-interactive subset)
5. rm-compatible output format and exit codes
6. Home directory depth protection
7. Comprehensive test suite
8. Updated documentation

---

## Summary

| Feature | Behavior |
|---------|----------|
| Default graveyard | `~/.graveyard` |
| Non-root `rm file` | Moves to graveyard |
| Non-root `rip -d` | Error: "Only root can permanently delete" |
| Root `rip -d` | Empties graveyard |
| Interactive flags | Not supported |
| Invoked as `rm` | rm-compatible interface |
| Invoked as `rip` | Original rip2 interface |

**The common case becomes safe. Accidents become recoverable. Power requires privilege.**
