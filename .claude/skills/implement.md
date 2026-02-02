# /implement - Implement Features

Implement planned features.

## Implementation Workflow

### 1. Verify Prerequisites
- Read related files
- Identify existing patterns
- Check dependencies

### 2. Write Tests First (TDD)
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_new_feature() {
        // Define expected behavior
        assert!(false, "Not implemented");
    }
}
```

### 3. Minimal Implementation
- Write minimal code to pass tests
- Save complex logic for later

### 4. Refactor
- Remove duplication
- Improve readability
- Optimize performance

### 5. Verify
- `cargo test`
- `cargo clippy`
- `cargo fmt`

## Code Templates

### New Module
```rust
//! Module description
//!
//! # Examples
//! ```
//! use crate::module_name;
//! ```

mod submodule;

pub use submodule::PublicType;
```

### New Struct
```rust
/// Struct description
#[derive(Debug, Clone)]
pub struct NewStruct {
    /// Field description
    field: Type,
}

impl NewStruct {
    /// Create new instance
    pub fn new(field: Type) -> Self {
        Self { field }
    }
}
```

### New Error Type
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MyError {
    #[error("Description: {0}")]
    Variant(String),

    #[error("IO error")]
    Io(#[from] std::io::Error),
}
```

### Async Function
```rust
/// Function description
///
/// # Errors
/// Describe error conditions
pub async fn async_function() -> Result<ReturnType, Error> {
    let result = some_async_op().await?;
    Ok(result)
}
```

## Checklist

Before marking complete:

- [ ] Tests written and passing
- [ ] Doc comments added
- [ ] Error handling appropriate
- [ ] No clippy warnings
- [ ] Formatted

## Usage Example

```text
/implement "Jolokia HTTP client"
```
