import type { ForgeExecutionProvider } from '@/agent-runtime/gateway/provider'

const labels: Record<ForgeExecutionProvider, string> = {
  deepseek: 'DeepSeek Harness',
  warp: 'Warp / Oz',
  openclaw: 'OpenClaw',
}

type LaneRoute = {
  lane: 'Scout' | 'Smith' | 'Assay'
  profile: 'scout-volume' | 'builder-flash' | 'verifier-mini'
  provider: ForgeExecutionProvider
}

export function GatewayControl({ routes }: { routes: LaneRoute[] }) {
  return (
    <section className="mx-2 mt-3 rounded-[var(--portal-panel-radius)] border border-[var(--portal-gold)]/25 bg-[var(--portal-navy-deep)]/95 px-4 py-3 sm:mx-4">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <p className="text-[9px] font-light uppercase tracking-[0.22em] text-[var(--portal-feature-eyebrow)]">
            Forge V4 / Gateway Control
          </p>
          <div className="mt-2 flex flex-wrap items-center gap-2 text-xs text-white/80">
            <span className="rounded-md border border-white/10 bg-white/[0.04] px-3 py-1.5">Forge</span>
            <span className="text-[var(--portal-gold-soft)]">→</span>
            <span className="rounded-md border border-white/10 bg-white/[0.04] px-3 py-1.5">Execution Gateway</span>
          </div>
        </div>
        <div className="grid min-w-[22rem] gap-1.5 sm:grid-cols-3">
          {routes.map((route) => (
            <div key={route.profile} className="rounded-md border border-white/10 bg-white/[0.04] px-3 py-2">
              <div className="text-[9px] uppercase tracking-[0.14em] text-white/40">{route.lane}</div>
              <div className="mt-0.5 text-xs font-medium text-[var(--portal-gold-soft)]">{labels[route.provider]}</div>
              <div className="mt-0.5 font-mono text-[9px] text-white/45">{route.profile}</div>
            </div>
          ))}
        </div>
      </div>
      <div className="mt-2 text-[10px] text-white/45">
        Forge owns lane order · Smith is the only commit keeper · Assay owns acceptance
      </div>
    </section>
  )
}
