# /doc - Generate Documentation

Generate and verify Rust documentation.

## Execution Steps

1. Run `cargo doc`
2. Check build results
3. Show missing documentation warnings

## Commands

```bash
# Generate docs
cargo doc

# Generate and open in browser
cargo doc --open

# Exclude dependencies
cargo doc --no-deps

# Warn on missing docs
RUSTDOCFLAGS="-D missing_docs" cargo doc
```

## Quality Check

- Verify all public items have doc comments
- Check example code compiles (`cargo test --doc`)
- Check for broken links

## Missing Doc Example

```rust
// Warning: pub function without documentation
pub fn my_function() { }

// Recommended: Add documentation
/// Function description
///
/// # Examples
/// ```
/// let result = my_function();
/// ```
pub fn my_function() { }
```
