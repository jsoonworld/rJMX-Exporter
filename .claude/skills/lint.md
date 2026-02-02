# /lint - Code Quality Check

Check Rust code formatting and linting.

## Execution Steps

1. `cargo fmt --check` - Format check
2. `cargo clippy` - Lint check
3. Summarize issues and suggest fixes

## Commands

```bash
# Format check (no changes)
cargo fmt -- --check

# Auto-fix formatting
cargo fmt

# Clippy lint
cargo clippy

# Strict mode (warnings as errors)
cargo clippy -- -D warnings
```

## Auto-fix

On format issues:
1. Run `cargo fmt` for auto-fix
2. Display changed files

On clippy warnings:
1. Explain each warning
2. Suggest fix
3. Auto-fix on request
