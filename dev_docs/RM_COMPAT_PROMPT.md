# Task: Add rm-Compatibility Mode to rip2

## What is rip2?

rip2 is a Rust-based safe alternative to the `rm` command. Instead of permanently deleting files, it moves them to a "graveyard" directory where they can be recovered later. Think of it as a command-line recycle bin.

**Repository:** https://github.com/MilesCranmer/rip2

**Current rip2 commands:**
```bash
rip file.txt          # Move file to graveyard
rip -s                # List what was deleted (seance)
rip -u                # Restore last deleted file (unbury)
rip -d                # Permanently empty the graveyard (decompose)
rip -i file.txt       # Inspect file before deleting
```

## Your Task

Add a **GNU rm-compatible mode** to rip2 so it can be used as a drop-in replacement for `rm`.

When someone creates a symlink like `ln -s /usr/bin/rip /usr/local/bin/rm`, running `rm file.txt` should behave exactly like GNU rm - same flags, same output, same exit codes - but still use the graveyard under the hood.

## Key Requirements

### 1. Mode Detection

The binary should detect how it was invoked:
- Invoked as `rm` → rm-compatible mode
- Invoked as `rip` → normal rip mode (unchanged)

Also support `RIP_MODE=rm` environment variable for testing.

### 2. Full GNU rm Flag Compatibility

Support all standard GNU rm flags:
- `-f, --force` - never prompt, ignore missing files
- `-i` - prompt before each removal
- `-I` - prompt once for bulk operations (>3 files or recursive)
- `-r, -R, --recursive` - remove directories
- `-d, --dir` - remove empty directories
- `-v, --verbose` - show what's being removed
- `--interactive=WHEN` - control prompting (never/once/always)
- `--one-file-system` - don't cross filesystem boundaries
- `--preserve-root` / `--no-preserve-root` - root protection

**Important:** Some flags conflict with current rip flags:
- rip's `-i` means "inspect", but rm's `-i` means "interactive"
- rip's `-d` means "decompose graveyard", but rm's `-d` means "remove empty dirs"

These must work correctly based on which mode is active.

### 3. Behavioral Compatibility

Match GNU rm behavior exactly:
- Error messages should use rm's format: `rm: cannot remove 'file': Reason`
- Exit codes: 0 for success, 1 for any error
- `-f` with no arguments should exit 0 (not an error)
- User declining an `-I` prompt should exit 0 (user choice, not error)

### 4. Safety Features

**Standard (GNU rm compatible):**
- `--preserve-root` enabled by default - refuse to `rm -r /`

**Enhanced (rip2 addition):**
Add protection against accidentally deleting shallow home directory paths. Block recursive removal of paths like:
- `/home/username`
- `/home/username/Desktop`
- `~/Documents`

Only allow recursive removal when the path is deeper, like:
- `/home/username/Desktop/project`
- `~/Documents/old-files`

Require a special flag `--yes-i-am-100-percent-certain` to override this protection. This flag should be hidden from help output.

### 5. Backward Compatibility

**Critical:** Normal rip mode must remain completely unchanged. Users who invoke the binary as `rip` should see no difference in behavior.

## What NOT to Change

- The graveyard mechanism stays the same
- All deletions still go to graveyard (recoverable)
- Existing rip flags work exactly as before in rip mode
- No need to implement xdg-trash or desktop integration

## Technical Notes

- The project is written in Rust
- Uses `clap` for argument parsing
- Target compatibility: GNU coreutils rm version 9.4
- Must work on Linux, macOS, and Windows

## Deliverables

1. Mode detection that switches behavior based on binary name
2. Complete rm-compatible argument parsing
3. All rm behaviors implemented correctly
4. Home directory depth protection feature
5. Tests comparing behavior against real GNU rm
6. Updated documentation

## Reference

For the formal requirements specification, see `docs/RM_COMPAT_SRS.md` in the repository.

For GNU rm documentation: https://www.gnu.org/software/coreutils/rm

---

**Good luck! The goal is 100% behavioral compatibility with GNU rm, while secretly keeping everything recoverable via the graveyard.**
