# CulebraLuxe Property UI Contract

## Routes
- /buyers
- /properties
- /properties/[slug]

## Property Card Fields
- id
- title
- slug
- listPrice
- standardStatus
- propertyType
- bedroomsTotal
- bathroomsTotal
- livingArea
- neighborhood
- viewType
- heroImage
- featured
- shortDescription

## Property Detail Fields
Use the Neon/Postgres `property` table as the canonical source
(PLAT-01 Property Source Consolidation — Sanity is retired as a property source).

Key display fields:
- title
- slug
- listPrice
- standardStatus
- propertyType
- bedroomsTotal
- bathroomsTotal
- livingArea
- lotSizeArea
- lotSizeUnits
- yearBuilt
- neighborhood
- viewType
- heroImage
- gallery
- shortDescription
- editorialDescription
- amenities
- latitude
- longitude

## Behavioral Rules
- No hard-coded property content
- Hero image is explicit
- Gallery order follows `property_media` ordering (role/order owned by
  `property_media`, per AGENTS.md)
- Hide missing values
- Do not invent values
- Missing price => "Price Upon Request"
- Land does not show bedroom/bath fields
- Empty detail sections do not render
