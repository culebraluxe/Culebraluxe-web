import type { ForgePosition } from '@/agent-runtime/team'

const labels: Record<string, string> = {
  deepseek: 'DeepSeek',
  forge: 'Forge',
  warp: 'Warp / Oz',
  openclaw: 'OpenClaw',
  opencode: 'OpenCode',
}

type TeamRoute = {
  position: ForgePosition
  profile: string
  player: string
  harness: string
  field: string
  provider: string
}

const positionLabel: Record<ForgePosition, string> = {
  scout: 'Scout',
  architect: 'Architect',
  lead: 'Lead',
  smith: 'Smith',
  inspector: 'Inspector',
  assay: 'Assay',
  archive: 'Archive',
  night: 'Night',
  dev_ops: 'DEV_OPS',
}

export function GatewayControl({
  teamName,
  routes,
}: {
  teamName: string
  routes: TeamRoute[]
}) {
  return (
    <section className="mx-2 mt-3 rounded-[var(--portal-panel-radius)] border border-[var(--portal-gold)]/25 bg-[var(--portal-navy-deep)]/95 px-4 py-3 sm:mx-4">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <p className="text-[9px] font-light uppercase tracking-[0.22em] text-[var(--portal-feature-eyebrow)]">
            Forge V6 / Team & Gateway
          </p>
          <div className="mt-2 flex flex-wrap items-center gap-2 text-xs text-white/80">
            <span className="rounded-md border border-white/10 bg-white/[0.04] px-3 py-1.5">{teamName}</span>
            <span className="text-[var(--portal-gold-soft)]">→</span>
            <span className="rounded-md border border-white/10 bg-white/[0.04] px-3 py-1.5">Execution Gateway</span>
          </div>
        </div>
        <div className="grid min-w-[22rem] gap-1.5 sm:grid-cols-2 xl:grid-cols-5">
          {routes.map((route) => (
            <div key={route.position} className="rounded-md border border-white/10 bg-white/[0.04] px-3 py-2">
              <div className="text-[9px] uppercase tracking-[0.14em] text-white/40">
                {positionLabel[route.position]}
              </div>
              <div className="mt-0.5 text-xs font-medium text-[var(--portal-gold-soft)]">{route.player}</div>
              <div className="mt-1 text-[9px] text-white/55">{route.harness} · {route.field}</div>
              <div className="mt-0.5 font-mono text-[9px] text-white/40">{route.profile}</div>
              <div className="mt-0.5 text-[9px] text-white/35">gateway: {labels[route.provider] ?? route.provider}</div>
            </div>
          ))}
        </div>
      </div>
      <div className="mt-2 text-[10px] text-white/45">
        Position chooses responsibility · Player chooses intelligence · Harness chooses connection · Field chooses execution topology
      </div>
    </section>
  )
}
