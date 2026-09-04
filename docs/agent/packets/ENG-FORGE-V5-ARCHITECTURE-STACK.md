# ENG-FORGE-V5 — Architecture Build Stack

This file defines planned order, not a predecessor chain. Git holds the architecture plan; Neon remains live execution truth. Forge executes serially, but a failed/Hold story must not freeze unrelated eligible work.

0. `ENG-FORGE-V5-03R` — incident history / superseded recovery attempt; not a pack gate
1. `ENG-FORGE-V5-04` — True OpenCode Operational Proof
2. `ENG-FORGE-V5-05` — Forge OpenCode Agent Profile
3. `ENG-FORGE-V5-06` — Repository AGENTS Contract
4. `ENG-FORGE-V5-07` — Execution Cost and Throughput Telemetry
5. `ENG-FORGE-V5-08` — Forge Consistency Janitor
6. `ENG-FORGE-V5-09` — Stale Work Recovery and Lease Semantics
7. `ENG-FORGE-V5-10` — Dependency-Aware Story Feeder
8. `ENG-FORGE-V5-11` — Lead / Dev / QA Serial Topology
9. `ENG-FORGE-V5-12` — Decomposition and Output Contracts
10. `ENG-FORGE-V5-13` — Measured Parallelism Gate

## Eligibility invariant
Planned order is only a tie-breaker among eligible stories. It is not a dependency. A story is blocked only by an explicit `HARD:` dependency edge in its packet. Hold/Error/Failed on one story blocks that story and its true descendants only; Forge continues with the next unrelated eligible story.

## Hard dependency graph
- `ENG-FORGE-V5-04` through `ENG-FORGE-V5-11`: independent children of the accepted V5-03 baseline.
- `ENG-FORGE-V5-12` HARD-depends on `ENG-FORGE-V5-11` because decomposition contracts extend the Lead / Dev / QA topology.
- `ENG-FORGE-V5-13` HARD-depends on `ENG-FORGE-V5-12` because parallel eligibility consumes the decomposition/output contract.
- V5-07 telemetry is useful to V5-13 when available, but is not a hard gate.

## Failure behavior
A story failure is local. Preserve its evidence and continue to the next eligible independent story. Missing packet or ambiguous hard dependency blocks only the affected story. A human/product gate blocks only the story it gates unless an explicit `HARD:` descendant requires it.

## Operating principle
Boring serial reliability first. One worker at a time; no swarm. Earn concurrency only from measured decomposition quality and cost/time evidence.
