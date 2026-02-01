# /test - Run Tests

Execute Rust project tests.

## Execution Steps

1. Run `cargo test`
2. Analyze failures if any
3. Provide test coverage summary

## Options

- `/test` - Run all tests
- `/test <name>` - Run specific test
- `/test --nocapture` - Show output

## Commands

```bash
# Basic test
cargo test

# Specific test
cargo test <test_name>

# Show output
cargo test -- --nocapture

# Specific module
cargo test <module>::
```

## On Failure

When tests fail:
1. Display failed test name and error message
2. Identify related code location
3. Suggest fix (on request)
