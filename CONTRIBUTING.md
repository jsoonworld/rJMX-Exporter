# Contributing to rJMX-Exporter

Welcome to rJMX-Exporter! We're excited that you're interested in contributing to this high-performance JMX Metric Exporter written in Rust. Whether you're fixing bugs, adding features, improving documentation, or reporting issues, your contributions are valuable to us.

## Code of Conduct

This project adheres to a Code of Conduct that all contributors are expected to follow. Please read [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) before contributing. We are committed to providing a welcoming and inclusive environment for everyone.

## How to Contribute

### Reporting Issues

If you find a bug or have a suggestion for improvement:

1. **Search existing issues** to avoid duplicates
2. **Create a new issue** with a clear, descriptive title
3. **Provide details** including:
   - Steps to reproduce (for bugs)
   - Expected vs actual behavior
   - Your environment (OS, Rust version, etc.)
   - Relevant logs or error messages

### Feature Requests

We welcome feature requests! When proposing a new feature:

1. **Open an issue** describing the feature
2. **Explain the use case** and why it would be valuable
3. **Discuss the implementation** approach if you have ideas
4. **Wait for feedback** before starting implementation

### Pull Requests

Ready to contribute code? Here's how:

1. **Fork the repository** and clone your fork
2. **Create a feature branch** from `develop`
3. **Make your changes** following our coding standards
4. **Write or update tests** for your changes
5. **Ensure all checks pass** (tests, linting, formatting)
6. **Submit a pull request** targeting the `develop` branch

## Development Setup

### Prerequisites

- **Rust** (stable, 2021 edition) - Install via [rustup](https://rustup.rs/)
- **Docker** (optional) - For running Jolokia test environment

### Setting Up Your Environment

```bash
# Clone your fork
git clone https://github.com/YOUR_USERNAME/rJMX-Exporter.git
cd rJMX-Exporter

# Add upstream remote
git remote add upstream https://github.com/jsoonworld/rJMX-Exporter.git

# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Verify installation
rustc --version
cargo --version
```

### Essential Cargo Commands

```bash
# Build the project
cargo build

# Build with optimizations (release mode)
cargo build --release

# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run linter (clippy) with strict warnings
cargo clippy -- -D warnings

# Check code formatting
cargo fmt -- --check

# Auto-format code
cargo fmt

# Generate documentation
cargo doc --open
```

## Git Workflow

We follow a branching strategy to maintain code quality and stability.

### Branch Structure

```
main (protected, production-ready)
  └── develop (integration branch)
        ├── feature/* (new features)
        ├── fix/* (bug fixes)
        └── docs/* (documentation)
```

### Branch Naming Convention

| Branch Type | Pattern | Example |
|-------------|---------|---------|
| Feature | `feature/description` | `feature/add-metric-caching` |
| Bug Fix | `fix/description` | `fix/memory-leak-collector` |
| Documentation | `docs/description` | `docs/update-api-guide` |

### Workflow Steps

1. **Sync with upstream**
   ```bash
   git checkout develop
   git fetch upstream
   git merge upstream/develop
   ```

2. **Create your branch**
   ```bash
   git checkout -b feature/your-feature-name
   ```

3. **Make changes and commit**
   ```bash
   git add <specific-files>
   git commit -m "feat: add your feature description"
   ```

4. **Push to your fork**
   ```bash
   git push origin feature/your-feature-name
   ```

5. **Open a Pull Request** targeting `develop`

## Commit Message Format

We use conventional commit messages for clear history and automated tooling.

### Format

```
type: short description in English

- Detail 1
- Detail 2

Co-Authored-By: Name <email>
```

### Types

| Type | Description |
|------|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation changes |
| `refactor` | Code refactoring (no feature change) |
| `test` | Adding or updating tests |
| `chore` | Maintenance tasks, dependencies |

### Examples

```
feat: add Prometheus histogram support

- Implement histogram bucket calculation
- Add configuration options for custom buckets
- Include unit tests for edge cases
```

```
fix: resolve memory leak in JMX collector

- Fix connection pool not releasing resources
- Add timeout for idle connections
```

## Pull Request Process

### Before Submitting

1. **Target the `develop` branch** (not `main`)
2. **Ensure all tests pass**: `cargo test`
3. **Run linting**: `cargo clippy -- -D warnings`
4. **Check formatting**: `cargo fmt -- --check`
5. **Update documentation** if needed

### PR Requirements

- [ ] Clear, descriptive title
- [ ] Description of changes and motivation
- [ ] Reference to related issues (if applicable)
- [ ] All CI checks passing
- [ ] Code review approval from maintainer

### Review Process

1. A maintainer will review your PR
2. Address any feedback or requested changes
3. Once approved, a maintainer will merge your PR
4. Your contribution will be included in the next release

## Code Quality Standards

### Absolutely Forbidden

These patterns are not allowed in production code:

```rust
// NO: panic! in production code
panic!("This should never happen");

// NO: Direct unwrap() calls
let value = some_option.unwrap();

// NO: Blocking I/O in async context
std::thread::sleep(Duration::from_secs(1)); // in async fn
```

### Required Practices

```rust
// YES: Return Result for fallible operations
pub fn parse_config(path: &str) -> Result<Config, ConfigError> {
    // ...
}

// YES: Use ? operator for error propagation
let config = parse_config(path)?;

// YES: Handle Options explicitly
let value = some_option.ok_or(MyError::ValueNotFound)?;

// YES: Use tracing for logging
tracing::info!("Starting server on port {}", port);

// YES: Document public items
/// Parses JMX MBean data from Jolokia JSON response.
///
/// # Arguments
/// * `response` - Raw JSON response from Jolokia
///
/// # Returns
/// Parsed MBean data or error if parsing fails
pub fn parse_mbean(response: &str) -> Result<MBean, ParseError> {
    // ...
}
```

### Testing Requirements

- Write tests for all new functionality
- Maintain or improve code coverage
- Include both unit tests and integration tests where appropriate

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_mbean() {
        let json = r#"{"value": {"HeapMemoryUsage": 1024}}"#;
        let result = parse_mbean(json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_invalid_json_returns_error() {
        let result = parse_mbean("invalid json");
        assert!(result.is_err());
    }
}
```

## Language Policy

**English Only** - This is an international open-source project. All contributions must be in English:

- Commit messages
- Pull request titles and descriptions
- Code comments and documentation
- Issue reports and discussions
- Variable and function names

This ensures the project is accessible to contributors worldwide.

## Getting Help

- **Questions?** Open a discussion or issue
- **Stuck?** Ask for help in your PR
- **Ideas?** We'd love to hear them in an issue

Thank you for contributing to rJMX-Exporter!
