// ═══════════════════════════════════════════════════════════════════════════
// ⚠️ TEMP ARCHITECT REVIEW SEAM — REMOVE WHEN EXTERNAL REVIEW ACCESS IS NO
//    LONGER NEEDED.
//
// Presentation-only marking shown at the top of every preview page. No
// navigation, no links, no client interactivity.
// ═══════════════════════════════════════════════════════════════════════════

export function ReviewBanner() {
  return (
    <div className="mb-8 rounded-sm border border-[var(--portal-gold)]/40 bg-[var(--portal-gold-pale)] px-4 py-3">
      <p className="text-[10px] font-medium uppercase tracking-[0.2em] text-[var(--portal-gold-muted)]">
        TEMP ARCHITECT REVIEW SEAM
      </p>
      <p className="mt-1 text-xs font-light leading-5 text-black/55">
        REMOVE WHEN EXTERNAL REVIEW ACCESS IS NO LONGER NEEDED — read-only
        preview, not indexed.
      </p>
    </div>
  )
}
