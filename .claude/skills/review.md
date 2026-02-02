# /review - Code Review

Review current changes or specified files.

## Execution Steps

1. Read changed or specified files
2. Analyze code quality
3. Suggest improvements

## Review Perspectives

### 1. Correctness
- Logic errors
- Edge case handling
- Error handling

### 2. Rust Idioms
- Ownership/borrowing patterns
- Idiomatic code
- Unnecessary clone/copy

### 3. Performance
- Unnecessary allocations
- Async patterns
- Loop optimization

### 4. Security
- Input validation
- Unsafe conversions
- Sensitive data exposure

### 5. Readability
- Clear naming
- Appropriate abstraction
- Comments where needed

## Usage

- `/review` - Review all changes
- `/review <file>` - Review specific file
- `/review --staged` - Review staged changes only

## Output Format

```text
## File: src/example.rs

### Critical
- Line 42: unwrap() used - Need Result handling

### Suggestion
- Line 15: Consider Vec::with_capacity()

### Good
- Error type design is clear
```
