# Handoff — Agent OS Phase 2 Harness Integration — 2026-10-18

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | Phase 1 executive operating layer is on `main`. | `docs/agent/AGENT-OS-PHASE-1.md` |
| S2 | Phase 2 now has a typed Rust contract in the live Forge engine module. | `forge/src/engine/executive_swarm.rs` and `forge/src/engine/mod.rs` |
| S3 | The contract defines four executive IDs, eight expert IDs, routing, Council gates, limits, verdicts, synthesis, escalation and Forge handoff. | unit tests at bottom of `executive_swarm.rs` |
| S4 | Existing Forge XML and seven runtime services were deliberately not changed. | `forge/definitions/FORGE_SDLC-v6.xml`, `forge/src/roles/` |
| S5 | Existing OpenCode V2 harness remains the authority for actual tool permissions, skills, subagents and process execution. | `forge/src/engine/opencode_agents.rs` |
| S6 | Every Phase 2 executive/expert is required to be read-only. | `AgentAuthority::ReadOnly` in `executive_swarm.rs` |
| S7 | Phase 2 may hand a decision contract into Forge; it may not become a second implementation/release path. | `ForgeDecisionHandoff`, `docs/agent/AGENT-OS-PHASE-2.md` |

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | Do not add executive nodes to `FORGE_SDLC-v6.xml` merely to make these profiles executable. | Architecture | Integrate at the harness/executive seam first; XML remains delivery ownership. |
| H2 | Do not grant executive/expert roles edit, publication, migration, deployment or production-write authority. | Captain / Phase 2 contract | Enforce read-only in rendered OpenCode permissions and tests. |
| H3 | Do not create another role catalogue or routing table inside the harness. | Architecture | Consume IDs and `plan_swarm()` from `engine/executive_swarm.rs`. |
| H4 | Do not bypass existing subagent arming/cost-accounting rules. | Existing harness | Reuse the current `FORGE_SUBAGENTS`, child-depth and spend constraints; stricter current controls win. |

## 3. WHERE TO LOOK — task → the one place

| Your task | Read | The files you should expect to touch |
| --- | --- | --- |
| Understand Phase 2 behavior | `docs/agent/AGENT-OS-PHASE-2.md` | none |
| Consume stable agent IDs/routing | `forge/src/engine/executive_swarm.rs` | harness integration only |
| Add selectable OpenCode profiles | current profile renderer | `forge/src/engine/opencode_agents.rs` and its existing tests |
| Reuse canonical skills | skill catalogue/generator in `opencode_agents.rs` | `docs/agent/skills/*.md` only if a genuinely missing skill is required |
| Execute bounded panel | current OpenCode client/session code | existing harness execution seam; do not create a second runtime |
| Persist decision result | current decision/HOLD/artifact paths | existing `decisions`, `hold`, evidence/artifact writers as appropriate |
| Prove authority | current OpenCode V2 agent tests | `tests/tests/opencode_v2_agents.rs` or the live equivalent |

## 4. THE INTEGRATION TARGET

### A. Profile resolution

Make these IDs resolvable by the existing OpenCode renderer:

```text
exec-chief-of-staff
exec-advisor
exec-principal-architect
exec-adversary

expert-security
expert-database
expert-rust
expert-ui-mvi
expert-devops
expert-integration
expert-performance
expert-research
```

Do not duplicate these strings by hand if a direct call to `ExecutiveAgent::id()` / `ExpertAgent::id()` or a shared catalogue can be used.

All twelve profiles must render with the harness's real **read-only** permission shape.

### B. Skills

Map from the existing canonical skill library first.

Suggested starting fit (verify against current skill bodies rather than blindly accepting this table):

| Profile | Existing skills likely relevant |
| --- | --- |
| Chief of Staff | planner, workflow |
| Advisor | planner, workflow, research/ripwire where useful |
| Principal Architect | abstract-service, planner, rust-yew-mvi, security-entitlements, workflow, ui |
| Adversary | semgrep, ripwire, abstract-service, security-entitlements |
| Security | security-entitlements, semgrep, abstract-service |
| Database | neon, database-migration, abstract-service |
| Rust | rust-testing, abstract-service, ripwire |
| UI/MVI | rust-yew-mvi, ui, rust-testing |
| DevOps | workflow, rust-testing, neon as required |
| Integration | abstract-service, workflow, semgrep |
| Performance | rust-testing, ripwire, rtk |
| Research | planner, ripwire, serena |

If a capability truly does not exist, add ONE canonical skill body under `docs/agent/skills/` and let the existing renderer generate `.opencode/skills/...`. Do not hand-author the generated mirror.

### C. Swarm execution

The smallest valid implementation is:

```text
SwarmRequest
  -> plan_swarm()
  -> execute each SwarmPlan participant independently against same decision packet
  -> parse/validate ExpertVerdict
  -> Chief of Staff synthesis
  -> DecisionSynthesis
  -> persist
  -> optional ForgeDecisionHandoff
```

The harness is allowed to decide *how* a selected profile is invoked. It is not allowed to decide *which* profiles belong in the panel after `plan_swarm()` has ruled.

### D. Independence

Initial expert calls must not see the other experts' initial outputs.

Council may then perform challenge/reconsideration as a later step, but Round 1 is independent.

### E. Council

Council request must pass `CouncilGate::warranted()`.

If it does not, fail back to advisory routing or return the routing error — do not quietly run Council anyway.

Council always includes `exec-adversary` and cannot exceed `MAX_COUNCIL_EXPERTS`.

### F. Micro-agents

Phase 2 contract permits up to:

- `MAX_CHILDREN_PER_EXPERT = 3`
- `MAX_CHILD_DEPTH = 1`

But the harness already has explicit arming/accounting restrictions. Keep whichever bound is stricter.

A child is a bounded question, not a project. No child gets write/publish authority and no grandchild exists.

## 5. REQUIRED RAILS

Add/extend harness tests that prove at least:

1. every new profile resolves;
2. every new profile is `ReadOnly` in actual rendered permission policy;
3. no new profile can invoke edit/write tools;
4. no new profile can invoke publication/JJ/Git mutation paths;
5. unknown executive/expert profile fails closed;
6. normal Security panel renders Principal Architect + Security and no unrelated experts;
7. Build-vs-buy renders Advisor + Principal Architect + Research;
8. Council without the full gate does not run;
9. valid Council contains Adversary;
10. panel cap is enforced without silently dropping a requested expert;
11. one expert's initial output is not injected into another expert's initial prompt;
12. synthesized dissent survives into the durable decision/Forge handoff;
13. existing seven Forge runtime profiles and QA/DEV_OPS authority tests remain unchanged/green.

## 6. EXISTING TESTS TO RUN

At minimum after integration, run the narrowest current equivalents of:

```sh
cargo test -p forge executive_swarm
cargo test -p test-harness --test opencode_v2_agents
pnpm forge:opencode-config --check
pnpm forge:sync-agents --check
pnpm forge:packet-lint
pnpm forge:harness
```

Follow the repo's current tier rules if command names have changed. Do not invent a full regression requirement merely because this handoff names targeted rails.

## 7. OPEN — the next actions, in order

1. Read `executive_swarm.rs` and the current `opencode_agents.rs` together; identify the smallest profile-rendering seam that can consume the new typed catalogue without duplicating it.
2. Add the twelve read-only harness profiles and map existing canonical skills.
3. Add harness tests proving read-only authority/fail-closed profile resolution.
4. Add a bounded swarm runner that executes a `SwarmPlan` independently and validates `ExpertVerdict` results.
5. Add Chief of Staff synthesis and durable decision persistence using existing decision/HOLD/artifact infrastructure.
6. Wire optional `ForgeDecisionHandoff` into the current story/packet entry seam; do not modify XML unless a measured missing capability proves XML change is actually required.
7. Run the targeted gates and leave exact receipts.

## 8. NOT VERIFIED

- No local `cargo test` was run by the author of this handoff; the GitHub connector can write repository files but cannot execute the repository toolchain.
- The new typed module is designed against the current Forge crate dependencies (`serde` is already present) but DeepSeek must compile it immediately after pull and correct any repository-specific lint/edition issue before harness integration.
- No claim is made that the twelve Phase 2 profiles are currently selectable in OpenCode. That is the integration task above.
- No production, database, workflow XML, scheduler, deployment, or OpenCode generated configuration was changed by this Phase 2 landing.

## 9. ASK THE OWNER

None required for the harness integration described above. The Captain already authorized Phase 2 and asked for it to be built and checked into `main` for DeepSeek to pull and resolve against the harness.
