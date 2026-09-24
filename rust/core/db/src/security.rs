use crate::{Database, DbFailure, DbResult};
use domain::{
    security::{canonical_primary_role, RoleEntitlements, SecurityUserRoles},
    ActingUser,
};
use sqlx::FromRow;

#[derive(Debug, FromRow)]
struct PrincipalRow {
    app_user_id: String,
    display_name: String,
    email: Option<String>,
    account_type: String,
    person_id: Option<String>,
    role_codes: Vec<String>,
    authority_codes: Vec<String>,
    entitlement_codes: Vec<String>,
}

#[derive(Debug, FromRow)]
struct RoleEntitlementRow {
    role_code: String,
    account_type: String,
    entitlement_codes: Vec<String>,
}

#[derive(Debug, FromRow)]
struct SecurityUserRoleRow {
    app_user_id: String,
    display_name: String,
    email: Option<String>,
    account_type: String,
    active: bool,
    role_codes: Vec<String>,
}

#[derive(Clone)]
pub struct SecurityDao {
    db: Database,
}

impl SecurityDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// One statement makes the grant change atomic and refuses unknown or inactive
    /// roles/actions. Account type is checked here as well as by the service port.
    pub async fn set_role_entitlement(
        &self,
        role_code: &str,
        action: &str,
        granted: bool,
    ) -> DbResult<bool> {
        sqlx::query_scalar::<_, bool>(
            r#"
            with target as (
                select r.id as role_id, e.id as entitlement_id
                from security_role r cross join entitlement e
                where r.code = $1 and r.active = true and r.account_type = 'internal'
                  and e.code = $2 and e.active = true
                  and (e.code not in ('security.entitlement.manage', 'security.role.manage') or r.code = 'root')
            ), added as (
                insert into role_entitlement (role_id, entitlement_id)
                select role_id, entitlement_id from target where $3
                on conflict do nothing returning 1
            ), removed as (
                delete from role_entitlement re using target
                where re.role_id = target.role_id and re.entitlement_id = target.entitlement_id
                  and not $3 returning 1
            )
            select exists(select 1 from target)
            "#,
        )
        .bind(role_code)
        .bind(action)
        .bind(granted)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("security.set_role_entitlement", &error))
    }

    /// Replace only the user's coarse security role, preserving any future specialist
    /// additive roles. The final active ROOT cannot be demoted in this statement.
    pub async fn set_user_primary_role(
        &self,
        app_user_id: &str,
        role_code: &str,
    ) -> DbResult<bool> {
        sqlx::query_scalar::<_, bool>(
            r#"
            with target_user as (
                select u.id
                from app_user u
                where u.id = $1::uuid
                  and u.account_type = 'internal'
                  and u.active = true
            ), target_role as (
                select r.id, r.code
                from security_role r
                where r.code = $2
                  and r.account_type = 'internal'
                  and r.active = true
                  and r.code in (
                    'internal_guest',
                    'user',
                    'business_power_user',
                    'owner',
                    'root'
                  )
            ), root_holders as (
                select count(distinct aur.app_user_id)::bigint as count
                from app_user_role aur
                join app_user u
                  on u.id = aur.app_user_id
                 and u.account_type = 'internal'
                 and u.active = true
                join security_role r
                  on r.id = aur.role_id
                 and r.active = true
                 and r.code = 'root'
            ), safe_target as (
                select tu.id as user_id, tr.id as role_id, tr.code
                from target_user tu
                cross join target_role tr
                cross join root_holders rh
                where not (
                    tr.code <> 'root'
                    and rh.count <= 1
                    and exists (
                        select 1
                        from app_user_role aur
                        join security_role current_role
                          on current_role.id = aur.role_id
                         and current_role.active = true
                         and current_role.code = 'root'
                        where aur.app_user_id = tu.id
                    )
                )
            ), removed as (
                delete from app_user_role aur
                using security_role current_role, safe_target st
                where aur.app_user_id = st.user_id
                  and aur.role_id = current_role.id
                  and aur.role_id <> st.role_id
                  and current_role.account_type = 'internal'
                  and current_role.code in (
                    'internal_guest',
                    'guest',
                    'user',
                    'ops',
                    'viewer',
                    'business_power',
                    'business_power_user',
                    'bus_power_user',
                    'agent',
                    'owner',
                    'root'
                  )
                returning 1
            ), added as (
                insert into app_user_role (app_user_id, role_id)
                select user_id, role_id
                from safe_target
                on conflict do nothing
                returning 1
            )
            select exists(select 1 from safe_target)
            "#,
        )
        .bind(app_user_id)
        .bind(role_code)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("security.set_user_primary_role", &error))
    }

    pub async fn list_security_users(&self) -> DbResult<Vec<SecurityUserRoles>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, SecurityUserRoleRow>(
                r#"
                select
                  u.id::text as app_user_id,
                  u.display_name,
                  u.email,
                  u.account_type,
                  u.active,
                  array(
                    select distinct r.code
                    from app_user_role aur
                    join security_role r
                      on r.id = aur.role_id
                     and r.active = true
                    where aur.app_user_id = u.id
                    order by r.code
                  ) as role_codes
                from app_user u
                where u.account_type = 'internal'
                order by u.active desc, u.display_name asc, u.id
                "#,
            )
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("security.list_security_users", &error))
        })?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let primary_role_code = canonical_primary_role(&row.role_codes).map(str::to_owned);
                SecurityUserRoles {
                    app_user_id: row.app_user_id,
                    display_name: row.display_name,
                    email: row.email,
                    account_type: row.account_type,
                    active: row.active,
                    role_codes: row.role_codes,
                    primary_role_code,
                }
            })
            .collect())
    }

    pub async fn list_role_entitlements(&self) -> DbResult<Vec<RoleEntitlements>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, RoleEntitlementRow>(
                r#"
                select r.code as role_code, r.account_type,
                  coalesce(array_agg(e.code order by e.code) filter (where e.code is not null), '{}'::text[]) as entitlement_codes
                from security_role r
                left join role_entitlement re on re.role_id = r.id
                left join entitlement e on e.id = re.entitlement_id and e.active = true
                where r.active = true
                group by r.id
                order by r.code
                "#,
            )
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("security.list_role_entitlements", &error))
        })?;
        Ok(rows
            .into_iter()
            .map(|row| RoleEntitlements {
                role_code: row.role_code,
                account_type: row.account_type,
                entitlement_codes: row.entitlement_codes,
            })
            .collect())
    }

    pub async fn resolve_provider_subject(
        &self,
        provider: &str,
        subject: &str,
    ) -> DbResult<Option<String>> {
        crate::retrying_read!(async {
            sqlx::query_scalar::<_, String>(
                r#"
                select app_user_id::text
                from auth_identity
                where provider = $1 and provider_subject = $2
                limit 1
                "#,
            )
            .bind(provider)
            .bind(subject)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("security.resolve_provider_subject", &error))
        })
    }

    pub async fn get_principal(&self, app_user_id: &str) -> DbResult<Option<ActingUser>> {
        let row = crate::retrying_read!(async {
            sqlx::query_as::<_, PrincipalRow>(
                r#"
                select
                  u.id::text as app_user_id,
                  u.display_name,
                  u.email,
                  u.account_type,
                  u.person_id::text as person_id,
                  array(
                    select distinct r.code
                    from app_user_role aur
                    join security_role r on r.id = aur.role_id and r.active = true
                    where aur.app_user_id = u.id
                    order by r.code
                  ) as role_codes,
                  array(
                    select distinct a.code
                    from app_user_role aur
                    join security_role r on r.id = aur.role_id and r.active = true
                    join role_authority ra on ra.role_id = r.id
                    join authority a on a.id = ra.authority_id
                    where aur.app_user_id = u.id
                    order by a.code
                  ) as authority_codes,
                  array(
                    select distinct e.code
                    from app_user_role aur
                    join security_role r on r.id = aur.role_id and r.active = true
                    join role_entitlement re on re.role_id = r.id
                    join entitlement e on e.id = re.entitlement_id
                    where aur.app_user_id = u.id and e.active = true
                    order by e.code
                  ) as entitlement_codes
                from app_user u
                where u.id = $1::uuid and u.active = true
                limit 1
                "#,
            )
            .bind(app_user_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("security.get_principal", &error))
        })?;

        Ok(row.map(|row| ActingUser {
            app_user_id: row.app_user_id,
            display_name: row.display_name,
            email: row.email,
            account_type: row.account_type,
            role_codes: row.role_codes,
            authority_codes: row.authority_codes,
            entitlement_codes: row.entitlement_codes,
            person_id: row.person_id,
        }))
    }
}
