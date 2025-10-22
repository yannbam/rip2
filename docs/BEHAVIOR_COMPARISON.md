# Behavior Comparison: rip vs rm-compat vs GNU rm

Quick reference for understanding behavioral differences.

---

## Command-Line Flags

| Flag | rip Mode | rm-compat Mode | GNU rm |
|------|----------|----------------|---------|
| `-i` | Inspect (show info) | Interactive always | Interactive always ✓ |
| `-I` | ❌ N/A | Interactive once | Interactive once ✓ |
| `-f` | Force (no big file prompt) | Force (never prompt) | Force (never prompt) ✓ |
| `-r`, `-R` | Auto-recursive | Recursive (required) | Recursive (required) ✓ |
| `-d` | Decompose graveyard | Remove empty dirs | Remove empty dirs ✓ |
| `-v` | ❌ N/A | Verbose output | Verbose output ✓ |
| `-s` | Seance (list deleted) | ❌ Disabled | ❌ N/A |
| `-u` | Unbury (restore) | ❌ Disabled | ❌ N/A |
| `--graveyard` | Set graveyard path | Set graveyard path | ❌ N/A |
| `--preserve-root` | ❌ N/A | Protect `/` (default ON) | Protect `/` (default ON) ✓ |
| `--no-preserve-root` | ❌ N/A | Allow removing `/` | Allow removing `/` ✓ |
| `--one-file-system` | ❌ N/A | Skip cross-filesystem | Skip cross-filesystem ✓ |
| `--interactive=WHEN` | ❌ N/A | never/once/always | never/once/always ✓ |

**Legend:** ✓ = Compatible with GNU rm | ❌ = Not available

---

## Default Behaviors

| Behavior | rip | rm-compat | GNU rm |
|----------|-----|-----------|---------|
| **Directory removal** | Automatic | Requires `-r` | Requires `-r` ✓ |
| **Missing files** | Error | Error (ok with `-f`) | Error (ok with `-f`) ✓ |
| **Deletion method** | Move to graveyard | Move to graveyard | Permanent unlink |
| **Root protection** | None | ON by default | ON by default ✓ |
| **Recovery** | Yes (`rip -u`) | Yes (`rip -u`) | No |
| **Prompting** | Big files only | Based on flags | Based on flags ✓ |
| **Verbose** | Silent | `-v` flag | `-v` flag ✓ |

---

## Interactive Prompting

### rip Mode

| Scenario | Prompt? | When? |
|----------|---------|-------|
| Regular file | Only if big (>500MB) | Before deletion |
| Directory | Only if big | Before deletion |
| With `-i` | Yes | Before deletion (shows info) |
| With `-f` | No | Never |

### rm-compat Mode

| Scenario | Interactive Mode | Prompt? |
|----------|------------------|---------|
| Any file/dir | NEVER (`-f`, `--interactive=never`) | No |
| Any file/dir | ALWAYS (`-i`, `--interactive=always`) | Yes, before each |
| >3 files OR recursive | ONCE (`-I`, `--interactive=once`) | Yes, once at start |
| ≤3 files, not recursive | ONCE (`-I`, `--interactive=once`) | No |

### GNU rm (identical to rm-compat)

Same as rm-compat mode ✓

---

## Error Messages

### rip Mode
```
Exception: No such file or directory (os error 2)
```

### rm-compat Mode
```
rm: cannot remove 'file.txt': No such file or directory
```

### GNU rm
```
rm: cannot remove 'file.txt': No such file or directory
```
✓ **Exact match**

---

## Exit Codes

| Scenario | rip | rm-compat | GNU rm |
|----------|-----|-----------|---------|
| Success | 0 | 0 | 0 ✓ |
| Any error | 1 | 1 | 1 ✓ |
| Missing file with `-f` | 1 | 0 | 0 ✓ |
| User declined `-I` prompt | 0 | 0 | 0 ✓ |
| No arguments with `-f` | 1 | 0 | 0 ✓ |
| No arguments without `-f` | 1 | 1 | 1 ✓ |

---

## Special Cases

### Removing `/` (root filesystem)

| Command | rip | rm-compat | GNU rm |
|---------|-----|-----------|---------|
| `rm /` | Would fail (permission) | Error: "refusing to remove '/' recursively" | Error ✓ |
| `rm -r /` | Would fail (permission) | Error: "refusing to remove '/' recursively" | Error ✓ |
| `rm -r --no-preserve-root /` | Would fail (permission) | Allowed (scary!) | Allowed (scary!) ✓ |

### Empty vs Non-Empty Directories

| Command | rip | rm-compat | GNU rm |
|---------|-----|-----------|---------|
| `rm emptydir/` | Success | Error: "Is a directory" | Error ✓ |
| `rm -d emptydir/` | Success | Success | Success ✓ |
| `rm -d nonemptydir/` | Success | Error: "Directory not empty" | Error ✓ |
| `rm -r nonemptydir/` | Success | Success | Success ✓ |

### Multiple Files with `-I`

| Command | rip | rm-compat | GNU rm |
|---------|-----|-----------|---------|
| `rm -I f1 f2` | N/A | No prompt (≤3 files) | No prompt ✓ |
| `rm -I f1 f2 f3` | N/A | No prompt (≤3 files) | No prompt ✓ |
| `rm -I f1 f2 f3 f4` | N/A | Prompt once | Prompt once ✓ |
| `rm -I -r dir/` | N/A | Prompt once | Prompt once ✓ |

---

## Verbose Output Format

### rip Mode
```
(no output unless error)
```

### rm-compat Mode (`-v`)
```
removed 'file1.txt'
removed directory 'dir1'
removed 'dir1/subdir/file2.txt'
```

### GNU rm (`-v`)
```
removed 'file1.txt'
removed directory 'dir1'
removed 'dir1/subdir/file2.txt'
```
✓ **Exact match**

---

## Filesystem Boundaries (`--one-file-system`)

### Scenario: Mounted filesystem
```
/home/user/data/
├── file1.txt          (on /home filesystem)
└── mount/
    └── file2.txt      (on /mnt/external filesystem)
```

| Command | rip | rm-compat | GNU rm |
|---------|-----|-----------|---------|
| `rm -r /home/user/data/` | Removes all | Removes all | Removes all |
| `rm -r --one-file-system /home/user/data/` | N/A | Removes file1.txt, skips mount/ | Removes file1.txt, skips mount/ ✓ |

---

## Recovery Operations

### Restore Last Deleted File

| Mode | Command | Works? |
|------|---------|--------|
| rip | `rip -u` | ✓ Yes |
| rm-compat | `rip -u` or `$(which rip) -u` | ✓ Yes |
| GNU rm | ❌ No recovery | ❌ |

### List Deleted Files

| Mode | Command | Works? |
|------|---------|--------|
| rip | `rip -s` | ✓ Yes |
| rm-compat | `$(which rip) -s` | ✓ Yes (must invoke as rip) |
| GNU rm | ❌ No listing | ❌ |

---

## Testing Matrix

When implementing rm-compat mode, test ALL these scenarios:

### Basic Operations
- [ ] Remove single file
- [ ] Remove multiple files
- [ ] Remove directory without `-r` (should error)
- [ ] Remove directory with `-r`
- [ ] Remove non-existent file
- [ ] Remove non-existent file with `-f` (should succeed)

### Interactive Modes
- [ ] `-i` prompts before each file
- [ ] `-I` prompts for >3 files
- [ ] `-I` prompts for recursive
- [ ] `-I` does NOT prompt for ≤3 files
- [ ] `-f` never prompts
- [ ] User accepts prompt
- [ ] User declines prompt

### Directory Handling
- [ ] `-d` removes empty directory
- [ ] `-d` fails on non-empty directory
- [ ] `-r` removes non-empty directory
- [ ] Nested directories with `-r`

### Root Protection
- [ ] Default: `rm -r /` fails
- [ ] `--preserve-root` explicitly: `rm -r --preserve-root /` fails
- [ ] `--no-preserve-root`: `rm -r --no-preserve-root /` works (if permissions allow)
- [ ] `--preserve-root=all` protects mount points

### Verbose Output
- [ ] `-v` prints each file
- [ ] `-v` with directories
- [ ] `-v` with recursive

### Edge Cases
- [ ] Files with spaces in name
- [ ] Files starting with `-`
- [ ] Symlinks
- [ ] Special files (FIFOs, sockets on Unix)
- [ ] Read-only files
- [ ] Cross-filesystem with `--one-file-system`

### Exit Codes
- [ ] Success returns 0
- [ ] Error returns 1
- [ ] User declines returns 0
- [ ] `-f` with no args returns 0
- [ ] No args without `-f` returns 1

---

## Implementation Status

### Phase 1: Foundation
- [ ] Mode detection working
- [ ] Argument parsing working
- [ ] Basic dispatch working

### Phase 2: Core Features
- [ ] Interactive modes (NEVER/ONCE/ALWAYS)
- [ ] Preserve-root protection
- [ ] Recursive requirement
- [ ] Directory handling
- [ ] Verbose output

### Phase 3: Advanced Features
- [ ] One-file-system
- [ ] Empty directory checking
- [ ] Error message format matching
- [ ] Exit code matching

### Phase 4: Integration
- [ ] Graveyard operations working
- [ ] Record keeping working
- [ ] Recovery still works
- [ ] Backward compatibility maintained

### Phase 5: Testing & Documentation
- [ ] Unit tests passing
- [ ] Integration tests passing
- [ ] Comparison tests vs GNU rm passing
- [ ] Documentation updated

---

**Reference:** This document is auto-generated from RM_COMPAT_SPEC.md
