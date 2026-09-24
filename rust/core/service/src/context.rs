use serde::{Deserialize, Serialize};

/// The actor the identity-resolution policy reserves for the Auth.js edge.
///
/// NAMED ONCE, because two layers compare it: the bootstrap rule that admits
/// `security.identity.resolve` and the login seam that asks for it. A typo in either string is a silent
/// denial at login, and a second literal is how the two get to disagree.
pub const AUTHJS_EDGE_ACTOR: &str = "authjs-edge";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceActorKind {
    User,
    System,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceActor {
    pub id: Option<String>,
    pub kind: ServiceActorKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServicePrincipal {
    pub app_user_id: String,
    pub level: String,
    pub role_codes: Vec<String>,
    #[serde(default)]
    pub account_type: String,
    #[serde(default)]
    pub entitlement_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceContext {
    pub actor: ServiceActor,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub principal: Option<ServicePrincipal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationKind {
    Query,
    Command,
}
