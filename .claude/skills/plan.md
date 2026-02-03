# /plan - Create Execution Plan

Analyze complex tasks and create execution plans.

## Cognitive Architecture: Plan-and-Execute Pattern

Complex tasks should be decomposed and planned before execution.

## Execution Steps

1. **Goal Analysis**: Clearly define the final objective
2. **Task Decomposition**: Break down into achievable units
3. **Dependency Mapping**: Determine task order
4. **Risk Identification**: Identify potential blockers
5. **Plan Documentation**: Write executable plan

## Plan Template

```markdown
## Goal
[Final objective description]

## Subtasks
1. [ ] Task 1 - [Description]
2. [ ] Task 2 - [Description] (depends: Task 1)
3. [ ] Task 3 - [Description]

## Risks
- [Risk 1]: [Mitigation]
- [Risk 2]: [Mitigation]

## Success Criteria
- [Criterion 1]
- [Criterion 2]
```

## Usage Example

```
/plan Implement Jolokia collector
```

Output:
```
## Goal
Implement module to collect JMX metrics from Jolokia HTTP endpoint

## Subtasks
1. [ ] Create collector/mod.rs - Define module structure
2. [ ] Implement collector/client.rs - HTTP client (depends: 1)
3. [ ] Implement collector/parser.rs - JSON parsing (depends: 1)
4. [ ] Define error types - Extend error.rs
5. [ ] Write unit tests (depends: 2, 3)
6. [ ] Write integration tests (depends: 5)

## Risks
- Network timeout: Configure reqwest timeout
- JSON schema changes: Implement flexible parsing

## Success Criteria
- cargo test passes
- cargo clippy no warnings
- Documentation complete
```

## Plan Storage

Save plans to files when needed:
- `docs/plans/` directory
- Filename with date and feature name
