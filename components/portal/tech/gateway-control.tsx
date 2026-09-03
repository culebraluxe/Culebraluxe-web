import type { ForgeExecutionProvider } from '@/agent-runtime/gateway/provider'

const labels: Record<ForgeExecutionProvider, string> = {
  deepseek: 'DeepSeek Harness',
  warp: 'Warp / Oz',
  openclaw: 'OpenClaw',
}

export function GatewayControl({ provider }: { provider: ForgeExecutionProvider }) {
  return (
    <section className="mx-2 mt-3 rounded-[var(--portal-panel-radius)] border border-[var(--portal-gold)]/25 bg-[var(--portal-navy-deep)]/95 px-4 py-3 sm:mx-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <p className="text-[9px] font-light uppercase tracking-[0.22em] text-[var(--portal-feature-eyebrow)]">
            Forge V4 / Gateway Control
          </p>
          <div className="mt-2 flex flex-wrap items-center gap-2 text-xs text-white/80">
            <span className="rounded-md border border-white/10 bg-white/[0.04] px-3 py-1.5">Forge</span>
            <span className="text-[var(--portal-gold-soft)]">→</span>
            <span className="rounded-md border border-white/10 bg-white/[0.04] px-3 py-1.5">Execution Gateway</span>
            <span className="text-[var(--portal-gold-soft)]">→</span>
            <span className="rounded-md border border-[var(--portal-gold)]/40 bg-[var(--portal-gold-pale)] px-3 py-1.5 font-medium text-[var(--portal-gold-soft)]">
              {labels[provider]}
            </span>
          </div>
        </div>
        <div className="text-right">
          <div className="text-[10px] uppercase tracking-[0.15em] text-white/40">Active provider</div>
          <div className="mt-0.5 font-mono text-xs text-white/80">{provider}</div>
          <div className="mt-1 text-[10px] text-white/45">Forge owns lanes · Smith commit · Assay acceptance</div>
        </div>
      </div>
    </section>
  )
}
