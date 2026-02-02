# /reflect - Self-Review

Critically review written code or output.

## Cognitive Architecture: Reflection Pattern

Improve quality through self-evaluation after generating output.

## Review Perspectives

### 1. Correctness
- Are there logic errors?
- Are edge cases handled?
- Any off-by-one errors?

### 2. Constraints
- Does it meet performance targets?
- Does it comply with memory limits?
- Does it honor API contracts?

### 3. Rust Idioms
- Is ownership/borrowing correct?
- Any unnecessary clones?
- Is error handling appropriate?

### 4. Security
- Is input validated?
- Any sensitive data exposure risk?
- Is it safe from injection attacks?

### 5. Readability
- Are names clear?
- Is logic easy to understand?
- Are comments where they're needed?

## Output Format

```markdown
## Review Result

### Critical
- [file:line] Problem description

### Suggestion
- [file:line] Suggested improvement

### Good
- Description of well-done parts
```

## Usage Example

```text
/reflect collector/client.rs
```

Output:
```markdown
## Review Result: collector/client.rs

### Critical
- [Line 45] `unwrap()` used - Replace with `?` operator
- [Line 72] No timeout set - Risk of infinite wait

### Suggestion
- [Line 23] Consider using `&str` instead of `String`
- [Line 58] Adding connection pooling would improve performance

### Good
- Error type design is clear
- Async patterns correctly applied
- Unit test coverage is good
```

## Auto-fix

After review, if fixes needed:
1. Fix critical issues first
2. Apply suggestions selectively
3. Run `/reflect` again after fixes
