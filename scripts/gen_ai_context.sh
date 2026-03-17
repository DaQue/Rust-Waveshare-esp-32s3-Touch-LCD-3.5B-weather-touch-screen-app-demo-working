#!/usr/bin/env bash

# Run from any directory — always writes to project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

OUT=PROJECT_CONTEXT.md

echo "# Rust Project Context" > $OUT
echo "" >> $OUT

echo "## Cargo.toml" >> $OUT
sed -n '1,120p' Cargo.toml >> $OUT
echo "" >> $OUT

echo "## Module Tree" >> $OUT
find src -name "*.rs" | sort >> $OUT
echo "" >> $OUT

echo "## Public Structs / Enums / Traits" >> $OUT
grep -R --line-number -E "pub struct|pub enum|pub trait" src >> $OUT
echo "" >> $OUT

echo "## Public Functions" >> $OUT
grep -R --line-number -E "pub fn" src >> $OUT
echo "" >> $OUT

echo "## Coding Preferences" >> $OUT
cat >> $OUT << 'EOF'

### Build & Fix Before Flashing
- ALWAYS run `cargo +esp build -Zbuild-std=std,panic_abort` and fix all errors before
  asking the user to flash or before flashing yourself.
- Do not ask the user to test on hardware until the build is clean.

### Flashing the Board
When a build is ready to flash, prompt the user to run:
```
cargo +esp run -Zbuild-std=std,panic_abort
```

### Watching the Log (no flash)
To monitor the serial output and save to disk without flashing:
```
minicom -D /dev/ttyACM0 -b 115200 -C /tmp/burn_in.log
```

### Version Bump + Commit on Flash-Ready Build
When a build is clean and ready to flash:
1. Bump the patch version in Cargo.toml by 0.0.1.
2. Run `cargo +esp build -Zbuild-std=std,panic_abort` to confirm the version change builds.
3. Create a local git commit with a short message describing what changed.
   Do NOT push yet.

### Minor Version Bump (0.1.0) + Push to GitHub
When told to bump the version by 0.1.0:
1. Bump the minor version in Cargo.toml (e.g. 0.4.x → 0.5.0, reset patch to 0).
2. Run `cargo +esp clippy -Zbuild-std=std,panic_abort` and fix ALL warnings.
3. Run `cargo +esp build -Zbuild-std=std,panic_abort` — must be clean.
4. Review all changes for AI slop:
   - Remove filler comments that restate the code ("// increment counter").
   - Remove unnecessary doc comments on private/obvious items.
   - Remove dead code, unused imports, or gratuitous abstractions introduced during edits.
   - Ensure variable/function names are idiomatic Rust, not verbose AI-style names.
   - Remove any backwards-compatibility shims or _unused suffixes on intentionally removed items.
5. Commit with a clear message, then `git push`.

EOF

echo "Context generated on $(date)" >> $OUT
