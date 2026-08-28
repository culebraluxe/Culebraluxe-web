import Link from "next/link"

import { RuntimeInspector } from "@/components/portal/runtime-inspector"

export const dynamic = "force-dynamic"

export default async function RuntimeInspectorPage({
  params,
}: {
  params: Promise<{ instanceId: string }>
}) {
  const { instanceId } = await params
  return (
    <div className="mx-auto max-w-6xl space-y-3 p-4 lg:p-6">
      <div className="flex items-center justify-between">
        <h1 className="font-serif text-2xl font-light text-[var(--portal-navy)]">
          Workflow Runtime Inspector
        </h1>
        <Link
          href={`/portal/workflows/${instanceId}`}
          className="text-xs font-medium uppercase tracking-[0.12em] text-[var(--portal-navy)]/60 hover:text-[var(--portal-navy)]"
        >
          ← Workflow instance
        </Link>
      </div>
      <RuntimeInspector instanceId={instanceId} />
    </div>
  )
}
