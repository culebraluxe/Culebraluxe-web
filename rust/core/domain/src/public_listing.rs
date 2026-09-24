//! The public site's listing copy: the one-line pitch a published listing carries.

use serde::{Deserialize, Serialize};

/// One published listing's tagline, keyed by the slug the public pages already carry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicListingCopy {
    pub slug: String,
    pub tagline: String,
}
