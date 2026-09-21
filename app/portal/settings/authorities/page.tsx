import { SettingsAuthorities } from "@/components/portal/settings-auth"
import { getSettingsAuthorities } from "@/legacy/db/settings-auth"

export const dynamic = "force-dynamic"

export default async function SettingsAuthoritiesPage() {
  const authorities = await getSettingsAuthorities()

  return <SettingsAuthorities authorities={authorities} />
}
