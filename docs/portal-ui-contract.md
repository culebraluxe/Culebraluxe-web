# CulebraLuxe Portal UI Contract

## Purpose

The CulebraLuxe Portal is a deliberately small operational CRM.

It is not intended to replace:

- Apple Contacts
- Apple Mail
- Apple Messages
- Apple Calendar
- Apple Phone
- HubSpot marketing functionality
- Sanity editorial/media management

The Portal exists to provide business context around:

- clients
- properties
- opportunities
- deals
- property interests
- interactions
- next actions

The initial Portal contains exactly three primary screens:

1. Dashboard
2. Deals Portfolio
3. Client Manager

The database model should exist to support these screens rather than dictate them.

---

# 1. Client Manager

## Client

Required fields:

- id
- display name
- role
- status
- location
- email
- phone
- assigned agent

Optional business profile fields:

- minimum budget
- maximum budget
- preferred areas
- preferred property types
- priorities
- buying/selling timeline
- relationship notes

## Client Role

Allowed initial values:

- buyer
- seller
- both

## Client Status

Allowed initial values:

- new
- warm
- active
- referral

## Property Interests

A client may be interested in zero or more properties.

Required relationship data:

- client
- property
- interest status

Optional relationship data:

- notes
- ranking
- created date
- updated date

Initial interest statuses:

- interested
- shortlisted
- tour completed

## Last Contact

The Client Manager must be able to display:

- interaction channel
- interaction date/time
- short summary

This should ultimately be derived from interaction history rather than permanently duplicated on the client record.

## Next Action

The Client Manager must display:

- action title
- date/time
- optional detail
- related client
- optionally related property
- optionally related deal

## Interaction Timeline

Required interaction fields:

- id
- client
- channel
- date/time
- direction where applicable
- short title
- summary

Optional relationships:

- property
- deal

Initial channels:

- email
- call
- imessage
- sms
- meeting
- showing
- note

Initial directions:

- inbound
- outbound

The Portal should store business-relevant interaction metadata and summaries.

It does not need to become an email, messaging, calendar, or telephone application.

---

# 2. Deals Portfolio

## Deal

Required fields:

- id
- property
- client
- stage
- owner

Financial fields:

- list price
- offer price

Workflow fields:

- next milestone
- next milestone date/time
- closing date

Activity:

- last activity
- last activity date/time

Last activity should eventually be derived from interactions and deal events where practical.

## Deal Stage

Initial stages:

- new lead
- qualified
- showing
- offer
- under contract
- closed

## Portfolio Table

The Deals Portfolio table requires:

- property
- property location
- property descriptor
- client
- stage
- price or current offer
- next milestone
- next milestone date
- last activity
- last activity date
- owner

## Pipeline View

The same deals must be groupable by stage.

No separate pipeline records should be required.

## Priority Closings

The Portal must be able to identify deals with closing dates and present upcoming closings.

This should be derived from deal records.

---

# 3. Dashboard

The Dashboard is primarily a derived view.

It should avoid introducing new business entities solely for dashboard presentation.

## Summary Metrics

Initial metrics:

- active clients
- live deals
- upcoming actions
- deals under contract

## Needs Attention

Derived primarily from:

- client status
- next actions
- overdue actions
- stale interactions

## Upcoming

Derived primarily from:

- actions
- meetings
- showings
- deal milestones

## Featured Opportunity

Derived from:

- deal
- property
- client

## Portfolio Snapshot

Derived from deals grouped by stage.

## Recent Activity

Derived from interactions across clients.

The Dashboard should query operational data rather than maintain its own duplicate records.

---

# 4. Property

Property is a shared business entity used by both the public website and the Portal.

The operational property identity should eventually be canonical in Postgres.

Initial operational fields should include:

- id
- title/name
- location
- status
- list price
- bedrooms
- bathrooms
- square feet
- property type
- MLS/listing identifier where applicable

Optional operational relationships:

- seller
- listing agent
- deals
- interested clients

Sanity remains responsible for editorial/media data such as:

- long-form property description
- hero image
- image gallery
- editorial presentation
- SEO copy
- neighborhood storytelling

Postgres and Sanity should share a stable property identifier.

---

# 5. Person Identity

A person may have multiple identifiers originating from different systems.

The operational model should support identities such as:

- email address
- phone number
- Apple contact identifier
- HubSpot identifier
- future external-system identifiers

These identities should map back to one canonical person.

This allows future Apple, HubSpot, website, email, phone, and messaging events to resolve to the correct client.

---

# 6. Design Principles

## Signal Over Noise

Every field and workflow must justify its presence in one of the three Portal screens.

## No Wheel Reinvention

Do not build replacements for mature tools already used by the company.

## Minimize Duplicate Data Entry

Information that already exists in another authoritative source should be integrated or referenced where practical.

## Postgres as Operational System of Record

Postgres should eventually own canonical operational entities and relationships.

## Sanity as Editorial System

Sanity should continue managing website-oriented editorial content and media.

## External Systems as Services

HubSpot, Apple services, signing platforms, and future systems should remain specialized services rather than becoming the Portal's data model.

## Derived Data Where Possible

Values such as:

- last contact
- activity counts
- pipeline counts
- dashboard metrics

should normally be calculated from underlying records rather than manually maintained.

---

# Initial Domain Model

Expected initial relational entities:

- person
- person_identity
- property
- property_interest
- deal
- interaction
- task
- user

This list is provisional.

The final Postgres schema should be derived from this UI contract and may consolidate or expand entities where relational integrity requires it.