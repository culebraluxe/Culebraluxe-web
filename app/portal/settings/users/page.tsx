import { SettingsUsers } from "@/components/portal/settings-auth"
import { getSettingsUsers } from "@/db/settings-auth"

export const dynamic = "force-dynamic"

export default async function SettingsUsersPage() {
  const users = await getSettingsUsers()

  return <SettingsUsers users={users} />
}
