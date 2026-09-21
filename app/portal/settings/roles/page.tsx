import { SettingsRoles } from "@/components/portal/settings-auth"
import { getSettingsRoles } from "@/db/settings-auth"

export const dynamic = "force-dynamic"

export default async function SettingsRolesPage() {
  const roles = await getSettingsRoles()

  return <SettingsRoles roles={roles} />
}
