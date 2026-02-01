# /build - Build Project

Build the Rust project.

## Execution Steps

1. Run `cargo build`
2. Analyze compile errors (if any)
3. Summarize build results

## Options

- `/build` - Debug build
- `/build --release` - Release build (optimized)

## Commands

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Build and run
cargo run

# Release run
cargo run --release
```

## Error Handling

On compile errors:
1. Analyze error message
2. Show related code location
3. Suggest fix
