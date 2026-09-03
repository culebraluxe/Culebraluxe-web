# ENG-FORGE-V5-02 — OpenCode Operational Proof

## Goal
Prove one real Smith coding task can run end-to-end through the new OpenCode harness while Forge preserves the existing Neon story/evidence contract, isolated worktree, Forge-owned candidate commit, exact-candidate Assay, and accepted-candidate publish.

## Scope
Make one deliberately small, useful Forge code improvement through OpenCode. Keep the change bounded to the OpenCode integration itself: add a concise exported helper that reports the pinned OpenCode execution identity (`opencode-harness` + `deepseek/deepseek-v4-flash`) for operator/test visibility, with focused unit coverage. Do not expand into UI redesign, sessions, agents, MCP, plugins, or provider orchestration changes.

The purpose of this story is operational proof, not feature volume.

## Execution contract
- Smith MUST execute through provider `opencode` / adapter `opencode-harness`.
- Model MUST be explicitly pinned to `deepseek/deepseek-v4-flash`.
- One OpenCode run only. No continuation, fork, child session, subagent, or background session.
- No MCP.
- No OpenCode plugins.
- No DeepSeek Pro.
- Forge owns the worktree and candidate commit.
- Assay verifies the exact Smith candidate using the normal verifier path.
- Slack/Neon remain observational/system-of-record surfaces; OpenCode does not receive direct external-system authority.

## Acceptance criteria
1. Smith durable work-item evidence records `runtime_adapter = opencode-harness`.
2. OpenCode invocation uses `deepseek/deepseek-v4-flash` explicitly; no default-model selection.
3. A small real code change is produced in the isolated Smith worktree and Forge creates the candidate commit.
4. Focused tests pass for the changed OpenCode integration surface.
5. Assay runs against the exact Smith candidate and passes before publish.
6. Accepted candidate reaches `origin/main` through the existing non-force publish path.
7. No schema changes.
8. No MCP, plugins, subagents, session continuation/forking, or Pro model usage.
9. Stop after this single operational proof. Do not chain another OpenCode experiment automatically.

## Test mode
SCOPED only. Full regression forbidden.

## Assay commands
- `pnpm exec tsx --test agent-runtime/opencode/opencode-client.test.ts agent-runtime/opencode/opencode-harness-adapter.test.ts`
- `pnpm exec tsx --test agent-runtime/opencode/opencode-routing.test.ts agent-runtime/gateway/provider.test.ts`

## Cost / control rule
This is a bounded R&D proof. If OpenCode cannot complete the narrow task cleanly in the single run, fail factually and stop. Do not retry by changing model, using Pro, adding subagents, or widening scope.

## Loop
Architect packet → Smith via OpenCode Flash → Forge candidate commit → exact-candidate Assay → publish → stop and inspect evidence/cost.