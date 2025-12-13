# Old (non-forked) rip Graveyard behavior

The graveyard is where rip2 stores deleted files, allowing recovery via `rip -u`.

## Location Determination

The graveyard location is determined by `get_graveyard()` in `src/lib.rs:573-591`.

**Priority order (highest to lowest):**

### 1. CLI Flag `--graveyard <PATH>`

```bash
rip --graveyard /custom/path file.txt
```

If provided, this path is used directly.

### 2. Environment Variable `$RIP_GRAVEYARD`

```bash
export RIP_GRAVEYARD=/my/graveyard
rip file.txt  # Uses /my/graveyard
```

### 3. Environment Variable `$XDG_DATA_HOME`

```bash
export XDG_DATA_HOME=~/.local/share
rip file.txt  # Uses ~/.local/share/graveyard
```

The path is constructed as `$XDG_DATA_HOME/graveyard`.

### 4. System Temp Directory (Default)

```bash
rip file.txt  # Uses /tmp/graveyard-$USER
```

Falls back to `env::temp_dir()` joined with `graveyard-{user}`, where user is:
- `$USER` on Unix
- `$USERNAME` on Windows
- `"unknown"` if neither is set

**Quick reference:**

| Condition | Graveyard Path |
|-----------|----------------|
| `--graveyard <PATH>` | `<PATH>` |
| `$RIP_GRAVEYARD` set | `$RIP_GRAVEYARD` |
| `$XDG_DATA_HOME` set | `$XDG_DATA_HOME/graveyard` |
| Default | `/tmp/graveyard-$USER` |

**View current location:**

```bash
rip graveyard
```

## Emptying the Graveyard

### 1. Delete Entire Graveyard: `rip -d` / `rip --decompose`

```bash
rip -d          # Prompts: "Really unlink the entire graveyard?"
rip -df         # Force mode - no prompt, deletes immediately
```

This calls `fs::remove_dir_all(graveyard)` to delete the entire graveyard directory.

**Code location:** `src/lib.rs:59-63`

### 2. Delete Individual Items Already in Graveyard

Running `rip` on a file that's already inside the graveyard permanently deletes it:

```bash
rip /tmp/graveyard-jan/home/jan/somefile.txt     # Prompts to permanently unlink
rip -f /tmp/graveyard-jan/home/jan/somefile.txt  # No prompt, deletes immediately
```

The code detects if `source.starts_with(graveyard)` and prompts:
`"{file} is already in the graveyard. Permanently unlink it?"`

**Code location:** `src/lib.rs:186-206`

### 3. Manual Deletion

```bash
rm -rf /tmp/graveyard-jan      # Direct path
rm -rf $(rip graveyard)        # Using rip's graveyard subcommand
```

**Summary table:**

| Method | Scope | Prompts? |
|--------|-------|----------|
| `rip -d` | Entire graveyard | Yes |
| `rip -df` | Entire graveyard | No |
| `rip <graveyard-file>` | Single item | Yes |
| `rip -f <graveyard-file>` | Single item | No |
| `rm -rf $(rip graveyard)` | Entire graveyard | No |
