# /debug - Debugging

Analyze and resolve errors.

## Debugging Process

### 1. Collect Symptoms
- Full error message
- Reproduction steps
- Expected vs actual behavior

### 2. Form Hypotheses
- List possible causes
- Rank by likelihood

### 3. Verify Hypotheses
- Read code
- Check logs
- Run tests

### 4. Fix and Verify
- Apply fix
- Verify with tests
- Confirm no regression

## Commands

```bash
# Enable backtrace
RUST_BACKTRACE=1 cargo run

# Full backtrace
RUST_BACKTRACE=full cargo run

# Debug specific test
cargo test test_name -- --nocapture

# Detailed compile errors
cargo build 2>&1 | head -50
```

## Common Rust Error Patterns

### Ownership Error
```
error[E0382]: borrow of moved value
```
Fix: Add Clone or change to reference

### Lifetime Error
```
error[E0597]: borrowed value does not live long enough
```
Fix: Specify lifetime or transfer ownership

### Type Mismatch
```
error[E0308]: mismatched types
```
Fix: Implement Into/From trait or explicit conversion

### Async Error
```
error: future cannot be sent between threads safely
```
Fix: Add Arc<Mutex<T>> or Send + Sync bounds

## Output Format

```markdown
## Debug Report

### Symptoms
[Error message and context]

### Analysis
[Root cause analysis]

### Fix
[Applied solution]

### Verification
[Test results]
```

## Usage Example

```
/debug "cargo test failure: test_parse_config"
```
