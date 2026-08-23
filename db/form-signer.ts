import type { QueryExecutor } from "./query-executor"
import type { FormSignerPerson } from "@/lib/forms/signer-resolution"

const ROLE_MAP: Record<string, string> = {
  client: "BUYER",
  seller: "SELLER",
  owner: "SELLER_BROKER",
}

async function executor(): Promise<QueryExecutor> {
  const client = await import("./client")
  return client.sql
}

export async function listFormSignerPeople(
  formId: string,
  execute?: QueryExecutor,
): Promise<FormSignerPerson[]> {
  const q = execute ?? (await executor())
  const people: FormSignerPerson[] = []

  const formRows = await q`
    select f.person_id, f.deal_id, person.display_name as person_name,
      person.id as resolved_person_id
    from document_form_instance f
    left join person on person.id = f.person_id
    where f.id = ${formId}
    limit 1
  `
  const formRow = formRows[0] as
    | {
        person_id?: string | null
        deal_id?: string | null
        person_name?: string | null
        resolved_person_id?: string | null
      }
    | undefined
  if (!formRow) return []

  if (formRow.resolved_person_id) {
    const emailRows = await q`
      select identity_value
      from person_identity
      where person_id = ${formRow.resolved_person_id}
        and identity_type = 'email'
      order by is_primary desc, created_at asc
      limit 1
    `
    people.push({
      personId: String(formRow.resolved_person_id),
      name: String(formRow.person_name ?? ""),
      email: emailRows[0]?.identity_value
        ? String(emailRows[0].identity_value)
        : null,
      role: "CLIENT",
    })
  }

  if (formRow.deal_id) {
    const clientRows = await q`
      select p.id, p.display_name,
        (
          select pi.identity_value
          from person_identity pi
          where pi.person_id = p.id
            and pi.identity_type = 'email'
          order by pi.is_primary desc, pi.created_at asc
          limit 1
        ) as email
      from deal d
      join person p on p.id = d.client_person_id
      where d.id = ${formRow.deal_id}
      limit 1
    `
    const client = clientRows[0] as
      | { id?: string; display_name?: string; email?: string | null }
      | undefined
    if (client?.id) {
      people.push({
        personId: String(client.id),
        name: String(client.display_name ?? ""),
        email: client.email ? String(client.email) : null,
        role: "BUYER",
      })
    }

    const participantRows = await q`
      select fp.role, fp.person_id, fp.display_name,
        (
          select pi.identity_value
          from person_identity pi
          where pi.person_id = fp.person_id
            and pi.identity_type = 'email'
          order by pi.is_primary desc, pi.created_at asc
          limit 1
        ) as email
      from document_form_participant fp
      where fp.form_instance_id = ${formId}
      order by fp.sort_order asc, fp.display_name
    `
    for (const row of participantRows) {
      people.push({
        personId: row.person_id ? String(row.person_id) : null,
        name: String(row.display_name ?? ""),
        email: row.email ? String(row.email) : null,
        role: String(row.role ?? "OTHER"),
      })
    }

    const dealRows = await q`
      select dp.role, dp.person_id, coalesce(person.display_name, dp.role) as display_name,
        (
          select pi.identity_value
          from person_identity pi
          where pi.person_id = dp.person_id
            and pi.identity_type = 'email'
          order by pi.is_primary desc, pi.created_at asc
          limit 1
        ) as email
      from deal_participant dp
      left join person on person.id = dp.person_id
      where dp.deal_id = ${formRow.deal_id}
        and dp.active = true
      order by dp.created_at asc
    `
    for (const row of dealRows) {
      people.push({
        personId: row.person_id ? String(row.person_id) : null,
        name: String(row.display_name ?? ""),
        email: row.email ? String(row.email) : null,
        role: ROLE_MAP[String(row.role)] ?? String(row.role ?? "OTHER"),
      })
    }
  }

  return people
}
