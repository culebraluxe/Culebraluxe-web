//! Moved from `client.rs` (move only): DirectoryRow, AdminRow, DetailBaseRow, PropertyInterestRow, CachedPropertyInterestRow, InteractionRow, CachedInteractionRow, EvidenceRow, HistoryRow, ClientDao, ClientReadCache.

#[allow(unused_imports)]
use super::*;

#[derive(Debug, Clone, FromRow)]
pub(super) struct DirectoryRow {
    pub(super) person_id: String,
    pub(super) display_name: String,
    pub(super) role: String,
    pub(super) status: String,
    pub(super) location: Option<String>,
    pub(super) primary_email: Option<String>,
    pub(super) primary_phone: Option<String>,
    pub(super) assigned_agent: Option<String>,
    pub(super) last_contact_label: Option<String>,
    pub(super) sources: Vec<String>,
    pub(super) name_sort_priority: i32,
    /// count(*) over () - the filtered total, delivered with the rows so the page costs one round trip.
    pub(super) total: i64,
}

#[derive(Debug, FromRow)]
pub(super) struct AdminRow {
    pub(super) id: String,
    pub(super) display_name: String,
    pub(super) role: String,
    pub(super) status: String,
    pub(super) location: Option<String>,
    pub(super) assigned_agent: Option<String>,
    pub(super) primary_email: Option<String>,
    pub(super) primary_phone: Option<String>,
    pub(super) last_interaction_label: Option<String>,
    pub(super) open_task_count: i64,
    pub(super) active_deal_count: i64,
    pub(super) interest_count: i64,
}

#[derive(Debug, FromRow)]
pub(super) struct DetailBaseRow {
    pub(super) id: String,
    pub(super) display_name: String,
    pub(super) role: String,
    pub(super) status: String,
    pub(super) location: Option<String>,
    pub(super) budget_min: Option<String>,
    pub(super) budget_max: Option<String>,
    pub(super) preferred_areas: Option<Vec<String>>,
    pub(super) property_types: Option<Vec<String>>,
    pub(super) priorities: Option<Vec<String>>,
    pub(super) timeline: Option<String>,
    pub(super) notes: Option<String>,
    pub(super) assigned_user_name: Option<String>,
    pub(super) assigned_user_id: Option<String>,
    pub(super) email: Option<String>,
    pub(super) phone: Option<String>,
    pub(super) last_contact_channel: Option<String>,
    pub(super) last_contact_at: Option<String>,
    pub(super) last_contact_summary: Option<String>,
    pub(super) next_action_title: Option<String>,
    pub(super) next_action_at: Option<String>,
    pub(super) next_action_detail: Option<String>,
}

#[derive(Debug, FromRow)]
pub(super) struct PropertyInterestRow {
    pub(super) id: String,
    pub(super) property_id: String,
    pub(super) property_name: String,
    pub(super) location: Option<String>,
    pub(super) price: Option<String>,
    pub(super) bedrooms: Option<String>,
    pub(super) property_type: Option<String>,
    pub(super) status: String,
    pub(super) hero_media_id: Option<String>,
}

#[derive(Debug, FromRow)]
pub(super) struct CachedPropertyInterestRow {
    pub(super) person_id: String,
    pub(super) id: String,
    pub(super) property_id: String,
    pub(super) property_name: String,
    pub(super) location: Option<String>,
    pub(super) price: Option<String>,
    pub(super) bedrooms: Option<String>,
    pub(super) property_type: Option<String>,
    pub(super) status: String,
    pub(super) hero_media_id: Option<String>,
}

#[derive(Debug, FromRow)]
pub(super) struct InteractionRow {
    pub(super) id: String,
    pub(super) channel: String,
    pub(super) event_type: String,
    pub(super) direction: Option<String>,
    pub(super) occurred_at: String,
    pub(super) title: Option<String>,
    pub(super) summary: Option<String>,
    pub(super) duration_seconds: Option<i64>,
    pub(super) source_metadata: Option<Value>,
}

#[derive(Debug, FromRow)]
pub(super) struct CachedInteractionRow {
    pub(super) person_id: String,
    pub(super) id: String,
    pub(super) channel: String,
    pub(super) event_type: String,
    pub(super) direction: Option<String>,
    pub(super) occurred_at: String,
    pub(super) title: Option<String>,
    pub(super) summary: Option<String>,
    pub(super) duration_seconds: Option<i64>,
    pub(super) source_metadata: Option<Value>,
}

#[derive(Debug, Clone, FromRow)]
pub(super) struct EvidenceRow {
    pub(super) canonical_person_id: String,
    pub(super) source: String,
    pub(super) first_observed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub(super) last_observed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub(super) last_inbound_at: Option<chrono::DateTime<chrono::Utc>>,
    pub(super) last_outbound_at: Option<chrono::DateTime<chrono::Utc>>,
    pub(super) inbound_count: Option<i32>,
    pub(super) outbound_count: Option<i32>,
    pub(super) is_two_way: Option<bool>,
    pub(super) is_automated_or_bulk: Option<bool>,
    pub(super) is_organization_or_service: Option<bool>,
    pub(super) has_email: Option<bool>,
    pub(super) has_phone: Option<bool>,
    pub(super) coverage_note: Option<String>,
}

#[derive(Debug, FromRow)]
pub(super) struct HistoryRow {
    pub(super) interaction_id: String,
    pub(super) channel: String,
    pub(super) direction: Option<String>,
    pub(super) occurred_at: chrono::DateTime<chrono::Utc>,
    pub(super) title: Option<String>,
    pub(super) summary: Option<String>,
}

#[derive(Clone)]
pub struct ClientDao {
    pub(super) db: Database,
    pub(super) read_cache: Arc<RwLock<Option<ClientReadCache>>>,
}

#[derive(Clone, Default)]
pub(super) struct ClientReadCache {
    pub(super) directory: Vec<ClientDirectoryRecord>,
    pub(super) evidence: HashMap<String, Vec<RelationshipEvidenceRecord>>,
    pub(super) details: HashMap<String, ClientDetail>,
}
