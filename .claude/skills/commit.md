# /commit - Create Commit

Analyze changes and create a commit.

## Execution Steps

1. `git status` - Check changed files
2. `git diff` - Analyze changes
3. Draft commit message
4. Commit after user confirmation

## Commit Message Format

```text
<type>: <subject>

<body>

Co-Authored-By: Claude <noreply@anthropic.com>
```

### Types
- `feat`: New feature
- `fix`: Bug fix
- `refactor`: Refactoring
- `docs`: Documentation
- `test`: Tests
- `chore`: Build/config

## Commands

```bash
# Check status
git status

# Check changes
git diff
git diff --staged

# Add files
git add <file>
git add -p  # Partial add

# Commit
git commit -m "message"
```

## Cautions

- Check for sensitive information (.env, credentials)
- Check for large binary files
- Exclude unnecessary files (.DS_Store, etc.)
