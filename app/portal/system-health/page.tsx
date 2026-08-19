import { SystemHealth } from "@/components/portal/system-health"
import { getSystemHealth } from "@/db/system-health"

export const dynamic = "force-dynamic"

export default async function SystemHealthPage() {
  const health = await getSystemHealth()

  return <SystemHealth health={health} />
}
