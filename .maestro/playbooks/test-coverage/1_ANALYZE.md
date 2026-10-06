# Test Coverage Analysis - Baseline Measurement

## Context
- **Playbook:** Testing
- **Agent:** {{AGENT_NAME}}
- **Project:** {{AGENT_PATH}}
- **Auto Run Folder:** {{AUTORUN_FOLDER}}
- **Loop:** {{LOOP_NUMBER}}

## Default Configuration

The agent prompt may supply `[COVERAGE_TARGET]`, `[AUTO_TEST_TESTABILITY]`, and `[AUTO_TEST_IMPORTANCE]`. In Goal-Driven Auto Run mode the prompt carries a free-text goal instead, so these defaults apply when the prompt does not state a value:

| Placeholder | Default | Meaning |
|-------------|---------|---------|
| `[COVERAGE_TARGET]` | `80%` | Overall line coverage the pipeline aims for |
| `[AUTO_TEST_IMPORTANCE]` | `CRITICAL or HIGH` | Importance levels eligible for auto-implementation |
| `[AUTO_TEST_TESTABILITY]` | `EASY or MEDIUM` | Testability levels eligible for auto-implementation |

Record the resolved values in the coverage report so documents 2-5 use the same ones.

## Objective

Measure current test coverage and identify the testing landscape. This document establishes the baseline metrics that drive the test generation pipeline.

## Instructions

1. **Identify the test framework** - Detect what testing tools the project uses
2. **Run coverage analysis** - Execute test suite with coverage enabled
3. **Document current metrics** - Line, branch, and function coverage
4. **Identify testing patterns** - How existing tests are organized
5. **Output a coverage report** to `{{AUTORUN_FOLDER}}/LOOP_{{LOOP_NUMBER}}_COVERAGE_REPORT.md`

## Analysis Checklist

- [ ] **Read configured values**: Read the agent prompt for `[COVERAGE_TARGET]`, `[AUTO_TEST_TESTABILITY]`, and `[AUTO_TEST_IMPORTANCE]`. If the prompt does not state them, use the defaults in **Default Configuration** above. Use the resolved values throughout this playbook wherever you see the corresponding placeholders.

- [ ] **Measure coverage (if needed)**: First check if `{{AUTORUN_FOLDER}}/LOOP_{{LOOP_NUMBER}}_COVERAGE_REPORT.md` already exists with coverage data (look for "Overall Line Coverage:" with a percentage OR a "Coverage Method:" line — a degraded report counts as in place). If it does, skip the analysis and mark this task complete—the coverage report is already in place. If it doesn't exist, identify the project's test framework and run the test suite with coverage enabled (or the test-gate degradation above when no coverage tooling exists). Document line coverage percentage — or `unknown` with test-gate evidence — and identify lowest-covered modules. Output results to `{{AUTORUN_FOLDER}}/LOOP_{{LOOP_NUMBER}}_COVERAGE_REPORT.md`.

## How to Find Coverage Commands

This project is Rust-first (see `AGENTS.md` → "Rust First"). Check in this order:

1. **Rust workspace** - `Cargo.toml` (workspace members, `[[test]]` targets), then coverage tooling:
   - `command -v cargo-llvm-cov` → `cargo llvm-cov` (preferred; emits LCOV)
   - `command -v grcov` → `RUSTFLAGS="-C instrument-coverage" cargo test` + `grcov`
   - Plain `cargo test` / `cargo nextest` produces **no** coverage report — there is no `--coverage` flag
2. **Package scripts** - `package.json` `scripts` (this repo: `test`, `slice:check`)
3. **Make/just** - `Makefile` or `justfile` targets
4. **CI definitions** - `.github/workflows`, `gates.yml` — coverage often runs on the runner's clock, not locally

### When no coverage tooling is installed

If neither `cargo-llvm-cov` nor `grcov` is available, **do not fail the playbook and never invent a percentage**. Degrade to test-gate evidence and record it:

- **T1 — sections you touched:** `pnpm slice:check` (runs `cargo test -p` for the crates behind changed files, plus `rustfmt`). A slice owes T0 (`cargo check`) + T1 and never T2 locally.
- **T2 — whole harness, CI-owned:** `cargo nextest run --workspace --profile ci` (1062 tests). Per `AGENTS.md` ("The gate is tiered") this belongs to CI, the nightly run, and releases — run it locally only when the goal explicitly asks for a full regression.
- Record `Coverage Method: none - test-gate evidence only` in the report, list the commands run with pass/fail counts, and mark per-module coverage as `unknown`.

An accurate "coverage unknown" is a valid baseline; an estimated number poisons every later document (2_FIND_GAPS derives gaps from it, 5_PROGRESS gates on it).

## Output Format

Create/update `{{AUTORUN_FOLDER}}/LOOP_{{LOOP_NUMBER}}_COVERAGE_REPORT.md` with:

```markdown
# Coverage Report - Loop {{LOOP_NUMBER}}

## Summary
- **Overall Line Coverage:** [XX.X% | unknown]
- **Coverage Method:** [cargo llvm-cov | grcov | none - test-gate evidence only]
- **Target:** [COVERAGE_TARGET]
- **Gap to Target:** [XX.X% | unknown]
- **Test Framework:** [name and version]
- **Coverage Command Used:** [the command that was run]
- **Total Test Files:** [count]
- **Total Test Cases:** [count]

## Coverage by Module

| Module | Lines | Branches | Functions | Status |
|--------|-------|----------|-----------|--------|
| [module1] | XX% | XX% | XX% | [NEEDS WORK / OK] |
| [module2] | XX% | XX% | XX% | [NEEDS WORK / OK] |
| ... | ... | ... | ... | ... |

## Lowest Coverage Files

Files with coverage below 50% that are good testing candidates:

1. **[filename]** - [XX%] line coverage
   - [Brief description of what this file does]
   - [Why it's important to test]

2. **[filename]** - [XX%] line coverage
   - ...

## Existing Test Patterns

### Test Location
- [ ] Tests alongside source files
- [ ] Tests in dedicated test directories
- [ ] Tests follow naming convention: [describe pattern]
- [ ] Other: [describe]

### Mocking Patterns
- [How the project handles mocks and test doubles]

### Fixture Patterns
- [How test data is organized - factories, fixtures, inline data]

## Recommendations

### Quick Wins (Easy to test, high impact)
1. [Module/file] - [why it's a quick win]

### Requires Setup (Need mocking infrastructure)
1. [Module/file] - [what setup is needed]

### Skip for Now (Low priority or too complex)
1. [Module/file] - [reason to skip]
```

## Guidelines

- **Be accurate**: Run actual coverage commands, don't estimate. If no coverage tooling exists, record that fact rather than guessing a number.
- **Note patterns**: Understanding existing tests helps write consistent new ones
- **Identify blockers**: Some code may need refactoring before it's testable
- **Focus on gaps**: We care most about untested critical code
