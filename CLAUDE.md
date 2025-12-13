# safe-rm Development Guide

## Project Vision

**safe-rm** transforms rip2 into a drop-in replacement for Unix `rm` where **permanent deletion requires root**. Files are moved to `~/.graveyard` instead of being deleted, making accidental deletions recoverable.

**Core Safety Invariant:** Non-root users can NEVER permanently delete files via `rm`.

## Session Workflow

### 1. Orient (Start of Session)

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
  main.rs         Entry point, CLI dispatch
  lib.rs          Core logic: run(), bury_target(), get_graveyard()
  args.rs         CLI arguments (clap) - will become args/ module
  record.rs       Deletion record keeping
  util.rs         Helper functions
  safety.rs       NEW: Root checks, path protection

tests/
  integration_tests.rs   Comprehensive integration tests
  unit_tests.rs          Unit tests
```

### Critical Code Locations

| Location | Purpose | Safety-Critical |
|----------|---------|-----------------|
| `lib.rs:59-63` | Decompose (empty graveyard) | YES - needs root gate |
| `lib.rs:186-212` | Delete file in graveyard | YES - needs root gate |
| `lib.rs:573-591` | `get_graveyard()` default | Change to ~/.graveyard |
| `args.rs` | CLI parsing | Split into rip/rm modes |

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

# Run as rm (once mode detection is implemented)
RIP_MODE=rm cargo run -- file.txt
```

### Dependencies to Add

```toml
[dependencies]
dirs = "5"

[target.'cfg(unix)'.dependencies]
libc = "0.2"
```

### Platform Support

- **Linux:** Full support
- **macOS:** Full support
- **Windows:** NOT supported (drop Windows code)

---

## Current Status

Check PlanAndTrack for live status:
```
mcp__PlanAndTrack__ViewPlan("safe-rm-implementation")
```

---

## Implementation Plan

@/home/jan/.claude/plans/wondrous-brewing-wand.md
