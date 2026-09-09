// Seed DEV `project` + `wbs_item` with a single test project so the Projects
// page reads REAL Neon data through the service repositories.
// DEV-only: refuses to run when APP_ENV is production.
// Usage: APP_ENV=development node --env-file=.env.local --import tsx scripts/seed-projects-test.ts
import { sql } from "../db/client"
import { SqlWbsRepository } from "../db/wbs-service-repository"

const PROJECT_ID = "seed-sea-to-soul"
const ITEMS = [
  { id: "seed-s2s-parties", title: "Clients / Parties", category: "clients", parentId: null },
  { id: "seed-s2s-property", title: "Property", category: "properties", parentId: null },
  { id: "seed-s2s-agreement", title: "Listing Agreement", category: "contracts", parentId: null },
  { id: "seed-s2s-signature", title: "Seller Signature", category: "contracts", parentId: "seed-s2s-agreement" },
  { id: "seed-s2s-media", title: "Media", category: "media", parentId: null },
  { id: "seed-s2s-marketing", title: "Marketing", category: "marketing", parentId: null },
  { id: "seed-s2s-accounting", title: "Accounting", category: "accounting", parentId: null },
]

async function main() {
  if (process.env.APP_ENV === "production") {
    throw new Error("refusing to seed test data into PRODUCTION")
  }

  // Idempotent reset of this seed's rows.
  await sql`delete from wbs_item where project_id = ${PROJECT_ID}`
  await sql`delete from wbs_project where id = ${PROJECT_ID}`

  const wbs = new SqlWbsRepository()

  await wbs.createProject({
    id: PROJECT_ID,
    name: "Sea to Soul Listing",
    owner: "Lisa Penfield",
  })

  for (const item of ITEMS) {
    await wbs.create({
      id: item.id,
      title: item.title,
      category: item.category as never,
      projectId: PROJECT_ID,
      parentId: item.parentId,
    })
  }

  const project = await wbs.getProject(PROJECT_ID)
  const due = await wbs.listDue({})
  console.log(
    `seeded project=${project?.id} name=${project?.name} wbsItemsLoadedByListDue=${due.filter((i) => i.projectId === PROJECT_ID).length}`,
  )
}

main().catch((e) => {
  console.error(String((e as Error)?.stack ?? e))
  process.exitCode = 1
})
