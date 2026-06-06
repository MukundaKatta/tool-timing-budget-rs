# tool-timing-budget

A small, dependency-free Rust crate for tracking a **total wall-clock time budget across all tool calls** in an LLM agent run.

Unlike per-tool timeouts, which cap how long any single tool call may take, `tool-timing-budget` enforces an *aggregate* cap: the combined time spent across every tool call in one agent turn or run. This is useful when you want an agent to stay responsive overall, regardless of how that time is distributed between individual tools.

## What it does

You create a `TimingBudget` with a millisecond budget, record the elapsed time of each tool call as it completes, and query the budget to decide whether the agent should keep going. The crate does not measure time itself — you supply the elapsed milliseconds — so it stays free of clock dependencies and is trivial to test.

Key capabilities:

- Record per-tool timings with a label (`record`, `record_and_check`).
- Query usage: `used_ms`, `remaining_ms`, `fraction_used`, `is_exhausted`.
- Gate further work: `has_remaining(needed_ms)`, `check`.
- Inspect history: `call_count`, `timings`, `slowest`, `avg_ms`.
- Reset between runs: `reset`.

When the budget is exceeded, `check` and `record_and_check` return a `BudgetExceeded` error carrying both `used_ms` and `budget_ms`, with a human-readable `Display` implementation.

## Installation

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
tool-timing-budget = "0.1"
```

This crate has no external dependencies.

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

Use `record_and_check` to record a timing and immediately fail if the run has gone over budget:

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
assert!((budget.avg_ms() - 500.0).abs() < 0.01);
```

## API overview

| Method | Description |
| --- | --- |
| `TimingBudget::new(budget_ms)` | Create a budget with a total cap in milliseconds. |
| `record(tool, elapsed_ms)` | Record a tool timing. Does not enforce the limit. |
| `record_and_check(tool, elapsed_ms)` | Record, then return `Err(BudgetExceeded)` if over budget. |
| `used_ms()` | Total milliseconds recorded so far. |
| `remaining_ms()` | Remaining budget (saturating at zero). |
| `budget_ms()` | The configured budget. |
| `has_remaining(needed_ms)` | `true` if at least `needed_ms` remain. |
| `is_exhausted()` | `true` once usage meets or exceeds the budget. |
| `fraction_used()` | Usage as a fraction of the budget (`0.0`–`1.0`+). |
| `check()` | `Err(BudgetExceeded)` if currently over budget. |
| `slowest()` | The most time-consuming recorded call, if any. |
| `call_count()` | Number of recorded tool calls. |
| `timings()` | Slice of all recorded timings. |
| `avg_ms()` | Average duration across recorded calls. |
| `reset()` | Clear all recorded timings (keeps the budget). |

## Tech stack

- **Language:** Rust (edition 2021)
- **Dependencies:** none
- **License:** MIT

## Testing

```sh
cargo test
```

## License

Licensed under the [MIT License](https://opensource.org/licenses/MIT).
