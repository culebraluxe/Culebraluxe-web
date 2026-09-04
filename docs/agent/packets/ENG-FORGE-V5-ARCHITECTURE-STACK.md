# ENG-FORGE-V5 — Architecture Build Stack

This file defines planned order only. Git is the ordered architecture stack; Neon remains live execution truth. Only one next story should become Ready at a time, and only after its predecessor is accepted and published.

1. `ENG-FORGE-V5-04` — True OpenCode Operational Proof
2. `ENG-FORGE-V5-05` — Forge OpenCode Agent Profile
3. `ENG-FORGE-V5-06` — Repository AGENTS Contract
4. `ENG-FORGE-V5-07` — Execution Cost and Throughput Telemetry
5. `ENG-FORGE-V5-08` — Forge Consistency Janitor
6. `ENG-FORGE-V5-09` — Stale Work Recovery and Lease Semantics
7. `ENG-FORGE-V5-10` — Ordered Story Feeder
8. `ENG-FORGE-V5-11` — Lead / Dev / QA Serial Topology
9. `ENG-FORGE-V5-12` — Decomposition and Output Contracts
10. `ENG-FORGE-V5-13` — Measured Parallelism Gate

## Sequence invariant
A successor may be promoted only when the predecessor has durable Complete evidence, exact-candidate Assay success where applicable, and accepted publication on `origin/main`.

## Stop conditions
Hold, Error, missing packet, missing execution contract, ambiguous evidence, failed Assay, publish conflict, or a human/product gate stops automatic advancement. Never skip forward to keep the queue moving.

## Operating principle
Boring serial reliability first. Earn concurrency with measured decomposition quality and cost/time evidence; never assume N parallel agents beat one coordinated path.
