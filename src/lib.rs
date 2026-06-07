/*!
tool-timing-budget: time budget across all tool calls in an agent run.

Limits the total wall-clock time spent on tool calls in one agent turn or
run. Different from per-tool timeouts — this enforces an aggregate cap.

The crate does not measure time itself: you supply the elapsed milliseconds
for each tool call. This keeps it free of any clock dependency and trivial to
test, and lets you feed it timings from whatever source you like (real wall
clock, a mock, or replayed traces).

```rust
use tool_timing_budget::TimingBudget;

let mut b = TimingBudget::new(5000); // 5000ms total
b.record("search", 1200);
b.record("fetch", 800);
assert_eq!(b.used_ms(), 2000);
assert!(b.has_remaining(3000));
```
*/

#![forbid(unsafe_code)]

/// A single recorded tool call: which tool ran and how long it took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolTiming {
    /// Caller-supplied label identifying the tool.
    pub tool: String,
    /// Wall-clock time the call took, in milliseconds.
    pub elapsed_ms: u64,
}

/// Error returned when the aggregate budget has been reached or exceeded.
///
/// Carries both the total time used and the configured budget so callers can
/// report or log how far over the limit the run went. Implements
/// [`std::fmt::Display`] and [`std::error::Error`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetExceeded {
    /// Total milliseconds used at the point the budget was exceeded.
    pub used_ms: u64,
    /// The configured budget, in milliseconds.
    pub budget_ms: u64,
}

impl std::fmt::Display for BudgetExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "timing budget exceeded ({}/{}ms)",
            self.used_ms, self.budget_ms
        )
    }
}

impl std::error::Error for BudgetExceeded {}

/// Tracks aggregate tool-call time against a fixed budget.
///
/// Create one with [`TimingBudget::new`], feed it the elapsed time of each
/// tool call with [`record`](TimingBudget::record), and gate further work with
/// [`has_remaining`](TimingBudget::has_remaining),
/// [`check`](TimingBudget::check), or
/// [`record_and_check`](TimingBudget::record_and_check).
#[derive(Debug, Clone)]
pub struct TimingBudget {
    budget_ms: u64,
    timings: Vec<ToolTiming>,
}

impl TimingBudget {
    /// Creates a new budget allowing `budget_ms` total milliseconds across all
    /// recorded tool calls.
    ///
    /// # Examples
    ///
    /// ```
    /// use tool_timing_budget::TimingBudget;
    /// let b = TimingBudget::new(5000);
    /// assert_eq!(b.budget_ms(), 5000);
    /// assert_eq!(b.used_ms(), 0);
    /// ```
    pub fn new(budget_ms: u64) -> Self {
        Self {
            budget_ms,
            timings: Vec::new(),
        }
    }

    /// Records the elapsed milliseconds for a tool call.
    ///
    /// This only accumulates the timing; it does **not** enforce the limit. Use
    /// [`check`](TimingBudget::check) or
    /// [`record_and_check`](TimingBudget::record_and_check) to enforce it.
    ///
    /// # Examples
    ///
    /// ```
    /// use tool_timing_budget::TimingBudget;
    /// let mut b = TimingBudget::new(5000);
    /// b.record("search", 1200);
    /// assert_eq!(b.used_ms(), 1200);
    /// ```
    pub fn record(&mut self, tool: impl Into<String>, elapsed_ms: u64) {
        self.timings.push(ToolTiming {
            tool: tool.into(),
            elapsed_ms,
        });
    }

    /// Records a timing and then checks the budget in one call.
    ///
    /// Returns `Err(BudgetExceeded)` if the run is at or over budget *after*
    /// recording this call, otherwise `Ok(())`.
    ///
    /// # Examples
    ///
    /// ```
    /// use tool_timing_budget::TimingBudget;
    /// let mut b = TimingBudget::new(300);
    /// assert!(b.record_and_check("fast", 100).is_ok());
    /// assert!(b.record_and_check("slow", 400).is_err());
    /// ```
    pub fn record_and_check(
        &mut self,
        tool: impl Into<String>,
        elapsed_ms: u64,
    ) -> Result<(), BudgetExceeded> {
        self.record(tool, elapsed_ms);
        self.check()
    }

    /// Returns the total milliseconds spent so far across all recorded calls.
    ///
    /// # Examples
    ///
    /// ```
    /// use tool_timing_budget::TimingBudget;
    /// let mut b = TimingBudget::new(5000);
    /// b.record("a", 1000);
    /// b.record("b", 500);
    /// assert_eq!(b.used_ms(), 1500);
    /// ```
    pub fn used_ms(&self) -> u64 {
        self.timings.iter().map(|t| t.elapsed_ms).sum()
    }

    /// Returns the remaining milliseconds in the budget.
    ///
    /// Saturates at zero: once the run is over budget this returns `0` rather
    /// than underflowing.
    ///
    /// # Examples
    ///
    /// ```
    /// use tool_timing_budget::TimingBudget;
    /// let mut b = TimingBudget::new(5000);
    /// b.record("a", 2000);
    /// assert_eq!(b.remaining_ms(), 3000);
    /// b.record("b", 9999);
    /// assert_eq!(b.remaining_ms(), 0); // saturates
    /// ```
    pub fn remaining_ms(&self) -> u64 {
        self.budget_ms.saturating_sub(self.used_ms())
    }

    /// Returns the configured budget, in milliseconds.
    pub fn budget_ms(&self) -> u64 {
        self.budget_ms
    }

    /// Returns `true` if at least `needed_ms` remain in the budget.
    ///
    /// Useful for gating the next tool call when you have an estimate of how
    /// long it will take.
    ///
    /// # Examples
    ///
    /// ```
    /// use tool_timing_budget::TimingBudget;
    /// let mut b = TimingBudget::new(5000);
    /// b.record("a", 1000);
    /// assert!(b.has_remaining(4000));
    /// assert!(!b.has_remaining(4001));
    /// ```
    pub fn has_remaining(&self, needed_ms: u64) -> bool {
        self.remaining_ms() >= needed_ms
    }

    /// Returns `true` once the used time has reached or exceeded the budget.
    ///
    /// # Examples
    ///
    /// ```
    /// use tool_timing_budget::TimingBudget;
    /// let mut b = TimingBudget::new(500);
    /// assert!(!b.is_exhausted());
    /// b.record("a", 500);
    /// assert!(b.is_exhausted());
    /// ```
    pub fn is_exhausted(&self) -> bool {
        self.used_ms() >= self.budget_ms
    }

    /// Returns the fraction of the budget used, where `1.0` means fully used.
    ///
    /// The result is not clamped, so it can exceed `1.0` when the run has gone
    /// over budget. A zero budget is treated as fully used (`1.0`) to avoid a
    /// division by zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use tool_timing_budget::TimingBudget;
    /// let mut b = TimingBudget::new(1000);
    /// b.record("a", 250);
    /// assert!((b.fraction_used() - 0.25).abs() < 1e-9);
    /// ```
    pub fn fraction_used(&self) -> f64 {
        if self.budget_ms == 0 {
            return 1.0;
        }
        self.used_ms() as f64 / self.budget_ms as f64
    }

    /// Checks the budget without recording anything.
    ///
    /// Returns `Err(BudgetExceeded)` if used time is at or over the budget,
    /// otherwise `Ok(())`.
    ///
    /// # Examples
    ///
    /// ```
    /// use tool_timing_budget::TimingBudget;
    /// let mut b = TimingBudget::new(500);
    /// b.record("a", 400);
    /// assert!(b.check().is_ok());
    /// b.record("b", 200);
    /// assert!(b.check().is_err());
    /// ```
    pub fn check(&self) -> Result<(), BudgetExceeded> {
        let used = self.used_ms();
        if used >= self.budget_ms {
            Err(BudgetExceeded {
                used_ms: used,
                budget_ms: self.budget_ms,
            })
        } else {
            Ok(())
        }
    }

    /// Returns the most time-consuming recorded call, or `None` if no calls
    /// have been recorded.
    ///
    /// # Examples
    ///
    /// ```
    /// use tool_timing_budget::TimingBudget;
    /// let mut b = TimingBudget::new(10_000);
    /// assert!(b.slowest().is_none());
    /// b.record("fast", 100);
    /// b.record("slow", 900);
    /// assert_eq!(b.slowest().unwrap().tool, "slow");
    /// ```
    pub fn slowest(&self) -> Option<&ToolTiming> {
        self.timings.iter().max_by_key(|t| t.elapsed_ms)
    }

    /// Returns the number of recorded tool calls.
    pub fn call_count(&self) -> usize {
        self.timings.len()
    }

    /// Returns the recorded timings, in the order they were recorded.
    pub fn timings(&self) -> &[ToolTiming] {
        &self.timings
    }

    /// Returns the mean milliseconds per recorded call, or `0.0` if no calls
    /// have been recorded.
    ///
    /// # Examples
    ///
    /// ```
    /// use tool_timing_budget::TimingBudget;
    /// let mut b = TimingBudget::new(10_000);
    /// assert_eq!(b.avg_ms(), 0.0);
    /// b.record("a", 200);
    /// b.record("b", 400);
    /// assert!((b.avg_ms() - 300.0).abs() < 1e-9);
    /// ```
    pub fn avg_ms(&self) -> f64 {
        if self.timings.is_empty() {
            return 0.0;
        }
        self.used_ms() as f64 / self.timings.len() as f64
    }

    /// Clears all recorded timings, keeping the configured budget.
    ///
    /// Use this to reuse a budget across agent runs.
    ///
    /// # Examples
    ///
    /// ```
    /// use tool_timing_budget::TimingBudget;
    /// let mut b = TimingBudget::new(1000);
    /// b.record("a", 500);
    /// b.reset();
    /// assert_eq!(b.used_ms(), 0);
    /// assert_eq!(b.budget_ms(), 1000);
    /// ```
    pub fn reset(&mut self) {
        self.timings.clear();
    }
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
    fn new_starts_empty() {
        let b = TimingBudget::new(5000);
        assert_eq!(b.used_ms(), 0);
        assert_eq!(b.call_count(), 0);
        assert_eq!(b.budget_ms(), 5000);
        assert_eq!(b.remaining_ms(), 5000);
        assert!(b.timings().is_empty());
    }

    #[test]
    fn remaining() {
        let mut b = TimingBudget::new(5000);
        b.record("t", 2000);
        assert_eq!(b.remaining_ms(), 3000);
    }

    #[test]
    fn remaining_saturates_at_zero() {
        let mut b = TimingBudget::new(1000);
        b.record("t", 5000);
        assert_eq!(b.remaining_ms(), 0);
    }

    #[test]
    fn has_remaining_true() {
        let mut b = TimingBudget::new(5000);
        b.record("t", 1000);
        assert!(b.has_remaining(4000));
    }

    #[test]
    fn has_remaining_exact_boundary() {
        let mut b = TimingBudget::new(5000);
        b.record("t", 1000);
        // Exactly the remaining amount should still count as available.
        assert!(b.has_remaining(4000));
        assert!(!b.has_remaining(4001));
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
    fn check_at_exact_budget_is_err() {
        // Reaching the budget exactly counts as exceeded (>= semantics).
        let mut b = TimingBudget::new(500);
        b.record("t", 500);
        assert!(b.check().is_err());
    }

    #[test]
    fn check_error_carries_values() {
        let mut b = TimingBudget::new(500);
        b.record("t", 600);
        let err = b.check().unwrap_err();
        assert_eq!(err.used_ms, 600);
        assert_eq!(err.budget_ms, 500);
    }

    #[test]
    fn record_and_check_convenience() {
        let mut b = TimingBudget::new(300);
        let r = b.record_and_check("t", 400);
        assert!(r.is_err());
    }

    #[test]
    fn record_and_check_ok_path_records() {
        let mut b = TimingBudget::new(1000);
        assert!(b.record_and_check("t", 400).is_ok());
        // The call is recorded even when within budget.
        assert_eq!(b.used_ms(), 400);
        assert_eq!(b.call_count(), 1);
    }

    #[test]
    fn slowest_tool() {
        let mut b = TimingBudget::new(10000);
        b.record("fast", 100);
        b.record("slow", 900);
        assert_eq!(b.slowest().unwrap().tool, "slow");
    }

    #[test]
    fn slowest_empty_is_none() {
        let b = TimingBudget::new(10000);
        assert!(b.slowest().is_none());
    }

    #[test]
    fn avg_ms() {
        let mut b = TimingBudget::new(10000);
        b.record("a", 200);
        b.record("b", 400);
        assert!((b.avg_ms() - 300.0).abs() < 0.01);
    }

    #[test]
    fn avg_ms_empty_is_zero() {
        let b = TimingBudget::new(10000);
        assert_eq!(b.avg_ms(), 0.0);
    }

    #[test]
    fn fraction_used() {
        let mut b = TimingBudget::new(1000);
        b.record("t", 250);
        assert!((b.fraction_used() - 0.25).abs() < 0.001);
    }

    #[test]
    fn fraction_used_zero_budget_is_one() {
        let b = TimingBudget::new(0);
        assert_eq!(b.fraction_used(), 1.0);
    }

    #[test]
    fn fraction_used_can_exceed_one() {
        let mut b = TimingBudget::new(1000);
        b.record("t", 2000);
        assert!((b.fraction_used() - 2.0).abs() < 0.001);
    }

    #[test]
    fn zero_budget_is_immediately_exhausted() {
        let b = TimingBudget::new(0);
        assert!(b.is_exhausted());
        assert!(b.check().is_err());
        assert_eq!(b.remaining_ms(), 0);
    }

    #[test]
    fn call_count_and_timings_accessor() {
        let mut b = TimingBudget::new(10000);
        b.record("a", 10);
        b.record("b", 20);
        assert_eq!(b.call_count(), 2);
        let ts = b.timings();
        assert_eq!(ts.len(), 2);
        // Timings preserve insertion order.
        assert_eq!(ts[0].tool, "a");
        assert_eq!(ts[0].elapsed_ms, 10);
        assert_eq!(ts[1].tool, "b");
        assert_eq!(ts[1].elapsed_ms, 20);
    }

    #[test]
    fn reset_clears() {
        let mut b = TimingBudget::new(1000);
        b.record("t", 500);
        b.reset();
        assert_eq!(b.used_ms(), 0);
    }

    #[test]
    fn reset_keeps_budget() {
        let mut b = TimingBudget::new(1000);
        b.record("t", 500);
        b.reset();
        assert_eq!(b.budget_ms(), 1000);
        assert_eq!(b.call_count(), 0);
        assert_eq!(b.remaining_ms(), 1000);
    }

    #[test]
    fn budget_exceeded_display() {
        let err = BudgetExceeded {
            used_ms: 400,
            budget_ms: 300,
        };
        assert_eq!(err.to_string(), "timing budget exceeded (400/300ms)");
    }

    #[test]
    fn budget_exceeded_is_std_error() {
        // Confirm BudgetExceeded can be used as a boxed std error.
        let err = BudgetExceeded {
            used_ms: 400,
            budget_ms: 300,
        };
        let boxed: Box<dyn std::error::Error> = Box::new(err);
        assert!(boxed.to_string().contains("exceeded"));
    }

    #[test]
    fn record_accepts_string_and_str() {
        let mut b = TimingBudget::new(1000);
        b.record("borrowed", 1);
        b.record(String::from("owned"), 1);
        assert_eq!(b.call_count(), 2);
    }
}
