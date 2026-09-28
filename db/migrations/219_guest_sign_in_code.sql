-- Emailed sign-in codes for EXTERNAL GUESTS of the public website.
--
-- A guest is an ordinary app_user with account_type 'external' and the 'guest' role, reached through auth_identity
-- like any other sign-in (provider 'email-code', subject = the verified, lower-cased email). This table only holds
-- the codes. Only a hash is kept; a code lives ten minutes, is good once, allows five tries, and issuing a new one
-- expires the older ones for that address.
set lock_timeout = '5s';
set statement_timeout = '30s';

create table if not exists guest_sign_in_code (
    id uuid primary key default gen_random_uuid(),
    email text not null,
    code_hash text not null,
    requester_ip text,
    created_at timestamptz not null default now(),
    expires_at timestamptz not null,
    attempts integer not null default 0,
    consumed_at timestamptz
);

create index if not exists idx_guest_sign_in_code_email
    on guest_sign_in_code(email, created_at desc);
create index if not exists idx_guest_sign_in_code_ip
    on guest_sign_in_code(requester_ip, created_at desc)
    where requester_ip is not null;

comment on table guest_sign_in_code is
    'Emailed guest sign-in codes, hashed. Ten-minute life, single use, five attempts; rate-limited per email and per IP.';
