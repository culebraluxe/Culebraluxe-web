# CIL-09 — Deal / offer desk convenes council

## Why

List-price and offer calls cost real deals. This is the first app subject.

## Units

- **A** — From deal or offer command path (not from comms), call
  `council.convene` with subjectType `deal` or `offer`, subjectId = deal/offer
  id, domain `app`, panel `deal-desk`.
- **B** — Brief is comps + asking + last offer + buyer constraints pulled
  through person/property/contract services. CRM notes labelled ASSUMPTION.
- **C** — Persist via council service. Deal stage still changes only through
  existing deal commands. Council does not write `deal.stage`.
- **D** — Refuse last-contact / channel questions.

## Acceptance

- An offer accept/counter/hold/walk pick exists as a verdict row.
- Deal stage is unchanged until a human or existing command acts on `next`.
- Comms service does not import council.

## Not this story

- Portal card is CIL-12.
- Match and intake are separate.
