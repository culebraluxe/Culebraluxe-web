-- Severity levels on the structured error capture (Log4j-style INFO/WARN/ERROR/FATAL).
alter table app_error
  add column if not exists level text not null default 'error'
    check (level in ('info', 'warn', 'error', 'fatal'));
