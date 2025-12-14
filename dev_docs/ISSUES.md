# safe-rm Issues

Tracking issues discovered during E2E testing and general development.

## Open Issues

### 1. Version string shows "rip 0.9.5" instead of "safe-rm"

**Discovered:** E2E testing session (Dec 14, 2025)

**Observed:**
```
$ /tmp/safe-rm --version
rip 0.9.5
```

**Expected:**
```
safe-rm 0.1.0
```

**Location:** Likely in `Cargo.toml` (package name/version) and possibly CLI help strings.

**Notes:**
- The binary name is `rip` in the build output
- Need to decide: rename package to `safe-rm` or keep `rip2` as package name with `safe-rm` as display name?
- Consider: should we keep `rip` as the binary name for ergonomic mode, with `rm` symlink for rm mode?

---

### 2. Interactive prompts incompatible with LLM usage

**Discovered:** E2E testing session (Dec 14, 2025)

**Problem:**
safe-rm will be primarily used by LLMs (Claude, etc.) which cannot interact with prompts. Current design has two issues:

1. **Interactive prompts block automation** - Operations like `rip -d` prompt for confirmation, which hangs when run non-interactively
2. **Root check happens after prompt** - If an operation will definitely fail (non-root trying to permanently delete), we still prompt first, wasting time

**Current behavior:**
```
$ rip -d
About to permanently delete graveyard. Continue? [y/N]  # HANGS waiting for input
# User types 'y'
Error: Only root can permanently delete files  # NOW it tells us
```

**Desired behavior:**
```
$ rip -d
Error: Only root can permanently delete files  # Immediate, no prompt
```

**Design requirements:**
1. Replace ALL interactive prompts with non-interactive flags
2. Use dynamically generated confirmation tokens (can't be reused across operations)
3. Fail-fast: if operation is known to be blocked (e.g., non-root), return denial immediately WITHOUT prompting
4. Example flow:
   ```
   $ rip -d
   Error: Permanent deletion requires root. Run with sudo.

   $ sudo rip -d
   This will permanently delete ~/.graveyard (15 files, 2.3MB)
   To confirm, run: sudo rip -d --confirm=a1b2c3d4

   $ sudo rip -d --confirm=a1b2c3d4
   Permanently deleted ~/.graveyard
   ```

**Rationale:**
- LLMs can read the confirmation token and re-run with it
- Token is single-use, preventing accidental re-execution
- No hanging on prompts
- Clear, actionable error messages

**Affected code paths:**
- `rip -d` (decompose/permanent delete)
- Big file prompts (>500MB)
- Any other interactive confirmations

---

### 3. Blocked operations return exit code 0 instead of non-zero

**Discovered:** E2E testing session (Dec 14, 2025)

**Problem:**
When safe-rm blocks an operation (preserve-root, home protection, root-only decompose), it prints the correct error message but returns exit code 0 instead of non-zero.

**Observed:**
```
$ RIP_MODE=rm safe-rm -rf /
rm: it is dangerous to operate recursively on '/'
rm: use --no-preserve-root to override this failsafe
$ echo $?
0   # Should be 1!

$ RIP_MODE=rm safe-rm -r ~/Desktop
rm: refusing to remove '/home/user/Desktop': protected home directory location
$ echo $?
0   # Should be 1!

$ echo y | rip -d
Really unlink the entire graveyard? (y/N) Exception: safe-rm: Only root can permanently delete files
$ echo $?
0   # Should be 1!
```

**Expected:**
- GNU rm returns non-zero when operations are blocked/fail
- Scripts rely on exit codes for error handling
- Exit code 0 means "success" which is misleading when operation was blocked

**Fix required:**
Ensure all error paths (preserve-root block, home protection block, root check failure) return non-zero exit codes.

**Affected code paths:**
- `run_rm()` preserve-root check
- `run_rm()` home depth protection
- `run()` decompose root check
- `run()` in-graveyard delete root check

---

### 4. Deleting files directly in ~/ should require confirmation

**Discovered:** E2E testing session (Dec 14, 2025)

**Problem:**
Files directly in the home directory (e.g., `~/important-file.txt`) can be deleted without any confirmation. This is risky since home directory files are often important configuration or personal files.

**Current behavior:**
```
$ rm ~/important-file.txt
# Silently moved to graveyard, no confirmation
```

**Expected behavior:**
```
$ rm ~/important-file.txt
rm: '/home/user/important-file.txt' is in home directory root
To confirm: rm --confirm=<token> ~/important-file.txt
```

**Rationale:**
- Files in `~/` are often important (.bashrc, .profile, documents)
- Subdirectories are already protected at depth 1
- Files should have similar protection

---

### 5. Non-root should NOT be able to change graveyard path

**Discovered:** E2E testing session (Dec 14, 2025)

**Problem:**
Non-root users can currently set a custom graveyard path via `--graveyard`, `RIP_GRAVEYARD`, or `XDG_DATA_HOME`. This could be exploited to bypass safety measures.

**Attack vector:**
```
$ RIP_GRAVEYARD=/dev/null rm -rf ~/important
# Files "moved" to /dev/null = permanent deletion!
```

**Expected behavior:**
- Non-root users MUST use the default graveyard (`~/.graveyard`)
- Custom graveyard paths should require root
- Error message: "safe-rm: Only root can change graveyard location"

**Rationale:**
- Core safety invariant: "non-root users can NEVER permanently delete"
- Custom graveyard to /dev/null or tmpfs bypasses this
- Graveyard location is a system administration decision

---

## Resolved Issues

(None yet)
