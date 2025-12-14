# E2E Testing Safety Guide

## Overview

safe-rm is a file deletion tool. E2E testing deletion tools is inherently dangerous. This document outlines safe approaches to verify the tool works correctly without risking real data.

**Golden Rule:** Never test on your real user account or real filesystem.

## What Needs E2E Testing

### Non-root scenarios
- `rm file` → moves to graveyard (not deletes!)
- `rip -d` → fails with "Only root can permanently delete"
- `rm -rf /` → blocked by preserve-root
- `rm -rf ~/Desktop` → blocked by home protection
- Recovery works (`rip -u`)

### Root scenarios
- `sudo rip -d` → actually empties graveyard
- `--no-preserve-root` works (extremely dangerous to test!)
- Root bypasses work as intended

## Threat Model

| Threat | Impact | Likelihood |
|--------|--------|------------|
| Bug causes actual deletion instead of graveyard move | **CATASTROPHIC** | Low (unit tests pass) |
| Bug lets non-root permanently delete | **HIGH** | Low (adversarial tests) |
| Tester runs wrong command | **VARIABLE** | Medium |
| Bug in preserve-root | **CATASTROPHIC** | Low |

## Safety Hierarchy

### 1. Docker Container (NO host mounts) — Best for root tests

```bash
# Ephemeral container, can't touch host filesystem
docker run --rm -it ubuntu:24.04 bash

# Inside: install rust, build safe-rm, test freely
# Even rm -rf / is safe - it's isolated
```

**Why safest for root:**
- Container filesystem is completely isolated
- Root inside container ≠ root on host
- `--rm` ensures container is deleted after
- **CRITICAL:** Never use `-v` to mount host directories!

### 2. Test User + tmpfs Home — Best for non-root tests

```bash
# Create test user
sudo useradd -m -s /bin/bash safe-rm-test

# Mount tmpfs over their home (volatile - RAM only)
sudo mount -t tmpfs -o size=512M,uid=$(id -u safe-rm-test),gid=$(id -g safe-rm-test) tmpfs /home/safe-rm-test

# Test as that user
sudo -u safe-rm-test /path/to/safe-rm ...
```

**Why good for non-root:**
- Realistic user environment (real UID, real home)
- Home protection tests work properly (`~/Desktop` is real path)
- If bugs exist, only RAM-backed tmpfs is affected
- Unmount = all test data gone, no traces

### 3. VM — Maximum paranoia

- Complete isolation
- Can snapshot before tests, restore after
- Overkill for this but available if needed

## Recommended Test Environment Setup

### Phase A: Docker for root tests

Build safe-rm inside a Docker container:

```bash
# Start container with source mounted READ-ONLY
docker run --rm -it -v /home/jan/projects/safe-rm:/src:ro ubuntu:24.04 bash

# Inside container:
apt-get update && apt-get install -y curl build-essential pkg-config
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env
cp -r /src /safe-rm  # Copy to writable location
cd /safe-rm
cargo build --release
```

Test scenarios (safe inside container):
- `./target/release/rip -d` empties graveyard as root
- `rm -rf /` blocked even as root
- `rm --no-preserve-root -rf /testdir` works as root
- Container is disposable - destroy freely

### Phase B: Test user for non-root tests

```bash
# Setup (run once)
sudo useradd -m -s /bin/bash safe-rm-test
sudo mount -t tmpfs -o size=512M,uid=$(id -u safe-rm-test),gid=$(id -g safe-rm-test) tmpfs /home/safe-rm-test

# Create test directories
sudo -u safe-rm-test mkdir -p /home/safe-rm-test/Desktop
sudo -u safe-rm-test mkdir -p /home/safe-rm-test/testdir
sudo -u safe-rm-test touch /home/safe-rm-test/testfile.txt

# Run tests as test user
sudo -u safe-rm-test bash
# Now you're the test user, run safe-rm commands manually

# Cleanup when done
exit  # Leave test user shell
sudo umount /home/safe-rm-test
sudo userdel -r safe-rm-test
```

## Critical Safety Rules

1. **NEVER** mount your real home into a Docker container for these tests
2. **NEVER** test as your own user on your real filesystem
3. **ALWAYS** verify Docker has no writable `-v` mounts before root tests
4. **ALWAYS** use tmpfs for test user's home
5. **ALWAYS** review commands before running them
6. **ALWAYS** verify you're in the right environment before destructive tests

## Verification Checklist

Before running any test:

- [ ] Am I in the correct environment? (Docker / test user)
- [ ] Is this environment disposable?
- [ ] Have I verified no host mounts? (`mount | grep /home/jan` should be empty in container)
- [ ] Do I understand what this command will do?

## E2E Test Cases

### Non-root (test user environment)

| Test | Command | Expected Result | Verify |
|------|---------|-----------------|--------|
| Basic rm | `rm testfile.txt` | File in ~/.graveyard | `ls ~/.graveyard` |
| Directory rm | `rm -r testdir` | Dir in ~/.graveyard | `ls ~/.graveyard` |
| Decompose blocked | `rip -d` | Error message, exit 1 | Check exit code |
| Preserve-root | `rm -rf /` | Error message | Should fail |
| Home protection | `rm -r ~/Desktop` | Error message | Should fail |
| Recovery | `rip -u` | File restored | Check original location |

### Root (Docker environment)

| Test | Command | Expected Result | Verify |
|------|---------|-----------------|--------|
| Decompose works | `rip -d` | Graveyard emptied | `ls ~/.graveyard` |
| Preserve-root default | `rm -rf /` | Still blocked | Error message |
| No-preserve-root | `rm --no-preserve-root -rf /tmp/test` | Deleted | Check it's gone |

## Recovery Procedures

### If something goes wrong in Docker
```bash
# Just exit - container is destroyed
exit
# Or Ctrl+D
```

### If something goes wrong with test user
```bash
# Unmount tmpfs (destroys all test data)
sudo umount /home/safe-rm-test

# Remove user
sudo userdel -r safe-rm-test
```

### If you accidentally ran something on host
- STOP immediately
- Check ~/.graveyard for your files
- Use `rip -u` to recover if possible
- Check system integrity
