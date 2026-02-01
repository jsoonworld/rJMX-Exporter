# /check - Full Quality Check

Perform comprehensive project quality check.

## Execution Steps

1. `cargo fmt --check` - Format check
2. `cargo clippy -- -D warnings` - Strict lint
3. `cargo test` - Run tests
4. Summarize results

## Commands

```bash
# Run all checks sequentially
cargo fmt -- --check && cargo clippy -- -D warnings && cargo test
```

## Checklist Output

```
[ ] Format check (cargo fmt)
[ ] Lint check (cargo clippy)
[ ] Tests (cargo test)
[ ] Doc build (cargo doc)
```

## CI Simulation

This skill simulates CI pipeline checks locally.

## On Failure

If any step fails:
1. Show detailed error for that step
2. Do not proceed to next step
3. Suggest fix
