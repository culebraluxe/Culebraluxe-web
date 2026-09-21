import { getClients } from "@/db/clients"

export const dynamic = "force-dynamic"

export default async function DbTestPage() {
  const clients = await getClients()

  return (
    <main className="p-10">
      <h1 className="font-serif text-3xl font-light">
        Database Test
      </h1>

      <pre className="mt-8 overflow-auto bg-white p-6 text-sm">
        {JSON.stringify(
          {
            connected: true,
            clientCount: clients.length,
            clients,
          },
          null,
          2
        )}
      </pre>
    </main>
  )
}