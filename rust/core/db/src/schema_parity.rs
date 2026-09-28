//! Schema parity: structural comparison of two control-plane databases.
//!
//! Rust home for what used to be `lib/schema-parity.ts` (the pure comparison) plus
//! `legacy/workflow_app/forge/schema-parity.ts` (the database reader). Both were deleted with the
//! TypeScript application in `4cf98110`, which left `pnpm db:parity` — a release gate — dead.
//!
//! The comparison is PURE on purpose (snapshot in, report out): unit testable without a database, and
//! reusable by a Rust DEV_OPS gate. `read_snapshot` is the only part that touches a database, and it
//! reads nothing but the catalogue.
//!
//! Compares five axes: tables, columns, indexes, foreign keys, check constraints.
//! Indexes are not cosmetic — the Forge dispatch lock lives in a partial unique index, so a missing
//! index silently changes engine behavior.
//!
//! FORGE-PARITY-CHECK-01: the fifth axis was added because it was the one carrying real drift. A CHECK
//! constraint is enforcement, not decoration: on 2026-09-12 PROD carried
//! `agent_work_item_parallel_shape_check` on the split lane's parallel shape and DEV did not, and the
//! gate called the two databases identical. A constraint is also asymmetric in a way the other axes are
//! not — PROD being STRICTER than DEV is the dangerous direction (a worker passes in DEV and fails in
//! PROD), so validity is part of the compared value: a NOT VALID constraint enforces nothing for
//! existing rows and must not read as equal to a validated one.

use crate::error::{DbFailure, DbResult};
use crate::pool::Database;
use std::collections::BTreeMap;

/// Collapse runs of whitespace, as the TypeScript comparison did with `/\s+/g` + `trim()`.
///
/// The format has ONE owner: the reader supplies a definition and the comparison's meaning stays pure
/// and testable.
fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The compared value of one CHECK constraint: its definition normalized, with `NOT VALID` appended
/// when Postgres is not enforcing it for existing rows.
///
/// A NOT VALID constraint enforces nothing retroactively, so treating it as equal to a validated one
/// would hide exactly the "PROD is stricter than DEV" case this axis exists to catch.
pub fn check_value(definition: &str, validated: bool) -> String {
    let normalized = normalize_whitespace(definition);
    if validated {
        normalized
    } else {
        format!("{normalized} NOT VALID")
    }
}

/// One database's structural shape. Column, index, FK and check maps are keyed the way the report
/// names them, so a rename is visible rather than silently treated as equivalent.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SchemaSnapshot {
    pub tables: Vec<String>,
    /// table -> column -> `"data_type[ NOT NULL]"`.
    pub columns: BTreeMap<String, BTreeMap<String, String>>,
    /// `"table.indexname"` -> normalized `indexdef`.
    pub indexes: BTreeMap<String, String>,
    /// constraint name -> `"child -> parent"`.
    pub fks: BTreeMap<String, String>,
    /// `"table.constraintName"` -> normalized definition, with NOT VALID marked.
    pub checks: BTreeMap<String, String>,
}

/// The gate's answer: what differs, and whether anything does.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ParityReport {
    pub tables_only_dev: Vec<String>,
    pub tables_only_prod: Vec<String>,
    pub column_drift: Vec<String>,
    pub index_drift: Vec<String>,
    pub fk_drift: Vec<String>,
    pub check_drift: Vec<String>,
    pub clean: bool,
}

impl ParityReport {
    /// Every drift line, in report order, for callers that only want to print them.
    pub fn drift_lines(&self) -> impl Iterator<Item = &str> {
        self.column_drift
            .iter()
            .chain(self.index_drift.iter())
            .chain(self.fk_drift.iter())
            .chain(self.check_drift.iter())
            .map(String::as_str)
    }
}

pub fn compare_snapshots(dev: &SchemaSnapshot, prod: &SchemaSnapshot) -> ParityReport {
    let tables_only_dev: Vec<String> = dev
        .tables
        .iter()
        .filter(|t| !prod.tables.contains(t))
        .cloned()
        .collect();
    let tables_only_prod: Vec<String> = prod
        .tables
        .iter()
        .filter(|t| !dev.tables.contains(t))
        .cloned()
        .collect();

    let empty = BTreeMap::new();
    let mut column_drift: Vec<String> = Vec::new();
    for table in dev.tables.iter().filter(|t| prod.tables.contains(t)) {
        let dev_columns = dev.columns.get(table).unwrap_or(&empty);
        let prod_columns = prod.columns.get(table).unwrap_or(&empty);

        let dev_only: Vec<&str> = dev_columns
            .keys()
            .filter(|c| !prod_columns.contains_key(*c))
            .map(String::as_str)
            .collect();
        let prod_only: Vec<&str> = prod_columns
            .keys()
            .filter(|c| !dev_columns.contains_key(*c))
            .map(String::as_str)
            .collect();
        let type_drift: Vec<&str> = dev_columns
            .keys()
            .filter(|c| prod_columns.get(*c).is_some_and(|value| Some(value) != dev_columns.get(*c)))
            .map(String::as_str)
            .collect();

        if !dev_only.is_empty() {
            column_drift.push(format!("{table}: DEV-only cols {}", dev_only.join(", ")));
        }
        if !prod_only.is_empty() {
            column_drift.push(format!("{table}: PROD-only cols {}", prod_only.join(", ")));
        }
        for column in type_drift {
            column_drift.push(format!(
                "{table}.{column}: DEV={} PROD={}",
                dev_columns.get(column).map(String::as_str).unwrap_or("-"),
                prod_columns.get(column).map(String::as_str).unwrap_or("-")
            ));
        }
    }

    let mut index_drift: Vec<String> = Vec::new();
    for key in dev.indexes.keys().chain(
        prod.indexes
            .keys()
            .filter(|key| !dev.indexes.contains_key(*key)),
    ) {
        let dev_value = dev.indexes.get(key);
        let prod_value = prod.indexes.get(key);
        if dev_value == prod_value {
            continue;
        }
        match (dev_value, prod_value) {
            (None, Some(_)) => index_drift.push(format!("{key}: PROD-only")),
            (Some(_), None) => index_drift.push(format!("{key}: DEV-only")),
            _ => index_drift.push(format!("{key}: definition differs")),
        }
    }

    let mut fk_drift: Vec<String> = Vec::new();
    for key in dev.fks.keys().chain(
        prod.fks
            .keys()
            .filter(|key| !dev.fks.contains_key(*key)),
    ) {
        let dev_value = dev.fks.get(key);
        let prod_value = prod.fks.get(key);
        if dev_value == prod_value {
            continue;
        }
        fk_drift.push(format!(
            "{key}: DEV={} PROD={}",
            dev_value.map(String::as_str).unwrap_or("-"),
            prod_value.map(String::as_str).unwrap_or("-")
        ));
    }

    // Keyed by name (like indexes and FKs, so a rename is visible) and compared on the normalized
    // definition plus validity, because two constraints with the same name and different definitions
    // is precisely the drift this axis exists for. Present on one side only prints as `DEV=-`/`PROD=-`.
    let mut check_drift: Vec<String> = Vec::new();
    for key in dev.checks.keys().chain(
        prod.checks
            .keys()
            .filter(|key| !dev.checks.contains_key(*key)),
    ) {
        let dev_value = dev.checks.get(key);
        let prod_value = prod.checks.get(key);
        if dev_value == prod_value {
            continue;
        }
        check_drift.push(format!(
            "{key}: DEV={} PROD={}",
            dev_value.map(String::as_str).unwrap_or("-"),
            prod_value.map(String::as_str).unwrap_or("-")
        ));
    }

    let clean = tables_only_dev.is_empty()
        && tables_only_prod.is_empty()
        && column_drift.is_empty()
        && index_drift.is_empty()
        && fk_drift.is_empty()
        && check_drift.is_empty();

    ParityReport {
        tables_only_dev,
        tables_only_prod,
        column_drift,
        index_drift,
        fk_drift,
        check_drift,
        clean,
    }
}

/// Read one database's structural shape. Catalogue only — no business table is touched.
pub async fn read_snapshot(db: &Database) -> DbResult<SchemaSnapshot> {
    use sqlx::Row;

    let pool = db.pool();
    let operation = "db.schema_parity.read";
    let read = |error: sqlx::Error| DbFailure::from_sqlx(operation, &error);

    let tables: Vec<String> = sqlx::query_scalar(
        "select table_name from information_schema.tables \
         where table_schema = 'public' and table_type = 'BASE TABLE' order by 1",
    )
    .fetch_all(pool)
    .await
    .map_err(read)?;

    let mut columns: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let column_rows = sqlx::query(
        "select table_name, column_name, data_type, is_nullable from information_schema.columns \
         where table_schema = 'public' order by 1, 2",
    )
    .fetch_all(pool)
    .await
    .map_err(read)?;
    for row in column_rows {
        let table: String = row.try_get("table_name").map_err(read)?;
        let column: String = row.try_get("column_name").map_err(read)?;
        let data_type: String = row.try_get("data_type").map_err(read)?;
        let is_nullable: String = row.try_get("is_nullable").map_err(read)?;
        let value = if is_nullable == "NO" {
            format!("{data_type} NOT NULL")
        } else {
            data_type
        };
        columns.entry(table).or_default().insert(column, value);
    }

    let mut indexes: BTreeMap<String, String> = BTreeMap::new();
    let index_rows = sqlx::query(
        "select tablename, indexname, indexdef from pg_indexes \
         where schemaname = 'public' order by 1, 2",
    )
    .fetch_all(pool)
    .await
    .map_err(read)?;
    for row in index_rows {
        let table: String = row.try_get("tablename").map_err(read)?;
        let index: String = row.try_get("indexname").map_err(read)?;
        let definition: String = row.try_get("indexdef").map_err(read)?;
        indexes.insert(format!("{table}.{index}"), normalize_whitespace(&definition));
    }

    let mut fks: BTreeMap<String, String> = BTreeMap::new();
    let fk_rows = sqlx::query(
        "select con.conname, con.conrelid::regclass::text as child, \
                con.confrelid::regclass::text as parent \
         from pg_constraint con join pg_namespace n on n.oid = con.connamespace \
         where con.contype = 'f' and n.nspname = 'public' order by 1",
    )
    .fetch_all(pool)
    .await
    .map_err(read)?;
    for row in fk_rows {
        let name: String = row.try_get("conname").map_err(read)?;
        let child: String = row.try_get("child").map_err(read)?;
        let parent: String = row.try_get("parent").map_err(read)?;
        fks.insert(name, format!("{child} -> {parent}"));
    }

    let mut checks: BTreeMap<String, String> = BTreeMap::new();
    let check_rows = sqlx::query(
        "select con.conname, con.conrelid::regclass::text as tbl, con.convalidated, \
                pg_get_constraintdef(con.oid) as def \
         from pg_constraint con join pg_namespace n on n.oid = con.connamespace \
         where con.contype = 'c' and n.nspname = 'public' order by 2, 1",
    )
    .fetch_all(pool)
    .await
    .map_err(read)?;
    for row in check_rows {
        let name: String = row.try_get("conname").map_err(read)?;
        let table: String = row.try_get("tbl").map_err(read)?;
        let validated: bool = row.try_get("convalidated").map_err(read)?;
        let definition: String = row.try_get("def").map_err(read)?;
        checks.insert(
            format!("{table}.{name}"),
            check_value(&definition, validated),
        );
    }

    Ok(SchemaSnapshot {
        tables,
        columns,
        indexes,
        fks,
        checks,
    })
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Build a snapshot from literals, so each test states only the axis it is about.
    fn snapshot(
        tables: &[&str],
        columns: &[(&str, &[(&str, &str)])],
        indexes: &[(&str, &str)],
        fks: &[(&str, &str)],
        checks: &[(&str, &str)],
    ) -> SchemaSnapshot {
        let to_map = |pairs: &[(&str, &str)]| {
            pairs
                .iter()
                .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                .collect::<BTreeMap<String, String>>()
        };
        SchemaSnapshot {
            tables: tables.iter().map(|table| (*table).to_string()).collect(),
            columns: columns
                .iter()
                .map(|(table, cols)| {
                    (
                        (*table).to_string(),
                        cols.iter()
                            .map(|(column, kind)| ((*column).to_string(), (*kind).to_string()))
                            .collect(),
                    )
                })
                .collect(),
            indexes: to_map(indexes),
            fks: to_map(fks),
            checks: to_map(checks),
        }
    }

    #[test]
    fn identical_schemas_are_clean() {
        let schema = snapshot(
            &["property"],
            &[("property", &[("id", "uuid NOT NULL")])],
            &[(
                "property.property_pkey",
                "CREATE UNIQUE INDEX property_pkey ON public.property USING btree (id)",
            )],
            &[],
            &[],
        );
        let report = compare_snapshots(&schema, &schema.clone());
        assert!(report.clean, "expected clean, got {report:?}");
        assert_eq!(report.drift_lines().count(), 0);
    }

    #[test]
    fn tables_on_one_side_only_are_named_in_the_right_bucket() {
        let dev = snapshot(&["property", "dev_only"], &[], &[], &[], &[]);
        let prod = snapshot(&["property", "prod_only"], &[], &[], &[], &[]);
        let report = compare_snapshots(&dev, &prod);
        assert_eq!(report.tables_only_dev, vec!["dev_only".to_string()]);
        assert_eq!(report.tables_only_prod, vec!["prod_only".to_string()]);
        assert!(!report.clean);
    }

    #[test]
    fn column_drift_reports_both_directions_and_type_changes() {
        let dev = snapshot(
            &["property"],
            &[(
                "property",
                &[
                    ("id", "uuid NOT NULL"),
                    ("dev_col", "text"),
                    ("price", "integer"),
                ],
            )],
            &[],
            &[],
            &[],
        );
        let prod = snapshot(
            &["property"],
            &[(
                "property",
                &[
                    ("id", "uuid NOT NULL"),
                    ("prod_col", "text"),
                    ("price", "bigint"),
                ],
            )],
            &[],
            &[],
            &[],
        );
        let report = compare_snapshots(&dev, &prod);
        assert!(report
            .column_drift
            .contains(&"property: DEV-only cols dev_col".to_string()));
        assert!(report
            .column_drift
            .contains(&"property: PROD-only cols prod_col".to_string()));
        assert!(report
            .column_drift
            .contains(&"property.price: DEV=integer PROD=bigint".to_string()));
        assert!(!report.clean);
    }

    #[test]
    fn index_drift_distinguishes_side_from_definition() {
        let dev = snapshot(
            &["property"],
            &[],
            &[
                (
                    "property.idx_dev",
                    "CREATE INDEX idx_dev ON public.property USING btree (a)",
                ),
                (
                    "property.idx_both",
                    "CREATE INDEX idx_both ON public.property USING btree (b)",
                ),
            ],
            &[],
            &[],
        );
        let prod = snapshot(
            &["property"],
            &[],
            &[
                (
                    "property.idx_prod",
                    "CREATE INDEX idx_prod ON public.property USING btree (c)",
                ),
                (
                    "property.idx_both",
                    "CREATE INDEX idx_both ON public.property USING btree (zz)",
                ),
            ],
            &[],
            &[],
        );
        let report = compare_snapshots(&dev, &prod);
        assert!(report
            .index_drift
            .contains(&"property.idx_dev: DEV-only".to_string()));
        assert!(report
            .index_drift
            .contains(&"property.idx_prod: PROD-only".to_string()));
        assert!(report
            .index_drift
            .contains(&"property.idx_both: definition differs".to_string()));
    }

    #[test]
    fn foreign_key_drift_names_both_sides() {
        let dev = snapshot(&["deal"], &[], &[], &[("deal_fk", "deal -> property")], &[]);
        let prod = snapshot(&["deal"], &[], &[], &[], &[]);
        let report = compare_snapshots(&dev, &prod);
        assert_eq!(
            report.fk_drift,
            vec!["deal_fk: DEV=deal -> property PROD=-".to_string()]
        );
    }

    /// FORGE-PARITY-CHECK-01: a constraint PROD enforces and DEV does not is DRIFT, even with the same
    /// name and the same definition text. This is the case the gate used to report as clean.
    #[test]
    fn a_not_valid_constraint_is_not_equal_to_a_validated_one() {
        assert_eq!(check_value("CHECK ((a > 0))", true), "CHECK ((a > 0))");
        assert_eq!(
            check_value("CHECK ((a > 0))", false),
            "CHECK ((a > 0)) NOT VALID"
        );

        let name = "agent_work_item.agent_work_item_parallel_shape_check";
        let dev = snapshot(
            &["agent_work_item"],
            &[],
            &[],
            &[],
            &[(name, "CHECK ((a > 0)) NOT VALID")],
        );
        let prod = snapshot(&["agent_work_item"], &[], &[], &[], &[(name, "CHECK ((a > 0))")]);
        let report = compare_snapshots(&dev, &prod);
        assert_eq!(report.check_drift.len(), 1, "{report:?}");
        assert!(report.check_drift[0].contains("DEV=CHECK ((a > 0)) NOT VALID"));
        assert!(report.check_drift[0].contains("PROD=CHECK ((a > 0))"));
        assert!(
            !report.clean,
            "PROD stricter than DEV must never read as clean"
        );
    }

    #[test]
    fn check_definitions_are_normalized_before_comparison() {
        // Normalization lives where the value is READ (`check_value`), so two databases agree even when
        // Postgres hands back the same constraint with different whitespace.
        let normalized = check_value("CHECK  ((a\n  > 0))", true);
        assert_eq!(normalized, "CHECK ((a > 0))");
        let dev = snapshot(&["t"], &[], &[], &[], &[("t.t_a_check", normalized.as_str())]);
        let prod = snapshot(&["t"], &[], &[], &[], &[("t.t_a_check", "CHECK ((a > 0))")]);
        assert!(compare_snapshots(&dev, &prod).clean);
    }

    #[test]
    fn a_check_constraint_on_one_side_only_is_drift() {
        let dev = snapshot(&["t"], &[], &[], &[], &[]);
        let prod = snapshot(&["t"], &[], &[], &[], &[("t.t_a_check", "CHECK ((a > 0))")]);
        let report = compare_snapshots(&dev, &prod);
        assert_eq!(
            report.check_drift,
            vec!["t.t_a_check: DEV=- PROD=CHECK ((a > 0))".to_string()]
        );
        assert!(!report.clean);
    }


}

