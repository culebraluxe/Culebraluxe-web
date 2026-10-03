//! A website lead, as the notification emails need it.

use serde::{Deserialize, Serialize};

/// One accepted website enquiry: who, what they asked for, and the property it is about, if any.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteLead {
    pub id: String,
    /// `private_viewing`, `property_information` or `general_enquiry`.
    pub request_type: String,
    pub display_name: String,
    pub email: String,
    pub message: Option<String>,
    pub property_name: Option<String>,
    pub property_slug: Option<String>,
}

/// What a notify request did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebsiteLeadNotice {
    /// Both emails went out.
    Sent,
    /// Nothing to do: no such recent lead, or it was already notified.
    AlreadyHandled,
}
