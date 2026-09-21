import { CommandConsole } from "@/components/portal/command-console"
import { getCommandConsoleSnapshot } from "@/lib/command-console-data"

export const dynamic = "force-dynamic"

export default async function CommandConsolePage() {
  const snapshot = await getCommandConsoleSnapshot()
  return <CommandConsole snapshot={snapshot} />
}
