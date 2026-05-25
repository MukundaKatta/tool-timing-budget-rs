/*!
tool-timing-budget: time budget across all tool calls in an agent run.

Limits the total wall-clock time spent on tool calls in one agent turn or
run. Different from per-tool timeouts — this enforces an aggregate cap.

```rust
use tool_timing_budget::TimingBudget;

let mut b = TimingBudget::new(5000); // 5000ms total
b.record("search", 1200);
b.record("fetch", 800);
assert_eq!(b.used_ms(), 2000);
assert!(b.has_remaining(3000));
```
*/

#[derive(Debug, Clone)]
pub struct ToolTiming {
    pub tool: String,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BudgetExceeded {
    pub used_ms: u64,
    pub budget_ms: u64,
}

impl std::fmt::Display for BudgetExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "timing budget exceeded ({}/{}ms)", self.used_ms, self.budget_ms)
    }
}

/// Tracks aggregate tool call time against a budget.
#[derive(Debug)]
pub struct TimingBudget {
    budget_ms: u64,
    timings: Vec<ToolTiming>,
}

impl TimingBudget {
    pub fn new(budget_ms: u64) -> Self {
        Self { budget_ms, timings: Vec::new() }
    }

    /// Record elapsed ms for a tool call. Does NOT enforce the limit.
    pub fn record(&mut self, tool: impl Into<String>, elapsed_ms: u64) {
        self.timings.push(ToolTiming { tool: tool.into(), elapsed_ms });
    }

    /// Record and check: returns Err if over budget after this call.
    pub fn record_and_check(
        &mut self,
        tool: impl Into<String>,
        elapsed_ms: u64,
    ) -> Result<(), BudgetExceeded> {
        self.record(tool, elapsed_ms);
        self.check()
    }

    /// Total ms spent so far.
    pub fn used_ms(&self) -> u64 {
        self.timings.iter().map(|t| t.elapsed_ms).sum()
    }

    /// Remaining ms in budget.
    pub fn remaining_ms(&self) -> u64 {
        self.budget_ms.saturating_sub(self.used_ms())
    }

    pub fn budget_ms(&self) -> u64 { self.budget_ms }

    /// True if at least `needed_ms` remain.
    pub fn has_remaining(&self, needed_ms: u64) -> bool {
        self.remaining_ms() >= needed_ms
    }

    pub fn is_exhausted(&self) -> bool { self.used_ms() >= self.budget_ms }

    pub fn fraction_used(&self) -> f64 {
        if self.budget_ms == 0 { return 1.0; }
        self.used_ms() as f64 / self.budget_ms as f64
    }

    pub fn check(&self) -> Result<(), BudgetExceeded> {
        let used = self.used_ms();
        if used >= self.budget_ms {
            Err(BudgetExceeded { used_ms: used, budget_ms: self.budget_ms })
        } else {
            Ok(())
        }
    }

    /// Most time-consuming tool call so far.
    pub fn slowest(&self) -> Option<&ToolTiming> {
        self.timings.iter().max_by_key(|t| t.elapsed_ms)
    }

    pub fn call_count(&self) -> usize { self.timings.len() }
    pub fn timings(&self) -> &[ToolTiming] { &self.timings }

    pub fn avg_ms(&self) -> f64 {
        if self.timings.is_empty() { return 0.0; }
        self.used_ms() as f64 / self.timings.len() as f64
    }

    pub fn reset(&mut self) { self.timings.clear(); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_used() {
        let mut b = TimingBudget::new(5000);
        b.record("t1", 1000);
        b.record("t2", 500);
        assert_eq!(b.used_ms(), 1500);
    }

    #[test]
    fn remaining() {
        let mut b = TimingBudget::new(5000);
        b.record("t", 2000);
        assert_eq!(b.remaining_ms(), 3000);
    }

    #[test]
    fn has_remaining_true() {
        let mut b = TimingBudget::new(5000);
        b.record("t", 1000);
        assert!(b.has_remaining(4000));
    }

    #[test]
    fn has_remaining_false() {
        let mut b = TimingBudget::new(1000);
        b.record("t", 800);
        assert!(!b.has_remaining(300));
    }

    #[test]
    fn exhausted_when_over() {
        let mut b = TimingBudget::new(500);
        b.record("t", 500);
        assert!(b.is_exhausted());
    }

    #[test]
    fn not_exhausted_under() {
        let mut b = TimingBudget::new(1000);
        b.record("t", 500);
        assert!(!b.is_exhausted());
    }

    #[test]
    fn check_ok() {
        let mut b = TimingBudget::new(1000);
        b.record("t", 400);
        assert!(b.check().is_ok());
    }

    #[test]
    fn check_exceeded() {
        let mut b = TimingBudget::new(500);
        b.record("t", 600);
        assert!(b.check().is_err());
    }

    #[test]
    fn record_and_check_convenience() {
        let mut b = TimingBudget::new(300);
        let r = b.record_and_check("t", 400);
        assert!(r.is_err());
    }

    #[test]
    fn slowest_tool() {
        let mut b = TimingBudget::new(10000);
        b.record("fast", 100);
        b.record("slow", 900);
        assert_eq!(b.slowest().unwrap().tool, "slow");
    }

    #[test]
    fn avg_ms() {
        let mut b = TimingBudget::new(10000);
        b.record("a", 200);
        b.record("b", 400);
        assert!((b.avg_ms() - 300.0).abs() < 0.01);
    }

    #[test]
    fn fraction_used() {
        let mut b = TimingBudget::new(1000);
        b.record("t", 250);
        assert!((b.fraction_used() - 0.25).abs() < 0.001);
    }

    #[test]
    fn reset_clears() {
        let mut b = TimingBudget::new(1000);
        b.record("t", 500);
        b.reset();
        assert_eq!(b.used_ms(), 0);
    }
}
