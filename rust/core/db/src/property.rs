use crate::{Database, DbFailure, DbResult, DbTransaction};
use chrono::{DateTime, Utc};
use domain::{
    CreatePropertyAdminRequest, FieldPatch, FindPropertyByAddressRequest, PersonPropertyContext,
    PersonPropertyRelation, Property, PropertyAddress, PropertyAddressPatch, PropertyAdminPage,
    PropertyAdminPageRequest, PropertyAdminRecord, PropertyAdminSummary, PropertyForPerson,
    PropertyStellarDetails, SavePropertyAdminRequest, SetPropertyDisplayNameRequest,
    SetPropertyListingTypeRequest, SetPropertyStatusRequest, UpsertPropertyForPersonRequest,
};
use sqlx::{FromRow, PgConnection};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

macro_rules! property_sql {
    ($prefix:literal, $suffix:literal) => {
        concat!(
            $prefix,
            "p.id::text as id, p.name, p.legal_owner_name, p.catastro_number, ",
            "p.registry_entry, p.finca_number, p.registry_section, ",
            "p.status, p.archived_at, p.address_line1, p.location, ",
            "p.street_number, p.street_name, p.unit_number, p.city, p.state_or_province, ",
            "p.neighborhood, p.postal_code, p.country, p.iso_country_code",
            $suffix
        )
    };
}
mod compact;
mod database;
mod merge_parcel_record;
mod property_row;
mod upsert_for_person_tx;
#[allow(unused_imports)]
pub use compact::*;
#[allow(unused_imports)]
pub use database::*;
#[allow(unused_imports)]
pub use merge_parcel_record::*;
#[allow(unused_imports)]
pub use property_row::*;
#[allow(unused_imports)]
pub use upsert_for_person_tx::*;
