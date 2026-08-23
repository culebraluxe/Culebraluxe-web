// ═══════════════════════════════════════════════════════════════════════════
// ⚠️ TEMP ARCHITECT REVIEW SEAM — REMOVE WHEN EXTERNAL REVIEW ACCESS IS NO
//    LONGER NEEDED.
//
// Design Lab preview. The Design Lab is a client-only component playground
// with FAKE demo data — every button toggles local component state only. It
// has no data queries, no server actions, no form submissions, and no
// mutation surface, so the existing page renders as-is inside the preview.
// ═══════════════════════════════════════════════════════════════════════════

import DesignLabPage from "@/app/portal/design-lab/page"

export function ReviewDesignLab() {
  return <DesignLabPage />
}
