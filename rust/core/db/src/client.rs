use crate::{Database, DbFailure, DbResult};
use domain::{
    AssignableAgent, ClientAdminPageRequest, ClientAdminRow, ClientDetail,
    ClientDirectoryPageRequest, ClientDirectoryRecord, ClientHistoryEventRecord, ClientInteraction,
    ClientLastContact, ClientNextAction, ClientPropertyInterest, Person,
    RelationshipEvidenceRecord,
};
use serde_json::{json, Value};
use sqlx::FromRow;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
mod detail;
mod directory_row;
mod warm_read_cache;
#[allow(unused_imports)]
pub use detail::*;
#[allow(unused_imports)]
pub use directory_row::*;
#[allow(unused_imports)]
pub use warm_read_cache::*;
