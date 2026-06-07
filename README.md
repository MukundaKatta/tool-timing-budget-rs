# tool-timing-budget

[![CI](https://github.com/MukundaKatta/tool-timing-budget-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/MukundaKatta/tool-timing-budget-rs/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

A small, dependency-free Rust crate for tracking a **total wall-clock time
budget across all tool calls** in an LLM agent run.

Unlike per-tool timeouts, which cap how long any single tool call may take,
`tool-timing-budget` enforces an *aggregate* cap: the combined time spent across
every tool call in one agent turn or run. This is useful when you want an agent
to stay responsive overall, regardless of how that time is distributed between
individual tools.

## What it does

You create a `TimingBudget` with a millisecond budget, record the elapsed time
of each tool call as it completes, and query the budget to decide whether the
agent should keep going. The crate does **not** measure time itself — you supply
the elapsed milliseconds — so it stays free of clock dependencies and is trivial
to test. You can feed it real wall-clock measurements, a mock clock, or replayed
traces.

Key capabilities:

- Record per-tool timings with a label (`record`, `record_and_check`).
- Query usage: `used_ms`, `remaining_ms`, `fraction_used`, `is_exhausted`.
- Gate further work: `has_remaining(needed_ms)`, `check`.
- Inspect history: `call_count`, `timings`, `slowest`, `avg_ms`.
- Reset between runs: `reset`.

When the budget is exceeded, `check` and `record_and_check` return a
`BudgetExceeded` error carrying both `used_ms` and `budget_ms`, with a
human-readable `Display` implementation and a `std::error::Error` impl.

## Installation

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
tool-timing-budget = "0.1"
```

This crate has no external dependencies. The minimum supported Rust version is
1.70.

## Usage

```rust
use tool_timing_budget::TimingBudget;

// 5000ms total budget for the whole agent run.
let mut budget = TimingBudget::new(5000);

budget.record("search", 1200);
budget.record("fetch", 800);

assert_eq!(budget.used_ms(), 2000);
assert_eq!(budget.remaining_ms(), 3000);

// Decide whether there is room for the next, estimated-cost call.
if budget.has_remaining(3000) {
    // ... run the next tool ...
}
```

### Enforcing the budget

Use `record_and_check` to record a timing and immediately fail if the run has
gone over budget:

```rust
use tool_timing_budget::TimingBudget;

let mut budget = TimingBudget::new(300);

match budget.record_and_check("slow_tool", 400) {
    Ok(()) => { /* still within budget */ }
    Err(exceeded) => {
        eprintln!("{exceeded}"); // "timing budget exceeded (400/300ms)"
        // ... stop issuing further tool calls ...
    }
}
```

### Inspecting the run

```rust
use tool_timing_budget::TimingBudget;

let mut budget = TimingBudget::new(10_000);
budget.record("fast", 100);
budget.record("slow", 900);

assert_eq!(budget.call_count(), 2);
assert_eq!(budget.slowest().unwrap().tool, "slow");
assert!((budget.avg_ms() - 500.0).abs() < 1e-9);
```

### Reusing a budget across runs

```rust
use tool_timing_budget::TimingBudget;

let mut budget = TimingBudget::new(1000);
budget.record("tool", 500);

budget.reset(); // clears timings, keeps the configured budget
assert_eq!(budget.used_ms(), 0);
assert_eq!(budget.budget_ms(), 1000);
```

## API

### `TimingBudget`

| Method | Description |
| ------ | ----------- |
| `new(budget_ms: u64) -> Self` | Create a budget allowing `budget_ms` total milliseconds. |
| `record(tool, elapsed_ms)` | Record a tool call's elapsed time. Does **not** enforce the limit. |
| `record_and_check(tool, elapsed_ms) -> Result<(), BudgetExceeded>` | Record, then return `Err` if at/over budget. |
| `check() -> Result<(), BudgetExceeded>` | Check the budget without recording. |
| `used_ms() -> u64` | Total milliseconds recorded so far. |
| `remaining_ms() -> u64` | Milliseconds left (saturates at `0`). |
| `budget_ms() -> u64` | The configured budget. |
| `has_remaining(needed_ms) -> bool` | `true` if at least `needed_ms` remain. |
| `is_exhausted() -> bool` | `true` once used time reaches the budget. |
| `fraction_used() -> f64` | Fraction of the budget used (may exceed `1.0`; `0`-budget is `1.0`). |
| `slowest() -> Option<&ToolTiming>` | The most time-consuming recorded call. |
| `call_count() -> usize` | Number of recorded calls. |
| `timings() -> &[ToolTiming]` | All recorded calls, in order. |
| `avg_ms() -> f64` | Mean milliseconds per call (`0.0` if none). |
| `reset()` | Clear recorded timings, keeping the budget. |

### `ToolTiming`

A recorded call: `tool: String` and `elapsed_ms: u64`.

### `BudgetExceeded`

Error type carrying `used_ms: u64` and `budget_ms: u64`. Implements `Display`
(`"timing budget exceeded (used/budgetms)"`) and `std::error::Error`.

## Semantics & edge cases

- The budget is enforced with `>=`: reaching the budget *exactly* counts as
  exceeded.
- `remaining_ms` saturates at `0` and never underflows.
- A zero budget is immediately exhausted and `fraction_used` reports `1.0`.
- `fraction_used` is **not** clamped, so it can exceed `1.0` when over budget.

## License

Licensed under the [MIT License](LICENSE).
