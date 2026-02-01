# /react - Execute ReAct Pattern

Explicitly perform the Reason-Act-Observe cycle.

## Cognitive Architecture: ReAct Pattern

Follow explicit thinking loops when solving complex problems.

## Cycle Steps

### 1. Reason
```
[Reason]
- Current state: {Known information}
- Goal: {What to achieve}
- Gap: {Missing information or required action}
- Plan: {Next action to take}
```

### 2. Act
```
[Act]
- Tool: {Tool to use}
- Purpose: {Purpose of this action}
```

### 3. Observe
```
[Observe]
- Result: {Action result}
- Evaluation: {Success/failure, meaning}
- Next step: {Required follow-up action}
```

## Usage Example

```
/react "Fix YAML parsing error in config.rs"
```

Output:
```
[Reason]
- Current state: config.rs exists but YAML parsing error occurs
- Goal: Identify and fix the error cause
- Gap: Need to check error message and related code
- Plan: Read config.rs first, identify error location

[Act]
- Tool: Read (config.rs)
- Purpose: Check YAML parsing related code

[Observe]
- Result: Found type mismatch at serde_yaml::from_str call
- Evaluation: Config struct and YAML schema mismatch
- Next step: Need to fix struct field types
```

## Benefits

- Decision process is clearly documented
- Easy debugging (identify which step failed)
- Maintain logical consistency in complex tasks

## Exit Conditions

Continue cycle until one of:
- Goal achieved
- Cannot proceed (user input needed)
- Unrecoverable error
