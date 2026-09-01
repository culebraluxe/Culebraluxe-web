import { neon } from '@neondatabase/serverless'

const envArgIndex = process.argv.indexOf('--env')
const environment = envArgIndex >= 0 ? process.argv[envArgIndex + 1] : 'prod'
if (environment !== 'prod') {
  console.error('This command audits Production only. Use --env prod.')
  process.exit(2)
}

const prodUrl = process.env.DATABASE_URL_PROD
if (!prodUrl) {
  console.error('DATABASE_URL_PROD is not configured in .env.local.')
  process.exit(2)
}
if (process.env.DATABASE_URL_DEV && process.env.DATABASE_URL_DEV === prodUrl) {
  console.error('DATABASE_URL_PROD must not equal DATABASE_URL_DEV.')
  process.exit(2)
}

const sql = neon(prodUrl)

const rows = await sql`
  with active_phone as (
    select
      pi.id as identity_id,
      pi.person_id,
      p.display_name,
      pi.identity_value,
      pi.is_primary,
      regexp_replace(pi.identity_value, '[^0-9]', '', 'g') as digits
    from person_identity pi
    join person p on p.id = pi.person_id
    where pi.identity_type = 'phone'
      and p.archived_at is null
  ),
  keyed as (
    select
      *,
      case
        when length(digits) = 10 then digits
        when length(digits) = 11 and left(digits, 1) = '1'
          then substring(digits from 2)
        else null
      end as nanp_key
    from active_phone
  )
  select
    nanp_key,
    count(*)::int as identity_count,
    count(distinct person_id)::int as person_count,
    json_agg(
      json_build_object(
        'identityId', identity_id,
        'personId', person_id,
        'displayName', display_name,
        'identityValue', identity_value,
        'isPrimary', is_primary,
        'canonicalE164', case when nanp_key is not null then '+1' || nanp_key else null end
      )
      order by display_name, person_id, identity_id
    ) as owners
  from keyed
  where nanp_key is not null
  group by nanp_key
  order by nanp_key
`

type Owner = {
  identityId: string
  personId: string
  displayName: string
  identityValue: string
  isPrimary: boolean
  canonicalE164: string | null
}

type Group = {
  nanp_key: string
  identity_count: number
  person_count: number
  owners: Owner[]
}

const groups = rows as unknown as Group[]
const samePersonDuplicates = groups.filter(
  (group) =>
    group.identity_count > group.person_count &&
    new Set(group.owners.map((owner) => owner.personId)).size < group.identity_count,
)
const crossPersonConflicts = groups.filter((group) => group.person_count > 1)
const safeNormalizations = groups.flatMap((group) =>
  group.owners.filter(
    (owner) => owner.canonicalE164 && owner.identityValue !== owner.canonicalE164,
  ),
)

console.log(JSON.stringify({
  env: 'prod',
  mode: 'read-only',
  summary: {
    activeNanpIdentityGroups: groups.length,
    activeNanpIdentityRows: groups.reduce((sum, group) => sum + group.identity_count, 0),
    safeNormalizationRows: safeNormalizations.length,
    samePersonDuplicateGroups: samePersonDuplicates.length,
    crossPersonConflictGroups: crossPersonConflicts.length,
  },
  crossPersonConflicts,
  samePersonDuplicates,
}, null, 2))
