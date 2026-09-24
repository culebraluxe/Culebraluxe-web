use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SecurityLevel {
    Guest,
    User,
    BusinessPowerUser,
    Root,
}

impl SecurityLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Guest => "GUEST",
            Self::User => "USER",
            Self::BusinessPowerUser => "BUSINESS_POWER_USER",
            Self::Root => "ROOT",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActingUser {
    pub app_user_id: String,
    pub display_name: String,
    pub email: Option<String>,
    pub account_type: String,
    pub role_codes: Vec<String>,
    pub authority_codes: Vec<String>,
    #[serde(default)]
    pub entitlement_codes: Vec<String>,
    pub person_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityPrincipal {
    pub acting_user: ActingUser,
    pub level: SecurityLevel,
}

/// Effective action grants for one named role; no account or identity secrets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleEntitlements {
    pub role_code: String,
    pub account_type: String,
    pub entitlement_codes: Vec<String>,
}

/// One internal application user as shown on the ROOT security-administration screen.
/// Role codes remain visible for diagnosis, while primary_role_code normalizes the
/// legacy coarse-role aliases to the canonical assignment vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityUserRoles {
    pub app_user_id: String,
    pub display_name: String,
    pub email: Option<String>,
    pub account_type: String,
    pub active: bool,
    pub role_codes: Vec<String>,
    pub primary_role_code: Option<String>,
}

/// The five assignable internal roles. External guest/client access is a separate
/// account type and is never assigned through the internal-user administration UI.
pub const CANONICAL_INTERNAL_ROLE_CODES: &[&str] = &[
    "internal_guest",
    "user",
    "business_power_user",
    "owner",
    "root",
];

pub fn canonical_primary_role(role_codes: &[String]) -> Option<&'static str> {
    if role_codes.iter().any(|role| role == "root") {
        Some("root")
    } else if role_codes.iter().any(|role| role == "owner") {
        Some("owner")
    } else if role_codes.iter().any(|role| {
        matches!(
            role.as_str(),
            "business_power" | "business_power_user" | "bus_power_user" | "agent"
        )
    }) {
        Some("business_power_user")
    } else if role_codes
        .iter()
        .any(|role| matches!(role.as_str(), "user" | "ops" | "viewer"))
    {
        Some("user")
    } else if role_codes
        .iter()
        .any(|role| matches!(role.as_str(), "internal_guest" | "guest"))
    {
        Some("internal_guest")
    } else {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityIdentityResolution {
    Known(SecurityPrincipal),
    Unmapped,
    Inactive,
}

pub fn resolve_security_level(role_codes: &[String]) -> SecurityLevel {
    role_codes
        .iter()
        .fold(SecurityLevel::Guest, |current, role| {
            let candidate = match role.trim().to_lowercase().as_str() {
                "root" | "owner" => SecurityLevel::Root,
                "business_power" | "business_power_user" | "bus_power_user" | "agent" => {
                    SecurityLevel::BusinessPowerUser
                }
                "user" | "ops" | "viewer" => SecurityLevel::User,
                "guest" | "internal_guest" | "client" => SecurityLevel::Guest,
                _ => SecurityLevel::Guest,
            };
            current.max(candidate)
        })
}

/// The sign-in providers an EXTERNAL GUEST may use: Google, or a code emailed to them (subject = the email).
pub const GUEST_PROVIDERS: &[&str] = &["google", "email-code"];

/// Who a guest sign-in says it is, for provisioning an external `guest` user on first sight.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestClaim {
    pub provider: String,
    pub subject: String,
    pub email: Option<String>,
    /// Only a verified email may link this sign-in to an existing external user.
    #[serde(default)]
    pub email_verified: bool,
    pub display_name: Option<String>,
}

/// The rate limits' view of recent guest codes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GuestCodeHistory {
    pub email_last_hour: i64,
    pub ip_last_hour: i64,
    pub seconds_since_last_for_email: Option<i64>,
}

/// An address's one live code, as a verification attempt sees it (this attempt already counted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestCodeAttempt {
    pub id: String,
    pub code_hash: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_roles_fail_closed() {
        assert_eq!(
            resolve_security_level(&["something-new".into()]),
            SecurityLevel::Guest
        );
    }

    #[test]
    fn highest_role_wins() {
        assert_eq!(
            resolve_security_level(&["viewer".into(), "agent".into()]),
            SecurityLevel::BusinessPowerUser
        );
    }

    #[test]
    fn canonical_internal_guest_is_an_explicit_guest_role() {
        assert_eq!(
            resolve_security_level(&["internal_guest".into()]),
            SecurityLevel::Guest
        );
        assert_eq!(
            canonical_primary_role(&["internal_guest".into()]),
            Some("internal_guest")
        );
    }

    #[test]
    fn legacy_role_aliases_normalize_only_at_the_primary_role_projection() {
        assert_eq!(
            canonical_primary_role(&["agent".into()]),
            Some("business_power_user")
        );
        assert_eq!(canonical_primary_role(&["ops".into()]), Some("user"));
    }
}
