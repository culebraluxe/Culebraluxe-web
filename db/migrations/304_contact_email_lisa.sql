-- The site's enquiries address is Lisa's own inbox.
--
-- `064_seed_marketing_content.sql` seeded `home.contact / email / Enquiries` with `hello@culebraluxe.com`, an
-- mailbox that was never created. Lisa does not want a second inbox, so the contact address becomes the one she
-- actually reads. 064 is left untouched on purpose: it is an applied migration, and editing it would change the
-- checksum the `schema_migration` ledger (144) records. This migration carries the correction instead.
--
-- Idempotent: re-running only rewrites the old address.
update marketing_content_item
   set value = 'lisa@culebraluxe.com'
 where value = 'hello@culebraluxe.com';

update marketing_content
   set body = replace(body, 'hello@culebraluxe.com', 'lisa@culebraluxe.com')
 where body like '%hello@culebraluxe.com%';
