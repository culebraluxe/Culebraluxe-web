//! SQL the Neon adapter issues. Same tables, same lock order as the TS kernel.

pub const LOCK_INSTANCE: &str = "SELECT ... FROM process_instances WHERE id = $1::uuid FOR UPDATE";
