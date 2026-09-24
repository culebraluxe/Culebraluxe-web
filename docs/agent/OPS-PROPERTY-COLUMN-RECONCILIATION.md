# OPS Property and Stellar column reconciliation

Source: live DEV `information_schema.columns` on 2026-09-24; actual `FieldSpec` controls and the Person picker in `rust/ui/src/yew_views/portal_ops.rs`.

Production currently has 120 `property` columns and 17 `property_stellar_listing` columns. Its Property table lacks the five columns in migration 214, eight in 215, and one in 216. This reconciliation targets the DEV schema and cannot run against production until those migrations are promoted with the corresponding Rust deployment.

| Table | Columns | Editable controls | Linked picker | Read-only columns | Unrepresented |
| --- | ---: | ---: | ---: | ---: | ---: |
| `property` | 134 | 79 | 1 | 54 | 0 |
| `property_stellar_listing` | 17 | 15 | 0 | 2 | 0 |

The Property count spans Property, Site, Legal, Website, and Sources tabs plus the Person picker. The MLS extension is counted separately. `archived_at` is represented by the Archived control. Read-only fields are displayed individually with their database column names; 42 Regrid keys come directly from the stored `property` row. System and provenance columns are not edited in this form; assignment and import processes own their writes. Empty values are shown as `—`. A missing MLS row has no persisted `property_id` or `updated_at` yet.

## Property (134 columns)

| Column | Type | OPS tab / control | Access |
| --- | --- | --- | --- |
| `id` | uuid | Sources / `id` | Read-only (system / provenance) |
| `name` | text | Property / `name` | Editable |
| `location` | text | Site / `location` | Editable |
| `status` | text | Property / `status` | Editable |
| `list_price` | numeric | Property / `listPrice` | Editable |
| `bedrooms` | numeric | Property / `bedrooms` | Editable |
| `bathrooms` | numeric | Property / `bathrooms` | Editable |
| `square_feet` | integer | Property / `squareFeet` | Editable |
| `property_type` | text | Property / `propertyType` | Editable |
| `listing_identifier` | text | Legal / `listingIdentifier` | Editable |
| `seller_person_id` | uuid | Person / `sellerPersonId` | Editable (person picker) |
| `listing_user_id` | uuid | Sources / `listing_user_id` | Read-only (system / provenance) |
| `archived_at` | timestamp with time zone | Legal / `archived` | Editable |
| `created_at` | timestamp with time zone | Sources / `created_at` | Read-only (system / provenance) |
| `updated_at` | timestamp with time zone | Sources / `updated_at` | Read-only (system / provenance) |
| `slug` | text | Website / `slug` | Editable |
| `original_list_price` | numeric | Property / `originalListPrice` | Editable |
| `featured` | boolean | Website / `featured` | Editable |
| `street_number` | text | Site / `streetNumber` | Editable |
| `street_name` | text | Site / `streetName` | Editable |
| `unit_number` | text | Site / `unitNumber` | Editable |
| `city` | text | Site / `city` | Editable |
| `state_or_province` | text | Site / `stateOrProvince` | Editable |
| `postal_code` | text | Site / `postalCode` | Editable |
| `neighborhood` | text | Site / `neighborhood` | Editable |
| `latitude` | numeric | Site / `latitude` | Editable |
| `longitude` | numeric | Site / `longitude` | Editable |
| `bathrooms_full` | integer | Property / `bathroomsFull` | Editable |
| `bathrooms_half` | integer | Property / `bathroomsHalf` | Editable |
| `lot_size` | numeric | Sources / `lot_size` | Read-only (legacy measurement) |
| `lot_size_units` | text | Sources / `lot_size_units` | Read-only (legacy measurement) |
| `year_built` | integer | Property / `yearBuilt` | Editable |
| `stories` | numeric | Property / `stories` | Editable |
| `parking_spaces` | integer | Property / `parkingSpaces` | Editable |
| `has_ocean_view` | boolean | Site / `hasOceanView` | Editable |
| `has_bay_view` | boolean | Site / `hasBayView` | Editable |
| `has_beach_view` | boolean | Site / `hasBeachView` | Editable |
| `has_harbor_view` | boolean | Site / `hasHarborView` | Editable |
| `has_island_view` | boolean | Site / `hasIslandView` | Editable |
| `has_mountain_view` | boolean | Site / `hasMountainView` | Editable |
| `has_sunrise_view` | boolean | Site / `hasSunriseView` | Editable |
| `has_sunset_view` | boolean | Site / `hasSunsetView` | Editable |
| `has_water_access` | boolean | Site / `hasWaterAccess` | Editable |
| `has_beach_access` | boolean | Site / `hasBeachAccess` | Editable |
| `has_pool` | boolean | Site / `hasPool` | Editable |
| `has_generator` | boolean | Site / `hasGenerator` | Editable |
| `has_solar` | boolean | Site / `hasSolar` | Editable |
| `is_furnished` | boolean | Site / `isFurnished` | Editable |
| `is_gated` | boolean | Site / `isGated` | Editable |
| `hero_title` | text | Website / `heroTitle` | Editable |
| `tagline` | text | Website / `tagline` | Editable |
| `short_description` | text | Website / `shortDescription` | Editable |
| `editorial_description` | text | Website / `editorialDescription` | Editable |
| `public_remarks` | text | Website / `publicRemarks` | Editable |
| `architecture_notes` | text | Website / `architectureNotes` | Editable |
| `amenities_notes` | text | Website / `amenitiesNotes` | Editable |
| `lifestyle_notes` | text | Website / `lifestyleNotes` | Editable |
| `listing_agent_name` | text | Legal / `listingAgentName` | Editable |
| `listing_agent_email` | text | Legal / `listingAgentEmail` | Editable |
| `listing_agent_phone` | text | Legal / `listingAgentPhone` | Editable |
| `listing_office` | text | Legal / `listingOffice` | Editable |
| `source_type` | text | Sources / `source_type` | Read-only (system / provenance) |
| `source_provider` | text | Sources / `source_provider` | Read-only (system / provenance) |
| `source_listing_key` | text | Sources / `source_listing_key` | Read-only (system / provenance) |
| `source_modified_at` | timestamp with time zone | Sources / `source_modified_at` | Read-only (system / provenance) |
| `last_synced_at` | timestamp with time zone | Sources / `last_synced_at` | Read-only (system / provenance) |
| `seo_title` | text | Website / `seoTitle` | Editable |
| `seo_description` | text | Website / `seoDescription` | Editable |
| `is_published` | boolean | Website / `isPublished` | Editable |
| `legal_owner_name` | text | Legal / `legalOwnerName` | Editable |
| `country` | text | Site / `country` | Editable |
| `iso_country_code` | text | Site / `isoCountryCode` | Editable |
| `address_line1` | text | Site / `addressLine1` | Editable |
| `registry_entry` | text | Site / `registryEntry` | Editable |
| `finca_number` | text | Site / `fincaNumber` | Editable |
| `registry_section` | text | Site / `registrySection` | Editable |
| `is_active_listing` | boolean | Website / `isActiveListing` | Editable |
| `catastro_source` | text | Sources / `catastro_source` | Read-only (system / provenance) |
| `regrid_address_source` | text | Sources / `regrid_address_source` | Read-only (Regrid) |
| `regrid_buyer_name` | text | Sources / `regrid_buyer_name` | Read-only (Regrid) |
| `regrid_cabida` | numeric | Sources / `regrid_cabida` | Read-only (Regrid) |
| `regrid_census_tract` | text | Sources / `regrid_census_tract` | Read-only (Regrid) |
| `regrid_data` | jsonb | Sources / `regrid_data` | Read-only (Regrid) |
| `regrid_deed_number` | text | Sources / `regrid_deed_number` | Read-only (Regrid) |
| `regrid_enriched_at` | timestamp with time zone | Sources / `regrid_enriched_at` | Read-only (Regrid) |
| `regrid_exemption` | numeric | Sources / `regrid_exemption` | Read-only (Regrid) |
| `regrid_exoneration` | numeric | Sources / `regrid_exoneration` | Read-only (Regrid) |
| `regrid_geoid` | text | Sources / `regrid_geoid` | Read-only (Regrid) |
| `regrid_gis_acreage` | numeric | Sources / `regrid_gis_acreage` | Read-only (Regrid) |
| `regrid_gis_square_feet` | numeric | Sources / `regrid_gis_square_feet` | Read-only (Regrid) |
| `regrid_improvement_value` | numeric | Sources / `regrid_improvement_value` | Read-only (Regrid) |
| `regrid_land_type` | text | Sources / `regrid_land_type` | Read-only (Regrid) |
| `regrid_land_value` | numeric | Sources / `regrid_land_value` | Read-only (Regrid) |
| `regrid_ll_uuid` | text | Sources / `regrid_ll_uuid` | Read-only (Regrid) |
| `regrid_loaded_at` | timestamp with time zone | Sources / `regrid_loaded_at` | Read-only (Regrid) |
| `regrid_lookup_query` | text | Sources / `regrid_lookup_query` | Read-only (Regrid) |
| `regrid_machinery` | numeric | Sources / `regrid_machinery` | Read-only (Regrid) |
| `regrid_match_address` | text | Sources / `regrid_match_address` | Read-only (Regrid) |
| `regrid_municipio` | text | Sources / `regrid_municipio` | Read-only (Regrid) |
| `regrid_num_catastro` | text | Sources / `regrid_num_catastro` | Read-only (Regrid) |
| `regrid_old_parcel_id` | text | Sources / `regrid_old_parcel_id` | Read-only (Regrid) |
| `regrid_original_address` | text | Sources / `regrid_original_address` | Read-only (Regrid) |
| `regrid_owner_mailing` | text | Sources / `regrid_owner_mailing` | Read-only (Regrid) |
| `regrid_owner_name` | text | Sources / `regrid_owner_name` | Read-only (Regrid) |
| `regrid_parcel_number` | text | Sources / `regrid_parcel_number` | Read-only (Regrid) |
| `regrid_path` | text | Sources / `regrid_path` | Read-only (Regrid) |
| `regrid_previous_owner` | text | Sources / `regrid_previous_owner` | Read-only (Regrid) |
| `regrid_qoz` | boolean | Sources / `regrid_qoz` | Read-only (Regrid) |
| `regrid_registry_book` | text | Sources / `regrid_registry_book` | Read-only (Regrid) |
| `regrid_registry_page` | text | Sources / `regrid_registry_page` | Read-only (Regrid) |
| `regrid_sale_date` | date | Sources / `regrid_sale_date` | Read-only (Regrid) |
| `regrid_sale_price` | numeric | Sources / `regrid_sale_price` | Read-only (Regrid) |
| `regrid_source_url` | text | Sources / `regrid_source_url` | Read-only (Regrid) |
| `regrid_stable_id` | text | Sources / `regrid_stable_id` | Read-only (Regrid) |
| `regrid_taxable_value` | numeric | Sources / `regrid_taxable_value` | Read-only (Regrid) |
| `regrid_total_value` | numeric | Sources / `regrid_total_value` | Read-only (Regrid) |
| `regrid_urbanization` | text | Sources / `regrid_urbanization` | Read-only (Regrid) |
| `regrid_usecode` | text | Sources / `regrid_usecode` | Read-only (Regrid) |
| `regrid_usedesc` | text | Sources / `regrid_usedesc` | Read-only (Regrid) |
| `regrid_zoning` | text | Sources / `regrid_zoning` | Read-only (Regrid) |
| `lot_size_sqft` | numeric | Site / `lotSizeSqft` | Editable |
| `road_frontage_feet` | numeric | Site / `roadFrontageFeet` | Editable |
| `road_surface_type` | text | Site / `roadSurfaceType` | Editable |
| `lot_description` | text | Site / `lotDescription` | Editable |
| `utilities_notes` | text | Site / `utilitiesNotes` | Editable |
| `catastro_number` | text | Site / `catastroNumber` | Editable |
| `buildability` | text | Site / `buildability` | Editable |
| `slope_description` | text | Site / `slopeDescription` | Editable |
| `pool_potential` | text | Site / `poolPotential` | Editable |
| `road_adjacency` | text | Site / `roadAdjacency` | Editable |
| `utilities_availability` | text | Site / `utilitiesAvailability` | Editable |
| `hoa_status` | text | Site / `hoaStatus` | Editable |
| `view_description` | text | Site / `viewDescription` | Editable |
| `lot_size_acres` | numeric | Site / `lotSizeAcres` | Editable |

## Stellar listing extension (17 columns)

| Column | Type | OPS tab / control | Access |
| --- | --- | --- | --- |
| `property_id` | uuid | MLS / `property_id` | Read-only (row metadata) |
| `listing_contract_date` | date | MLS / `listingContractDate` | Editable |
| `expiration_date` | date | MLS / `expirationDate` | Editable |
| `listing_type` | text | MLS / `listingType` | Editable |
| `agent_mls_id` | text | MLS / `agentMlsId` | Editable |
| `tax_id` | text | MLS / `taxId` | Editable |
| `tax_year` | integer | MLS / `taxYear` | Editable |
| `annual_tax` | numeric | MLS / `annualTax` | Editable |
| `legal_description` | text | MLS / `legalDescription` | Editable |
| `zoning` | text | MLS / `zoning` | Editable |
| `total_area_sqft` | numeric | MLS / `totalAreaSqft` | Editable |
| `heated_area_source` | text | MLS / `heatedAreaSource` | Editable |
| `ownership_type` | text | MLS / `ownershipType` | Editable |
| `hoa_details` | text | MLS / `hoaDetails` | Editable |
| `showing_instructions` | text | MLS / `showingInstructions` | Editable |
| `occupant_type` | text | MLS / `occupantType` | Editable |
| `updated_at` | timestamp with time zone | MLS / `updated_at` | Read-only (row metadata) |
