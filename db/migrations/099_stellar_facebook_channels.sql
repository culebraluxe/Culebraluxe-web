-- MKT-02 — Add Stellar as a first-class channel. Facebook stays the same id.
begin;

alter table listing_syndication_placement
  drop constraint if exists listing_syndication_placement_channel_check;

alter table listing_syndication_placement
  add constraint listing_syndication_placement_channel_check
  check (channel in (
    'culebraluxe',
    'clasificados',
    'facebook_marketplace',
    'stellar_mls',
    'pr_mls',
    'amplia_mls',
    'zillow_fsbo',
    'realtor_com',
    'hubspot'
  ));

commit;
