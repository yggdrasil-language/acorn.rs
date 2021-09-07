/// Outcome of a bounded write attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteOutcome {
    /// All requested bytes were written.
    Complete,
    /// The write would exceed the configured budget.
    WouldExceedBudget {
        /// Bytes that would have been written.
        attempted: u64,
        /// Active byte budget.
        limit: u64,
    },
}
