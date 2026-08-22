import { FactoryCommandCenter } from "@/components/portal/factory-command-center"
import { getFactoryCommandCenterSnapshot } from "@/lib/factory-command-center-data"

export const dynamic = "force-dynamic"

// ---------------------------------------------------------------------------
// AI Software Factory Command Center (ENG-16) — the PARENT operating console.
//
// One screen: executive rollup, agent dispatch/capacity, and the
// dependency-aware factory pipeline — all read projections over the canonical
// Story Board control plane (no duplicate state). `?focus=<storyId>` is set by
// the Story Execution Cockpit's "Factory" breadcrumb so drilling back preserves
// context (the story card is highlighted and scrolled into view).
// ---------------------------------------------------------------------------

export default async function FactoryCommandCenterPage({
  searchParams,
}: {
  searchParams: Promise<{ focus?: string }>
}) {
  const [{ focus }, snapshot] = await Promise.all([
    searchParams,
    getFactoryCommandCenterSnapshot(),
  ])

  return <FactoryCommandCenter snapshot={snapshot} focusStoryId={focus ?? null} />
}
