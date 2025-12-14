<div align="center">

# safe-rm

### A drop-in `rm` replacement where permanent deletion requires root

[![crates](https://img.shields.io/crates/v/rip2.svg)](https://crates.io/crates/rip2)
[![CI](https://github.com/MilesCranmer/rip2/actions/workflows/ci.yml/badge.svg)](https://github.com/MilesCranmer/rip2/actions/workflows/ci.yml)

</div>

**safe-rm** transforms the `rm` command into a recoverable operation. Files are moved to `~/.graveyard` instead of being permanently deleted, giving you a chance to recover from mistakes.

**Core Safety Invariant:** Non-root users can NEVER permanently delete files via `rm`.

## Why safe-rm?

Traditional `rm` is unforgiving. One typo in `rm -rf` and your data is gone forever. safe-rm changes this:

- **Recoverable by default** - Files go to the graveyard, not oblivion
- **Permanent deletion requires root** - Only `sudo` can truly delete
- **Drop-in replacement** - Works with existing scripts and muscle memory
- **Built-in protections** - Blocks `rm -rf /` and accidental home directory deletion

## Quick Start

```bash
# Install
cargo install --locked rip2

# Use like rm (files go to graveyard)
rm file.txt           # Moved to ~/.graveyard
rm -rf project/       # Moved to ~/.graveyard

# Recover mistakes
rip -u                # Restore last deleted file
rip -s                # See what's in the graveyard

# Permanent deletion (requires root)
sudo rip -d           # Empty the graveyard
```

## Two Modes

safe-rm provides two interfaces:

### rm mode (POSIX-compatible)

When invoked as `rm`, it behaves like GNU rm but moves files to the graveyard instead of deleting:

```bash
rm file.txt                 # Move to graveyard
rm -r directory/            # Move directory to graveyard
rm -f nonexistent           # Silent, exit 0 (like GNU rm)
rm -v file.txt              # Verbose: "removed 'file.txt'"
```

### rip mode (ergonomic)

When invoked as `rip`, it provides a friendlier interface:

```bash
rip file.txt dir/           # No -r needed for directories
rip -u                      # Undo last deletion
rip -s                      # Seance: see deleted files from current directory
rip -i file.txt             # Inspect before deleting
```

## Safety Protections

### Preserve-root (blocks `rm -rf /`)

```bash
rm -rf /                    # Error: "dangerous to operate recursively on '/'"
rm -rf /*                   # Each path checked individually
```

### Home directory protection

```bash
rm -r ~/Desktop             # Error: "protected home directory location"
rm -r ~/Documents           # Error: protected
rm -r ~/Desktop/project     # OK: depth > 1 from home
```

### Permanent deletion requires root

```bash
rip -d                      # Error: "Only root can permanently delete files"
sudo rip -d                 # OK: empties the graveyard
```

## Installation

### Cargo

```bash
cargo install --locked rip2
```

### From source

```bash
git clone https://github.com/MilesCranmer/rip2
cd rip2
cargo install --path .
```

### As system rm replacement (Debian/Ubuntu)

To make safe-rm your system's default `rm`:

```bash
# Build and install
cargo build --release
sudo cp target/release/rip /usr/local/bin/safe-rm

# Divert system rm and replace with safe-rm
sudo dpkg-divert --add --rename --divert /usr/bin/rm.real /usr/bin/rm
sudo ln -s /usr/local/bin/safe-rm /usr/bin/rm
```

To revert:

```bash
sudo rm /usr/bin/rm
sudo dpkg-divert --remove --rename /usr/bin/rm
```

## Usage

### rip mode

```text
Usage: rip [OPTIONS] [FILES]...

Arguments:
    [FILES]...  Files and directories to remove

Options:
      --graveyard <PATH>  Directory where deleted files rest (default: ~/.graveyard)
  -d, --decompose         Permanently delete the graveyard (requires root)
  -s, --seance            Print files deleted from the current directory
  -u, --unbury            Restore the last deleted file, or specified files
  -i, --inspect           Print info about files before deletion
  -f, --force             Non-interactive mode
  -h, --help              Print help
  -V, --version           Print version
```

### rm mode

```text
Usage: rm [OPTIONS] [FILES]...

Options:
  -f, --force             Ignore nonexistent files, never prompt
  -r, -R, --recursive     Remove directories and their contents
  -d, --dir               Remove empty directories
  -v, --verbose           Explain what is being done
      --preserve-root     Do not remove '/' (default)
      --no-preserve-root  Allow removing '/' (requires root)
  -h, --help              Print help
```

## Graveyard

The graveyard is where "deleted" files rest until permanently removed. Default location: `~/.graveyard`

### Customizing location

```bash
# Via environment variable
export RIP_GRAVEYARD=~/.local/share/Trash

# Via command line
rip --graveyard /path/to/graveyard file.txt
```

### Managing the graveyard

```bash
rip graveyard             # Print graveyard path
rip -s                    # List files deleted from current directory
rip -u                    # Restore most recently deleted file
rip -u path/in/graveyard  # Restore specific file
sudo rip -d               # Permanently delete graveyard contents
```

## Examples

### Basic workflow

```bash
# Accidentally delete important file
rm important.doc

# Realize mistake, check graveyard
rip -s
# ~/.graveyard/home/user/important.doc

# Restore it
rip -u
# Returned ~/.graveyard/home/user/important.doc to /home/user/important.doc
```

### Batch restore

```bash
# Restore everything deleted from current directory
rip -su
```

### Inspect before delete

```bash
rip -i large_directory/
# large_directory: directory, 1.2 GB, 3847 files including:
#   src/
#   README.md
#   ...
# Send large_directory to the graveyard? (y/n)
```

## Migration from rip2

safe-rm is a fork of rip2 with these changes:

1. **Default graveyard changed** from `/tmp/graveyard-$USER` to `~/.graveyard`
2. **Permanent deletion requires root** - `rip -d` now fails for non-root users
3. **rm mode added** - Can be used as drop-in rm replacement
4. **Safety protections** - preserve-root and home directory depth protection

If you have files in the old `/tmp/graveyard-$USER` location, move them:

```bash
mv /tmp/graveyard-$USER/* ~/.graveyard/
```

## License

GPLv3 - See [LICENSE](LICENSE) for details.

## Acknowledgments

safe-rm is based on [rip2](https://github.com/MilesCranmer/rip2) by Miles Cranmer, which is a fork of [rip](https://github.com/nivekuil/rip) by Kevin Liu.
