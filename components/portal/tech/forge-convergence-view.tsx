import type { StoryForgeConvergence } from "@/db/forge-convergence"

// ---------------------------------------------------------------------------
// Scope D — Forge execution lineage view (server-rendered).
// For each Forge story: execution generations -> candidate iterations with the
// exact SHA + QA verdict + convergence state. Read-only telemetry.
// ---------------------------------------------------------------------------

const STATE_TONE: Record<string, string> = {
  CONVERGED: "text-emerald-300",
  ITERATING: "text-sky-300",
  REPLAN_REQUIRED: "text-amber-300",
  HOLD: "text-rose-300",
}

const VERDICT_TONE: Record<string, string> = {
  PASS: "text-emerald-300",
  FAIL: "text-rose-300",
  PENDING: "text-white/40",
}

function shortSha(sha: string): string {
  return sha.length > 10 ? `${sha.slice(0, 10)}…` : sha
}

export function ForgeConvergenceView({ items }: { items: StoryForgeConvergence[] }) {
  if (items.length === 0) return null
  return (
    <section className="mt-4 overflow-hidden rounded-[calc(var(--portal-panel-radius)-6px)] border border-white/10 bg-white/[0.03]">
      <div className="flex items-baseline justify-between gap-3 border-b border-white/10 px-4 py-3">
        <div>
          <div className="text-[9px] font-light uppercase tracking-[0.18em] text-[var(--portal-on-navy)]/60">
            Forge execution lineage
          </div>
          <h2 className="mt-0.5 font-serif text-lg font-semibold leading-none text-white">
            Candidate convergence
          </h2>
        </div>
      </div>

      <div className="divide-y divide-white/10">
        {items.map((story) => (
          <div key={story.storyId} className="px-4 py-3">
            <div className="flex items-baseline gap-3">
              <span className="font-mono text-xs text-white">{story.storyId}</span>
              {story.processStatus ? (
                <span className="text-[10px] uppercase tracking-[0.14em] text-white/50">
                  process {story.processStatus}
                </span>
              ) : null}
            </div>

            <div className="mt-2 space-y-3">
              {story.executions.length === 0 ? (
                <p className="text-xs font-light text-white/40">No candidate iterations yet.</p>
              ) : (
                story.executions.map((exec) => (
                  <div key={exec.executionId} className="rounded-[10px] border border-white/10 bg-black/10 p-3">
                    <div className="flex items-center gap-3">
                      <span className="font-mono text-[11px] text-white/70">{exec.executionId}</span>
                      <span className={`text-[10px] font-medium uppercase tracking-[0.14em] ${STATE_TONE[exec.convergenceState] ?? "text-white/60"}`}>
                        {exec.convergenceState}
                      </span>
                      {exec.holdReason ? (
                        <span className="truncate text-[10px] font-light text-rose-300/80">{exec.holdReason}</span>
                      ) : null}
                      <span className="ml-auto text-[10px] text-white/40">
                        {exec.repairAttempts} repairs · {exec.iterations.length} candidates
                      </span>
                    </div>

                    <div className="mt-2 space-y-1">
                      {exec.iterations.map((it) => (
                        <div key={it.iteration} className="flex items-center gap-3 text-xs">
                          <span className="w-5 text-white/40">{it.iteration}</span>
                          <span className="font-mono text-white/90">{shortSha(it.candidateSha)}</span>
                          <span className={`font-mono text-[10px] ${VERDICT_TONE[it.qaVerdict] ?? "text-white/40"}`}>
                            QA {it.qaVerdict}
                          </span>
                          <span className="truncate text-[10px] text-white/40">
                            {it.producerNodeId}
                            {it.qaFailureClass ? ` · ${it.qaFailureClass}` : ""}
                          </span>
                        </div>
                      ))}
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>
        ))}
      </div>
    </section>
  )
}
