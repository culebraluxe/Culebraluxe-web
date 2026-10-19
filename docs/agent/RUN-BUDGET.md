# Run budget — time check card

**RUN_BUDGET_MINUTES: 120**

Rule: any Forge probe/cohort, CTO staff task, or automated run whose **projected or observed wall
time exceeds N minutes is flagged to the principal — not blocked**. Record the estimate/elapsed,
the reason, and continue unless the principal redirects. Change N only in this card.

Dollar/token rate limits are intentionally NOT enforced: free models are in use and there is no
spend worth gating. Cost may still be *reported* (placeholder DSH rates) but it does not gate work.
