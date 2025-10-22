# RM Compatibility Mode - Technical Specification

**Version:** 1.0
**Date:** 2025-01-22
**Target:** rip2 v0.9.4+
**Reference:** GNU coreutils rm v9.4

## Executive Summary

This document specifies how to add full GNU `rm` compatibility to `rip2`, allowing it to be used as a drop-in replacement for `rm` via symlink while maintaining backward compatibility with existing `rip` behavior.

---

## 1. Architecture Overview

### 1.1 Mode Detection

The binary will detect which mode to operate in using the following priority:

1. **Binary name detection** (highest priority)
   - If `argv[0]` ends with `/rm` or `\rm` → **rm-compat mode**
   - Otherwise → **rip mode**

2. **Environment variable override** (optional, for testing)
   - `RIP_MODE=rm` → force rm-compat mode
   - `RIP_MODE=rip` → force rip mode

3. **Explicit flag** (backup mechanism)
   - `--rm-compat` → force rm-compat mode (even when invoked as `rip`)

### 1.2 Code Structure

```
src/
├── main.rs           # Modified: detect mode, dispatch to correct parser
├── args.rs           # Modified: add RmArgs struct + mode enum
├── args_rm.rs        # NEW: GNU rm argument parsing
├── lib.rs            # Modified: add rm-compat behavior paths
├── rm_compat.rs      # NEW: rm-specific logic (preserve-root, verbose, etc.)
├── completions.rs    # Modified: support both rip and rm completions
├── util.rs           # Modified: shared utilities
└── record.rs         # Unchanged: graveyard record management
```

---

## 2. Argument Mapping

### 2.1 GNU rm Flags (must support)

| rm Flag | Short | Long | Behavior | rip Equivalent | Notes |
|---------|-------|------|----------|----------------|-------|
| `-f` | ✓ | `--force` | Never prompt, ignore errors | Similar to rip `-f` | Sets `interactive=NEVER`, `ignore_missing=true` |
| `-i` | ✓ | — | Prompt before every removal | ⚠️ **CONFLICT** with rip `-i` (inspect) | Sets `interactive=ALWAYS` |
| `-I` | ✓ | — | Prompt once if >3 files or recursive | N/A in rip | Sets `interactive=SOMETIMES`, `prompt_once=true` |
| — | — | `--interactive[=WHEN]` | never/once/always | N/A in rip | Flexible interactive control |
| `-r`, `-R` | ✓ | `--recursive` | Remove directories recursively | Automatic in rip | Must be explicit in rm |
| `-d` | ✓ | `--dir` | Remove empty directories | N/A in rip | Special case |
| `-v` | ✓ | `--verbose` | Explain what is being done | N/A in rip | Print each file as removed |
| — | — | `--one-file-system` | Don't cross filesystem boundaries | N/A in rip | Advanced feature |
| — | — | `--preserve-root[=all]` | Protect `/` from removal | N/A in rip | **Default: ON** |
| — | — | `--no-preserve-root` | Allow removing `/` | N/A in rip | Explicit override |

### 2.2 Rip-Specific Flags (disabled in rm-compat mode)

| Flag | Behavior | Status in rm-compat mode |
|------|----------|--------------------------|
| `-d, --decompose` | Permanently delete graveyard | ❌ **Disabled** - conflicts with rm `-d` |
| `-s, --seance` | List deleted files | ❌ **Disabled** - no rm equivalent |
| `-u, --unbury` | Restore files | ❌ **Disabled** - no rm equivalent |
| `-i, --inspect` | Show file info before deletion | ❌ **Disabled** - conflicts with rm `-i` |
| `--graveyard` | Custom graveyard path | ✅ **Kept** - useful for testing, no conflict |

### 2.3 Flag Conflict Resolution

**Problem:** `rip -i` means "inspect", but `rm -i` means "interactive prompt"

**Solution:** Mode-dependent parsing
```rust
match detected_mode {
    Mode::Rip => {
        // -i parses as --inspect
        // Graveyard features enabled
    }
    Mode::Rm => {
        // -i parses as --interactive
        // Graveyard features disabled
        // Error if rip-specific flags used
    }
}
```

---

## 3. Behavioral Differences

### 3.1 Interactive Prompting

#### GNU rm Behavior (from rm.c analysis)

**Three modes:**
```c
enum rm_interactive {
    RMI_NEVER = 3,      // -f, --interactive=never: never prompt
    RMI_SOMETIMES,      // -I, --interactive=once: single prompt if >3 files or recursive
    RMI_ALWAYS          // -i, --interactive=always: prompt for each file
}
```

**Prompt-once logic** (lines 351-364 of rm.c):
```c
if (prompt_once && (x.recursive || 3 < n_files)) {
    fprintf(stderr, "%s: remove %ju argument(s) recursively? ",
            program_name, n_files);
    if (!yesno())
        return EXIT_SUCCESS;  // Exit WITHOUT error if user says no
}
```

**Key insights:**
- `RMI_SOMETIMES` + `prompt_once` only triggers if:
  - More than 3 files being removed, OR
  - Recursive mode is active
- This is a **single up-front prompt**, not per-file
- Saying "no" exits with **success** (exit code 0)
- After this prompt, individual files aren't prompted again

#### Current rip Behavior

- `inspect` flag (`-i`): Shows file info, then prompts
- `force` flag (`-f`): Skips prompts for big files
- No concept of `RMI_SOMETIMES` or `prompt_once`

#### Required Changes

Implement three-level interactive mode:
1. **NEVER**: Skip all prompts (like `rip -f`)
2. **SOMETIMES**: Single up-front prompt if >3 files or recursive
3. **ALWAYS**: Prompt before each file/directory removal

### 3.2 Preserve-Root Protection

#### GNU rm Behavior (from rm.c lines 337-346)

```c
if (x.recursive && preserve_root) {
    static struct dev_ino dev_ino_buf;
    x.root_dev_ino = get_root_dev_ino(&dev_ino_buf);
    if (x.root_dev_ino == nullptr)
        error(EXIT_FAILURE, errno, "failed to get attributes of /");
}
```

**Protection logic:**
- **Default:** `preserve_root = true`
- **When active:** Gets device/inode of `/` filesystem root
- **Check happens:** During recursive removal
- **Two variants:**
  - `--preserve-root`: Only protect `/` itself (default)
  - `--preserve-root=all`: Protect ANY mount point on separate device

#### Current rip Behavior

- No special protection for `/`
- Would happily move `/` to graveyard if permissions allowed

#### Required Changes

Add root protection:
1. Detect if target is `/` (or canonicalizes to `/`)
2. In rm-compat mode with recursive + preserve_root:
   - Get root device/inode
   - Check target against root
   - Error out if match: `"refusing to remove '/' recursively"`
3. `--no-preserve-root` disables this check

### 3.3 Verbose Output

#### GNU rm Behavior

`-v, --verbose` prints each file as it's removed:
```
removed 'file1.txt'
removed directory 'dir1'
removed 'dir1/nested/file2.txt'
```

Format: `"removed 'FILENAME'"`

#### Current rip Behavior

Silent unless using `seance` afterward. No per-file output during deletion.

#### Required Changes

Add verbose mode that prints each file/directory as it's moved to graveyard:
```rust
if verbose {
    writeln!(stream, "removed '{}'", target.display())?;
}
```

### 3.4 Recursive Behavior

#### GNU rm Behavior

- **Default:** Does NOT remove directories
- **With `-r` or `-R`:** Removes directories and contents recursively
- **Error without `-r`:** `"cannot remove 'DIR': Is a directory"`

#### Current rip Behavior

- **Automatically** handles directories recursively
- No flag required

#### Required Changes

In rm-compat mode:
1. If target is directory AND not `recursive` flag:
   - Error: `"cannot remove 'DIR': Is a directory"`
   - Exit with failure
2. If target is directory AND recursive flag set:
   - Proceed with removal

### 3.5 Empty Directory Handling

#### GNU rm Behavior

`-d, --dir` allows removing empty directories without `-r`:
```bash
rm -d empty_dir/     # Works
rm -d nonempty_dir/  # Error: "Directory not empty"
```

#### Current rip Behavior

Removes any directory (empty or not) automatically.

#### Required Changes

In rm-compat mode with `-d` flag:
1. Check if directory is empty
2. If empty: remove it
3. If not empty: error `"cannot remove 'DIR': Directory not empty"`

### 3.6 One-File-System

#### GNU rm Behavior

`--one-file-system`: When removing recursively, skip directories on different filesystems.

**Use case:** Prevent accidentally removing mounted filesystems.

```bash
rm -r --one-file-system /home/user/
# Skips /home/user/mnt/external_drive (different filesystem)
```

#### Current rip Behavior

No filesystem boundary checking.

#### Required Changes

Implement filesystem boundary detection:
1. Get device ID of starting directory
2. During recursive traversal, check device ID of each subdirectory
3. If different device ID: skip with warning (or silently, check rm behavior)

---

## 4. Exit Codes

### GNU rm Exit Codes

```c
return status == RM_ERROR ? EXIT_FAILURE : EXIT_SUCCESS;
```

- **EXIT_SUCCESS (0):** All files removed successfully (or user declined with -I)
- **EXIT_FAILURE (1):** At least one error occurred

**Special cases:**
- No arguments with `-f`: exit 0 (success)
- No arguments without `-f`: error, exit 1
- User says "no" to `-I` prompt: exit 0 (success)

### Current rip Exit Codes

```rust
ExitCode::SUCCESS  // Everything worked
ExitCode::FAILURE  // Exception occurred
```

### Required Changes

Match rm exit code behavior exactly:
- Success: all files processed without errors
- Failure: any error during processing
- Special handling for `-f` with no args (exit 0)

---

## 5. Error Handling

### GNU rm Error Messages

**Format:** `"rm: REASON: 'FILENAME'"`

**Examples:**
```
rm: cannot remove 'file.txt': No such file or directory
rm: cannot remove '/': refusing to remove '/' recursively
rm: cannot remove 'dir': Is a directory
```

### Current rip Error Messages

**Format:** Custom rip messages

**Example:**
```
Exception: ...
```

### Required Changes

In rm-compat mode, match rm error message format:
```rust
format!("rm: cannot remove '{}': {}", path.display(), reason)
```

---

## 6. Argument Parsing Implementation

### 6.1 Struct Definitions

```rust
// In args.rs
pub enum OperatingMode {
    Rip,
    RmCompat,
}

// In args_rm.rs
pub struct RmArgs {
    pub targets: Vec<PathBuf>,
    pub force: bool,                    // -f, --force
    pub interactive: InteractiveMode,   // -i, -I, --interactive
    pub recursive: bool,                // -r, -R, --recursive
    pub dir: bool,                      // -d, --dir
    pub verbose: bool,                  // -v, --verbose
    pub one_file_system: bool,          // --one-file-system
    pub preserve_root: PreserveRoot,    // --preserve-root[=all], --no-preserve-root
    pub graveyard: Option<PathBuf>,     // --graveyard (kept for testing)
}

pub enum InteractiveMode {
    Never,      // -f, --interactive=never
    Once,       // -I, --interactive=once
    Always,     // -i, --interactive=always
}

pub enum PreserveRoot {
    None,       // --no-preserve-root
    Root,       // --preserve-root (default)
    All,        // --preserve-root=all
}
```

### 6.2 Clap Argument Definitions

```rust
#[derive(Parser)]
#[command(name = "rm", version, about = "remove files or directories")]
pub struct RmArgs {
    #[arg(value_name = "FILE")]
    pub targets: Vec<PathBuf>,

    #[arg(short = 'f', long = "force")]
    /// ignore nonexistent files and arguments, never prompt
    pub force: bool,

    #[arg(short = 'i')]
    /// prompt before every removal
    pub interactive_always: bool,

    #[arg(short = 'I')]
    /// prompt once before removing more than three files or when removing recursively
    pub interactive_once: bool,

    #[arg(long = "interactive", value_name = "WHEN")]
    /// prompt according to WHEN: never, once (-I), or always (-i)
    pub interactive: Option<String>,

    #[arg(short = 'r', short = 'R', long = "recursive")]
    /// remove directories and their contents recursively
    pub recursive: bool,

    #[arg(short = 'd', long = "dir')]
    /// remove empty directories
    pub dir: bool,

    #[arg(short = 'v', long = "verbose")]
    /// explain what is being done
    pub verbose: bool,

    #[arg(long = "one-file-system")]
    /// skip directories on different filesystems
    pub one_file_system: bool,

    #[arg(long = "preserve-root", value_name = "OPT")]
    /// do not remove '/' (default); with 'all', reject any separate device
    pub preserve_root: Option<Option<String>>,

    #[arg(long = "no-preserve-root")]
    /// do not treat '/' specially
    pub no_preserve_root: bool,

    #[arg(long = "graveyard")]
    /// directory where deleted files rest
    pub graveyard: Option<PathBuf>,
}
```

---

## 7. Implementation Phases

### Phase 1: Foundation (Session 1)
✅ Analysis complete
✅ Technical specification written
⏳ Remaining: Design validation

### Phase 2: Core Mode Detection (Session 2)
- [ ] Implement mode detection in `main.rs`
- [ ] Create `args_rm.rs` with basic parsing
- [ ] Add mode-aware dispatch

### Phase 3: Basic rm Functionality (Session 3)
- [ ] Implement interactive modes (NEVER/ONCE/ALWAYS)
- [ ] Add recursive flag requirement
- [ ] Implement verbose output
- [ ] Match rm error messages

### Phase 4: Advanced Features (Session 4)
- [ ] Implement preserve-root protection
- [ ] Add one-file-system support
- [ ] Implement empty directory handling (-d flag)

### Phase 5: Testing & Polish (Session 5)
- [ ] Comprehensive test suite
- [ ] Comparison tests vs GNU rm
- [ ] Documentation updates
- [ ] Shell completion updates

---

## 8. Testing Strategy

### 8.1 Unit Tests

Test each behavioral difference:
- [ ] Mode detection (argv[0] parsing)
- [ ] Interactive mode transitions
- [ ] Preserve-root protection
- [ ] Recursive flag requirement
- [ ] Verbose output format
- [ ] Exit code correctness

### 8.2 Integration Tests

Compare behavior against GNU rm:
```bash
# Test scenarios
rm -i file.txt        # Interactive always
rm -I file1 file2     # No prompt (<3 files)
rm -I file1 file2 file3 file4  # Prompt (>3 files)
rm -rf /              # Should fail with preserve-root
rm --no-preserve-root -rf /    # Should allow (scary!)
rm -v file1 file2     # Verbose output
rm dir/               # Error: is a directory
rm -r dir/            # Success
rm -d empty_dir/      # Success
rm -d nonempty_dir/   # Error: not empty
```

### 8.3 Backward Compatibility Tests

Ensure rip mode still works:
```bash
rip -i file.txt       # Inspect mode (not interactive)
rip -s                # Seance
rip -u                # Unbury
rip -d                # Decompose graveyard
```

---

## 9. Documentation Updates

### README.md

Add new section:
```markdown
## Using rip as rm replacement

rip can operate in GNU rm-compatible mode when invoked as `rm`:

\`\`\`bash
# Create symlink
sudo ln -s $(which rip) /usr/local/bin/rm

# Now 'rm' behaves like GNU rm but uses graveyard
rm -i file.txt       # Interactive prompt
rm -rf dir/          # Recursive removal
rm -v file1 file2    # Verbose output
\`\`\`

**Safety features in rm mode:**
- All deletions go to graveyard (not permanent!)
- `--preserve-root` is ON by default
- Can still restore with: `rip -u` or `$(which rip) -u`

**Compatibility:**
- 100% flag-compatible with GNU rm v9.4
- Same exit codes
- Same error messages
- Same interactive behavior
```

### Man Page / Help Text

Update help text to show mode-specific flags:
```
When invoked as 'rm':
  Operates in GNU rm-compatible mode with rm flags

When invoked as 'rip':
  Uses rip-specific flags and graveyard features
```

---

## 10. Known Limitations & Future Work

### Limitations in v1.0

1. **No xdg-trash support**: rip intentionally doesn't implement xdg-trash spec
2. **Graveyard location**: Still uses rip's graveyard, not permanent deletion
3. **Performance**: Slightly slower than rm due to graveyard operations
4. **Metadata preservation**: Some edge cases may differ from rm

### Future Enhancements

1. **Pure rm mode**: `--rm-pure` flag for actual permanent deletion?
2. **Audit logging**: Track all rm operations for security
3. **Policy framework**: System-wide policies for deletion behavior
4. **Windows compatibility**: Full rm emulation on Windows

---

## 11. Risk Analysis

### High Risk

- **Flag conflict** (`-i`, `-d`): Mitigated by mode detection
- **Breaking existing rip users**: Mitigated by mode isolation
- **Root filesystem protection**: Must be bulletproof

### Medium Risk

- **Incomplete rm compatibility**: Edge cases may differ
- **Test coverage**: Need extensive comparison testing
- **Performance regression**: Graveyard operations add overhead

### Low Risk

- **Documentation**: Easy to update
- **Backward compatibility**: Modes are separate
- **Symlink detection**: Well-understood pattern

---

## 12. References

### Source Code Analysis

- **GNU coreutils rm.c**: `/home/jan/src/coreutils/src/rm.c` (369 lines)
- **GNU coreutils remove.c**: `/home/jan/src/coreutils/src/remove.c` (648 lines)
- **GNU coreutils remove.h**: `/home/jan/src/coreutils/src/remove.h` (103 lines)
- **rip2 main.rs**: `/home/jan/src/rip2/src/main.rs` (52 lines)
- **rip2 args.rs**: `/home/jan/src/rip2/src/args.rs` (197 lines)
- **rip2 lib.rs**: `/home/jan/src/rip2/src/lib.rs` (487 lines)

### Key Behavioral Insights

From rm.c analysis:
- Interactive modes use `enum rm_interactive` (RMI_NEVER/SOMETIMES/ALWAYS)
- `prompt_once` triggers single up-front prompt for -I flag
- Preserve-root uses device/inode comparison
- Exit codes: 0 for success (including user-declined), 1 for errors

From rip2 analysis:
- Uses `clap` derive macros for argument parsing
- Graveyard location: `$TMPDIR/graveyard-$USER` or `$XDG_DATA_HOME/graveyard`
- File locking via `fs4` crate prevents concurrent operation races
- Record keeping in `.record` file for undo operations

---

**End of Specification**

*This document is a living specification and will be updated as implementation progresses.*
