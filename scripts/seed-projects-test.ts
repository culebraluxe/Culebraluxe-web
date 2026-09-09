// Seed a MOCK Project (Project service) + its WBS items referencing REAL DEV
// people/properties/contracts. This is "the project that wraps them together" —
// there is no fake table; only the Project/WBS rows are test data.
// DEV-only. Usage: APP_ENV=development node --env-file=.env.local --import tsx scripts/seed-projects-test.ts
import { sql } from "../db/client"
import { SqlProjectRepository } from "../db/project-service-repository"
import { SqlWbsRepository } from "../db/wbs-service-repository"

const PROJECT_ID = "mock-casa-luar-listing"

type Rows = Array<{ person_id?: string }>

/** Find a real person for this listing's client/seller: prefer someone actually
 *  linked to the Casa Luar property or its listing contract, then a resolved
 *  directory person (never an arbitrary `person limit 1` row). */
async function findSellerPerson(propertyId?: string, contractId?: string): Promise<string | null> {
  const run = async (q: Promise<unknown>): Promise<string | null> => {
    try {
      const rows = (await q) as Rows
      return rows[0]?.person_id ?? null
    } catch {
      return null
    }
  }
  if (propertyId) {
    const owner = await run(
      sql`select person_id from person_property where property_id = ${propertyId} and lower(coalesce(relation_type, '')) in ('seller', 'owner') limit 1`,
    )
    if (owner) return owner
    const linked = await run(sql`select person_id from person_property where property_id = ${propertyId} limit 1`)
    if (linked) return linked
  }
  if (contractId) {
    const onContract = await run(sql`select person_id from contract_person where contract_id = ${contractId} limit 1`)
    if (onContract) return onContract
  }
  return run(
    sql`select person_id from mv_client_directory where status <> 'new' and coalesce(display_name_source, '') <> 'identity_fallback' order by name_sort_priority desc, display_name asc limit 1`,
  )
}

async function main() {
  if (process.env.APP_ENV === "production") {
    throw new Error("refusing to seed test data into PRODUCTION")
  }

  // Real DEV entity ids this mock project wraps (property + contract first, then
  // a real linked seller/client via findSellerPerson).
  const property = await sql`select id, name from property where name = 'Casa Luar' limit 1`
  const contract = await sql`select id from contract limit 1`
  const propertyId = property[0]?.id as string | undefined
  const contractId = contract[0]?.id as string | undefined
  const personId = await findSellerPerson(propertyId, contractId)
  console.log("anchoring to", { property: propertyId, person: personId ?? null, contract: contractId })

  // Idempotent reset of this seed (project + its wbs items).
  await sql`delete from wbs_item where project_id = ${PROJECT_ID}`
  await sql`delete from project where id = ${PROJECT_ID}`

  const projects = new SqlProjectRepository()
  const wbs = new SqlWbsRepository()

  await projects.create({
    id: PROJECT_ID,
    name: "Casa Luar Listing",
    owner: "Alicia",
    description: "Mock project wrapping a real property listing for the Projects workspace wiring.",
    areas: ["properties"],
  })

  const ITEMS = [
    { id: `${PROJECT_ID}-parties`, title: "Clients / Parties", category: "clients", parentId: null, entity: { type: "person" as const, id: personId ?? "" } },
    { id: `${PROJECT_ID}-property`, title: "Property", category: "properties", parentId: null, entity: { type: "property" as const, id: propertyId ?? "" } },
    { id: `${PROJECT_ID}-agreement`, title: "Listing Agreement", category: "contracts", parentId: null, entity: { type: "contract" as const, id: contractId ?? "" } },
    { id: `${PROJECT_ID}-signature`, title: "Seller Signature", category: "contracts", parentId: `${PROJECT_ID}-agreement` },
    { id: `${PROJECT_ID}-media`, title: "Media", category: "media", parentId: null },
    { id: `${PROJECT_ID}-marketing`, title: "Marketing", category: "marketing", parentId: null },
    { id: `${PROJECT_ID}-accounting`, title: "Accounting", category: "accounting", parentId: null },
  ]
  for (const item of ITEMS) {
    await wbs.create({
      id: item.id,
      title: item.title,
      category: item.category as never,
      projectId: PROJECT_ID,
      parentId: item.parentId,
      entity: item.entity && item.entity.id ? { type: item.entity.type, id: item.entity.id } : undefined,
    })
  }

  const project = await projects.get(PROJECT_ID)
  const due = await wbs.listDue({})
  console.log(
    `seeded project=${project?.id} name=${project?.name} wbsItemsLoadedByListDue=${due.filter((i) => i.projectId === PROJECT_ID).length}`,
  )
}

main().catch((e) => {
  console.error(String((e as Error)?.stack ?? e))
  process.exitCode = 1
})
