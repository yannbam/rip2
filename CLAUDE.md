# safe-rm Development Guide

## Project Vision

**safe-rm** transforms rip2 into a drop-in replacement for Unix `rm` where **permanent deletion requires root**. Files are moved to `~/.graveyard` instead of being deleted, making accidental deletions recoverable.

**Core Safety Invariant:** Non-root users can NEVER permanently delete files via `rm`.

## Session Workflow

### 1. Orient (Start of Session)

**FIRST**, ground yourself in the project structure:

```bash
# See project layout
mcp__System__LSTool(path="/home/jan/projects/safe-rm")

# Read the README to understand what this project is
Read README.md
```

**THEN**, check development state:

```bash
# View current development progress
mcp__PlanAndTrack__ViewPlan("safe-rm-implementation")

# Check git status for context
git status && git log --oneline -5
```

- Review the implementation plan (linked at bottom)
- Check PlanAndTrack for current task status
- Identify the **single focused task** for this session

### 2. Understand Before Implementing

**CRITICAL:** Before writing ANY code, fully understand the related codebase.

```bash
# For each relevant source file:
lsp-cli-file rust /home/jan/projects/safe-rm/src/<file>.rs
```

- Use `lsp-cli-file` on every file you'll modify
- Read complete functions, not snippets
- Understand existing patterns before adding new code
- If uncertain, explore more before acting

### 3. Implement One Focused Task

- Work on exactly ONE task from the plan
- Mark task as `in_progress` in PlanAndTrack before starting
- Follow the implementation approach in the plan
- Write tests alongside implementation (not after)
- Test with adversarial corruption (e/test methodology)

### 4. Update Plans (End of Session)

```bash
# Mark completed tasks
mcp__PlanAndTrack__UpdateTask(...)

# View updated progress
mcp__PlanAndTrack__ViewPlan("safe-rm-implementation")
```

- Mark completed tasks as `completed`
- Add any discovered sub-tasks
- Update this CLAUDE.md if needed
- Commit with descriptive message including session ID

---

## Codebase Overview

```
src/
  main.rs         Entry point, mode detection, CLI dispatch (run_rip_mode/run_rm_mode)
  lib.rs          Core logic: run(), detect_mode_from_inputs()
  args/           CLI argument parsing module
    mod.rs        ExecutionMode enum, re-exports
    rip.rs        RipArgs, RipCommands, validate_rip_args (ergonomic interface)
    rm.rs         RmArgs, validate_rm_args (POSIX-compatible interface)
  record.rs       Deletion record keeping
  util.rs         Helper functions
  safety.rs       Root checks, path protection
  completions.rs  Shell completion generation

tests/
  integration_tests.rs   Comprehensive integration tests (uses RipArgs)
  unit_tests.rs          Unit tests (includes mode detection tests)
```

### Key Code Areas

Use `lsp-cli-file rust src/lib.rs` for current line numbers.

| Area | How to Find | Status |
|------|-------------|--------|
| Decompose gate | `grep -n "cli.decompose" src/lib.rs` — root check follows the prompt | ✅ Root gate added |
| In-graveyard delete gate | `grep -n "already in the graveyard" src/lib.rs` — root check follows the prompt | ✅ Root gate added |
| Mode detection | `grep -n "ExecutionMode" src/lib.rs` — enum and detect_mode_from_inputs() | ✅ Phase 3 complete |
| Graveyard location | `grep -n "fn get_graveyard" src/lib.rs` | ✅ Default: ~/.graveyard |
| CLI parsing | `src/args/` module | ✅ Phase 4.1-4.2: Split into rip.rs/rm.rs |
| RmArgs | `src/args/rm.rs` | ✅ All flags implemented, bridges to rip logic |

---

## Development Principles

### Safety-First Development

1. **Root gates come first** - Phase 1 must complete before other phases
2. **Never skip root checks** - Even in tests, mock them, don't remove them
3. **Test adversarially** - Corrupt code to verify tests catch bugs
4. **Clear error messages** - "safe-rm: Only root can permanently delete files"

### Testing Requirements

- Every safety feature needs a test that FAILS when the safety code is removed
- Use `MockRootChecker` trait for testing root-only behavior
- Test matrix: rip mode × rm mode × root × non-root

### Code Style

- Follow existing patterns in the codebase
- Use `e/code` commenting style (intention before implementation)
- Preserve existing test patterns from `integration_tests.rs`

### Lessons Learned

These insights emerged from actual implementation sessions:

1. **Check existing deps before adding new ones** — The codebase already had `nix` which provides `geteuid().is_root()` — a *safer* API than raw libc! We initially added `libc` per the plan, then discovered nix provides everything we need with better ergonomics. Removed libc entirely. Always check what existing deps offer.

2. **Estimate ripple effects of signature changes** — Adding a parameter to `run()` required updating 30+ test call sites. When changing core function signatures, scout the call sites first and plan for bulk updates (sed can help).

3. **Decide error placement as UX** — Root checks can go before prompting (fail fast) or after confirmation (check at moment of danger). This is a UX decision, not just technical. Think about it upfront.

4. **Be intentional about convenience wrappers** — Creating `require_root()` as a wrapper around `require_root_for_permanent_deletion(&SystemRootChecker)` seemed useful, but it went unused because we needed the injectable version everywhere. Don't create conveniences speculatively. *(The unused function was removed in Session 1a3f4571.)*

5. **Question inherited version choices** — The plan specified `dirs = "5"` but v6 existed. Investigation showed the v6 breaking change (config_dir on macOS) doesn't affect our use of home_dir(). We upgraded. Don't assume version choices in plans were deliberate — verify and update.

6. **Adversarial testing is non-negotiable** — Actually break the safety code and verify tests fail. "Tests pass" means nothing if they don't catch real bugs.

7. **The plan is a starting point, not a constraint** — The planning instance had codebase access but wrote the plan in one session without deeply investigating every decision. Implementing instances gain hands-on, in-depth knowledge that planning couldn't anticipate. Question specific details. Suggest improvements, corrections, or alternatives. After discussing with janbam, diverge freely. The plan serves us; we don't serve the plan.

8. **Use git stash for adversarial testing** — When corrupting code to verify tests catch bugs, use `git stash` before corruption and `git stash pop` after. Manual `.bak` files get confusing when doing multiple corruptions, and `git checkout <file>` can accidentally revert more than intended. One corruption → one test → one stash pop. Keep it clean.

9. **WIP commits as checkpoints** — Before making further changes to code you've already edited, commit a WIP checkpoint first. When things get confusing (and they will), you want restore points. `git commit -m "WIP: description"` takes seconds and saves hours of untangling. Better to have too many checkpoints than too few.

10. **replace_all can double-prefix** — When using the Edit tool's `replace_all` to rename `Args` to `RipArgs`, if the file already has some `RipArgs` from a previous import change, you'll get `RipRipArgs`. Always check for this pattern after bulk replacements, or do the import rename and internal renames in the same replace operation.

---

## Quick Reference

### Key Commands

```bash
# Build and test
cargo build
cargo test

# Run as rip
cargo run -- file.txt
cargo run -- -d          # decompose (root only!)

# Run as rm (mode detection implemented in Phase 3)
RIP_MODE=rm cargo run -- file.txt
```

### Dependencies Added (Phase 1)

```toml
[dependencies]
dirs = "6"  # Only using home_dir() - v6 breaking change (config_dir on macOS) doesn't affect us

[target.'cfg(unix)'.dependencies]
nix = { version = "0.29", features = ["fs", "user"] }  # user feature for geteuid().is_root()
# Note: libc NOT needed - nix re-exports it and provides safer APIs
```

### Platform Support

- **Linux:** Full support
- **macOS:** Full support
- **Windows:** NOT supported (drop Windows code)

---

## Current Status

**Phase 1: Safety Foundation — COMPLETE** (Session 93ba0f21)
**Phase 2: Graveyard Location — COMPLETE** (Session 7bf25a31)
**Phase 3: Mode Detection — COMPLETE** (Session ea890cd4)
**Phase 4: rm Mode Implementation — IN PROGRESS** (Session 0b740f97)
  - ✅ 4.1: Split args.rs into args/ module
  - ✅ 4.2: Create RmArgs struct with all flags
  - ⏳ 4.3-4.9: Remaining rm mode behavior

Check PlanAndTrack for live status:
```
mcp__PlanAndTrack__ViewPlan("safe-rm-implementation")
```

**Next:** Phase 4.3 (rm error format) or Phase 4.4 (Force mode semantics)

### Design Decisions

**Root check placement (decided Session 1a3f4571):** Root checks happen *after* user confirmation but *before* deletion. This separates two concerns:

- **Intent confirmation**: "Do you really want to permanently delete?" (prompt)
- **Permission check**: "Are you allowed to?" (root gate)

This order was chosen because:
1. The prompt confirms the user's intent is understood before checking permission
2. The error "Only root can permanently delete" is actionable → re-run with sudo
3. Fail-fast (checking first) could confuse: "Why didn't it ask me?"

If users report confusion, reconsider—but the current design is intentional.

---

## Implementation Plan

@/home/jan/.claude/plans/wondrous-brewing-wand.md
