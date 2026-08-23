// ═══════════════════════════════════════════════════════════════════════════
// ⚠️ TEMP ARCHITECT REVIEW SEAM — REMOVE WHEN EXTERNAL REVIEW ACCESS IS NO
//    LONGER NEEDED.
//
// Read-only Command Center preview. Reuses the exact snapshot reader and
// presentational component as /portal/command-center
// (getFactoryCommandCenterSnapshot + FactoryCommandCenter). The component is a
// pure read projection — the only interactive elements are drill-down links to
// auth-gated /portal/* pages, which continue to enforce normal portal auth.
// No command dispatch, no server actions, no mutation surface.
// ═══════════════════════════════════════════════════════════════════════════

import { PageHeader } from "@/components/portal/page-header"
import { FactoryCommandCenter } from "@/components/portal/factory-command-center"
import { getFactoryCommandCenterSnapshot } from "@/lib/factory-command-center-data"

export async function ReviewCommandCenter() {
  const snapshot = await getFactoryCommandCenterSnapshot()

  return (
    <div>
      <PageHeader
        eyebrow="Portal Preview"
        title="Command Center"
        subtitle="Read-only preview — same live projection as /portal/command-center."
      />
      <FactoryCommandCenter snapshot={snapshot} focusStoryId={null} />
    </div>
  )
}
