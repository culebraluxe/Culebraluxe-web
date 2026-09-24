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
                "guest" | "client" => SecurityLevel::Guest,
                _ => SecurityLevel::Guest,
            };
            current.max(candidate)
        })
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
}
