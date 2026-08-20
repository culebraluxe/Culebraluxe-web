import { sql } from './client'
import { getOpsCounts } from './ops-counts'

// Read-only Portal system-health projection (OPS-01). Exposes operational
// counts and data-quality signals derivable from the existing schema. This is
// not a generic DB admin tool: no table editing, no secrets, no writes.
//
// AUTH checks are run conditionally after probing for the AUTH tables, because
// Postgres resolves table names at parse time (to_regclass inside one query
// cannot guard a subquery referencing a not-yet-created table).

export type SystemHealthSnapshot = {
  unresolvedIntakeCount: number
  openTaskCount: number
  overdueTaskCount: number
  activeDealCount: number
  underContractCount: number
  activePropertyCount: number
  recentInteractionAtLabel: string | null
  interactionsLast7Days: number
  // Data-quality signals
  personsWithoutEmailIdentity: number
  personsWithoutPhoneIdentity: number
  openTasksWithoutDueDate: number
  activePropertiesWithoutHeroMedia: number
  completedShowingsMissingCompletedAt: number
  scheduledShowingsMissingScheduledAt: number
  activeParticipantsWithEndedAt: number
  otherParticipantsMissingRoleLabel: number
  offersWithCrossDealParent: number
  showingsWithDealPropertyMismatch: number
  // Write-side invariants relevant to current listing/showing/participant writes
  completedShowingsMissingShowingInteraction: number
  inactiveParticipantsWithoutEndedAt: number
  publicPropertiesWithMultipleHeroes: number
  heroMediaNotImage: number
  // AUTH-01 safety net: internal/external account-type mismatches in role
  // assignments (0 expected; the trigger enforces this).
  accountTypeMismatchCount: number
  // AUTH-02 security health
  activeAppUsersWithoutRole: number
  authIdentityInactiveAppUser: number
  ownerAssignments: number
  multipleOwners: number
  authIdentityWithoutUsableAppUser: number
}

export async function getSystemHealth(): Promise<SystemHealthSnapshot> {
  const [counts, deals, recent, quality, authTables] = await Promise.all([
    getOpsCounts(),
    sql`
      select count(*)::int as under_contract_count
      from deal
      where stage = 'under_contract'
    `,
    sql`
      select
        to_char(
          max(occurred_at) at time zone 'America/Puerto_Rico',
          'Mon FMDD, YYYY HH12:MI AM'
        ) as recent_label,
        (select count(*)::int
          from interaction
          where occurred_at >= now() - interval '7 days') as last_7_days
      from interaction
    `,
    sql`
      select
        (
          select count(*)::int
          from person p
          where p.archived_at is null
            and not exists (
              select 1 from person_identity pi
              where pi.person_id = p.id and pi.identity_type = 'email'
            )
        ) as persons_without_email,
        (
          select count(*)::int
          from person p
          where p.archived_at is null
            and not exists (
              select 1 from person_identity pi
              where pi.person_id = p.id and pi.identity_type = 'phone'
            )
        ) as persons_without_phone,
        (
          select count(*)::int
          from task
          where status = 'open' and due_at is null
        ) as open_tasks_without_due,
        (
          select count(*)::int
          from property p
          where p.archived_at is null
            and p.status in ('active', 'coming_soon', 'under_contract')
            and not exists (
              select 1 from property_media pm
              where pm.property_id = p.id and pm.role = 'hero'
            )
        ) as active_properties_without_hero,
        (
          select count(*)::int
          from showing
          where status = 'completed' and completed_at is null
        ) as completed_showings_missing_completed_at,
        (
          select count(*)::int
          from showing
          where status = 'scheduled' and scheduled_at is null
        ) as scheduled_showings_missing_scheduled_at,
        (
          select count(*)::int
          from deal_participant
          where active = true and ended_at is not null
        ) as active_participants_with_ended_at,
        (
          select count(*)::int
          from deal_participant
          where role = 'other' and role_label is null
        ) as other_participants_missing_role_label,
        (
          select count(*)::int
          from offer o
          join offer parent
            on parent.id = o.parent_offer_id
          where parent.deal_id <> o.deal_id
        ) as offers_with_cross_deal_parent,
        (
          select count(*)::int
          from showing s
          join deal d
            on d.id = s.deal_id
          where s.property_id is not null
            and s.property_id <> d.property_id
        ) as showings_with_deal_property_mismatch,
        (
          select count(*)::int
          from showing s
          where s.status = 'completed'
            and not exists (
              select 1 from interaction i
              where i.source_system = 'showing'
                and i.source_external_id = s.id::text
            )
        ) as completed_showings_missing_showing_interaction,
        (
          select count(*)::int
          from deal_participant
          where active = false and ended_at is null
        ) as inactive_participants_without_ended_at,
        (
          select count(*)::int
          from (
            select pm.property_id
            from property_media pm
            where pm.role = 'hero'
            group by pm.property_id
            having count(*) > 1
          ) multi_hero
          join property p
            on p.id = multi_hero.property_id
          where p.archived_at is null
        ) as public_properties_with_multiple_heroes,
        (
          select count(*)::int
          from property_media pm
          join media m
            on m.id = pm.media_id
          where pm.role = 'hero'
            and m.media_type <> 'image'
        ) as hero_media_not_image
    `,
    sql`
      select
        to_regclass('app_user_role') is not null as has_roles,
        to_regclass('auth_identity') is not null as has_identities
    `,
  ])

  const dealRow = deals[0] as { under_contract_count: number } | undefined
  const recentRow = recent[0] as
    | { recent_label: string | null; last_7_days: number }
    | undefined
  const qualityRow = quality[0] as
    | {
        persons_without_email: number
        persons_without_phone: number
        open_tasks_without_due: number
        active_properties_without_hero: number
        completed_showings_missing_completed_at: number
        scheduled_showings_missing_scheduled_at: number
        active_participants_with_ended_at: number
        other_participants_missing_role_label: number
        offers_with_cross_deal_parent: number
        showings_with_deal_property_mismatch: number
        completed_showings_missing_showing_interaction: number
        inactive_participants_without_ended_at: number
        public_properties_with_multiple_heroes: number
        hero_media_not_image: number
      }
    | undefined

  const authTableRow = authTables[0] as
    | { has_roles: boolean; has_identities: boolean }
    | undefined
  const hasRoles = authTableRow?.has_roles ?? false
  const hasIdentities = authTableRow?.has_identities ?? false

  let accountTypeMismatchCount = 0
  let activeAppUsersWithoutRole = 0
  let ownerAssignments = 0
  let multipleOwners = 0
  let authIdentityInactiveAppUser = 0
  let authIdentityWithoutUsableAppUser = 0

  if (hasRoles) {
    const roleRows = await sql`
      select
        (
          select count(*)::int
          from app_user_role aur
          join role r on r.id = aur.role_id
          join app_user u on u.id = aur.app_user_id
          where r.account_type <> u.account_type
        ) as account_type_mismatch_count,
        (
          select count(*)::int
          from app_user u
          where u.active = true
            and not exists (
              select 1 from app_user_role aur where aur.app_user_id = u.id
            )
        ) as active_app_users_without_role,
        (
          select count(*)::int
          from app_user_role aur
          join role r on r.id = aur.role_id
          where r.code = 'owner'
        ) as owner_assignments
    `
    const roleRow = roleRows[0] as
      | {
          account_type_mismatch_count: number
          active_app_users_without_role: number
          owner_assignments: number
        }
      | undefined
    accountTypeMismatchCount = roleRow?.account_type_mismatch_count ?? 0
    activeAppUsersWithoutRole = roleRow?.active_app_users_without_role ?? 0
    ownerAssignments = roleRow?.owner_assignments ?? 0
    multipleOwners = ownerAssignments > 1 ? 1 : 0
  }

  if (hasIdentities) {
    const identityRows = await sql`
      select
        (
          select count(*)::int
          from auth_identity ai
          join app_user u on u.id = ai.app_user_id
          where u.active = false
        ) as auth_identity_inactive_app_user,
        (
          select count(*)::int
          from auth_identity ai
          where not exists (
            select 1 from app_user u
            where u.id = ai.app_user_id and u.active = true
          )
        ) as auth_identity_without_usable_app_user
    `
    const identityRow = identityRows[0] as
      | {
          auth_identity_inactive_app_user: number
          auth_identity_without_usable_app_user: number
        }
      | undefined
    authIdentityInactiveAppUser =
      identityRow?.auth_identity_inactive_app_user ?? 0
    authIdentityWithoutUsableAppUser =
      identityRow?.auth_identity_without_usable_app_user ?? 0
  }

  return {
    unresolvedIntakeCount: counts.unresolvedIntakeCount,
    openTaskCount: counts.openTaskCount,
    overdueTaskCount: counts.overdueTaskCount,
    activeDealCount: counts.activeDealCount,
    underContractCount: dealRow?.under_contract_count ?? 0,
    activePropertyCount: counts.activePropertyCount,
    recentInteractionAtLabel: recentRow?.recent_label ?? null,
    interactionsLast7Days: recentRow?.last_7_days ?? 0,
    personsWithoutEmailIdentity: qualityRow?.persons_without_email ?? 0,
    personsWithoutPhoneIdentity: qualityRow?.persons_without_phone ?? 0,
    openTasksWithoutDueDate: qualityRow?.open_tasks_without_due ?? 0,
    activePropertiesWithoutHeroMedia:
      qualityRow?.active_properties_without_hero ?? 0,
    completedShowingsMissingCompletedAt:
      qualityRow?.completed_showings_missing_completed_at ?? 0,
    scheduledShowingsMissingScheduledAt:
      qualityRow?.scheduled_showings_missing_scheduled_at ?? 0,
    activeParticipantsWithEndedAt:
      qualityRow?.active_participants_with_ended_at ?? 0,
    otherParticipantsMissingRoleLabel:
      qualityRow?.other_participants_missing_role_label ?? 0,
    offersWithCrossDealParent: qualityRow?.offers_with_cross_deal_parent ?? 0,
    showingsWithDealPropertyMismatch:
      qualityRow?.showings_with_deal_property_mismatch ?? 0,
    completedShowingsMissingShowingInteraction:
      qualityRow?.completed_showings_missing_showing_interaction ?? 0,
    inactiveParticipantsWithoutEndedAt:
      qualityRow?.inactive_participants_without_ended_at ?? 0,
    publicPropertiesWithMultipleHeroes:
      qualityRow?.public_properties_with_multiple_heroes ?? 0,
    heroMediaNotImage: qualityRow?.hero_media_not_image ?? 0,
    accountTypeMismatchCount,
    activeAppUsersWithoutRole,
    authIdentityInactiveAppUser,
    ownerAssignments,
    multipleOwners,
    authIdentityWithoutUsableAppUser,
  }
}
