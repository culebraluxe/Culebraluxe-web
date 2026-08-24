# Neon Story Architecture Snapshot — 2026-08-24

- Project: `CulebraluxeData`
- Project ID: `snowy-salad-48970537`
- Database: `neondb`
- Investigation mode: read-only
- `public.storyboard_story` found: yes
- Full text policy: values below are preserved exactly as returned by the Neon connector. JSON escaping represents embedded newlines without summarization.

## Matching Tables

```json
[
  {
    "table_schema": "public",
    "table_name": "storyboard_story",
    "table_type": "BASE TABLE"
  },
  {
    "table_schema": "public",
    "table_name": "storyboard_story_run",
    "table_type": "BASE TABLE"
  }
]
```

## Complete Column Definitions, Constraints, and Indexes

```json
[
  {
    "table_schema": "public",
    "table_name": "storyboard_story",
    "columns": [
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "id",
        "is_identity": "NO",
        "is_nullable": "NO",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 1,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "workstream",
        "is_identity": "NO",
        "is_nullable": "NO",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 2,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "title",
        "is_identity": "NO",
        "is_nullable": "NO",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 3,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "priority",
        "is_identity": "NO",
        "is_nullable": "NO",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 4,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "status",
        "is_identity": "NO",
        "is_nullable": "NO",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 5,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "notes",
        "is_identity": "NO",
        "is_nullable": "NO",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": "''::text",
        "ordinal_position": 6,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "int4",
        "data_type": "integer",
        "udt_schema": "pg_catalog",
        "column_name": "batch",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": 0,
        "column_default": null,
        "ordinal_position": 7,
        "numeric_precision": 32,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "goal",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 8,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "scope",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 9,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "acceptance_criteria",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 10,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "dependencies",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 11,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "timestamptz",
        "data_type": "timestamp with time zone",
        "udt_schema": "pg_catalog",
        "column_name": "created_at",
        "is_identity": "NO",
        "is_nullable": "NO",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": "now()",
        "ordinal_position": 12,
        "numeric_precision": null,
        "datetime_precision": 6,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "timestamptz",
        "data_type": "timestamp with time zone",
        "udt_schema": "pg_catalog",
        "column_name": "updated_at",
        "is_identity": "NO",
        "is_nullable": "NO",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": "now()",
        "ordinal_position": 13,
        "numeric_precision": null,
        "datetime_precision": 6,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "int4",
        "data_type": "integer",
        "udt_schema": "pg_catalog",
        "column_name": "completion",
        "is_identity": "NO",
        "is_nullable": "NO",
        "is_generated": "NEVER",
        "numeric_scale": 0,
        "column_default": "0",
        "ordinal_position": 14,
        "numeric_precision": 32,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "bool",
        "data_type": "boolean",
        "udt_schema": "pg_catalog",
        "column_name": "rollup",
        "is_identity": "NO",
        "is_nullable": "NO",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": "true",
        "ordinal_position": 15,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "timestamptz",
        "data_type": "timestamp with time zone",
        "udt_schema": "pg_catalog",
        "column_name": "planned_start_at",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 16,
        "numeric_precision": null,
        "datetime_precision": 6,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "timestamptz",
        "data_type": "timestamp with time zone",
        "udt_schema": "pg_catalog",
        "column_name": "actual_start_at",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 17,
        "numeric_precision": null,
        "datetime_precision": 6,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "timestamptz",
        "data_type": "timestamp with time zone",
        "udt_schema": "pg_catalog",
        "column_name": "completed_at",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 18,
        "numeric_precision": null,
        "datetime_precision": 6,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "preconditions",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 19,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "architect_brief",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 20,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "context_refs",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 21,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "postconditions",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 22,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "timestamptz",
        "data_type": "timestamp with time zone",
        "udt_schema": "pg_catalog",
        "column_name": "architect_brief_updated_at",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 23,
        "numeric_precision": null,
        "datetime_precision": 6,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "operating_surface",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 24,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      }
    ],
    "constraints": [
      {
        "definition": "CHECK (completion >= 0 AND completion <= 100)",
        "constraint_name": "storyboard_story_completion_check",
        "constraint_type": "CHECK",
        "referenced_table": null
      },
      {
        "definition": "NOT NULL completion",
        "constraint_name": "storyboard_story_completion_not_null",
        "constraint_type": "NOT NULL",
        "referenced_table": null
      },
      {
        "definition": "NOT NULL created_at",
        "constraint_name": "storyboard_story_created_at_not_null",
        "constraint_type": "NOT NULL",
        "referenced_table": null
      },
      {
        "definition": "NOT NULL id",
        "constraint_name": "storyboard_story_id_not_null",
        "constraint_type": "NOT NULL",
        "referenced_table": null
      },
      {
        "definition": "NOT NULL notes",
        "constraint_name": "storyboard_story_notes_not_null",
        "constraint_type": "NOT NULL",
        "referenced_table": null
      },
      {
        "definition": "CHECK (operating_surface IS NULL OR (operating_surface = ANY (ARRAY['NEXUS'::text, 'OPS'::text, 'TECH'::text, 'SUPPORT'::text])))",
        "constraint_name": "storyboard_story_operating_surface_check",
        "constraint_type": "CHECK",
        "referenced_table": null
      },
      {
        "definition": "PRIMARY KEY (id)",
        "constraint_name": "storyboard_story_pkey",
        "constraint_type": "PRIMARY KEY",
        "referenced_table": null
      },
      {
        "definition": "NOT NULL priority",
        "constraint_name": "storyboard_story_priority_not_null",
        "constraint_type": "NOT NULL",
        "referenced_table": null
      },
      {
        "definition": "NOT NULL rollup",
        "constraint_name": "storyboard_story_rollup_not_null",
        "constraint_type": "NOT NULL",
        "referenced_table": null
      },
      {
        "definition": "CHECK (status = ANY (ARRAY['Planned'::text, 'Ready'::text, 'In Progress'::text, 'Complete'::text, 'Partial'::text, 'Blocked'::text, 'Failed'::text, 'Deferred'::text, 'Hold'::text]))",
        "constraint_name": "storyboard_story_status_check",
        "constraint_type": "CHECK",
        "referenced_table": null
      },
      {
        "definition": "NOT NULL status",
        "constraint_name": "storyboard_story_status_not_null",
        "constraint_type": "NOT NULL",
        "referenced_table": null
      },
      {
        "definition": "NOT NULL title",
        "constraint_name": "storyboard_story_title_not_null",
        "constraint_type": "NOT NULL",
        "referenced_table": null
      },
      {
        "definition": "NOT NULL updated_at",
        "constraint_name": "storyboard_story_updated_at_not_null",
        "constraint_type": "NOT NULL",
        "referenced_table": null
      },
      {
        "definition": "NOT NULL workstream",
        "constraint_name": "storyboard_story_workstream_not_null",
        "constraint_type": "NOT NULL",
        "referenced_table": null
      }
    ],
    "indexes": [
      {
        "index_name": "storyboard_story_pkey",
        "index_definition": "CREATE UNIQUE INDEX storyboard_story_pkey ON public.storyboard_story USING btree (id)"
      }
    ]
  },
  {
    "table_schema": "public",
    "table_name": "storyboard_story_run",
    "columns": [
      {
        "udt_name": "uuid",
        "data_type": "uuid",
        "udt_schema": "pg_catalog",
        "column_name": "id",
        "is_identity": "NO",
        "is_nullable": "NO",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": "gen_random_uuid()",
        "ordinal_position": 1,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "story_id",
        "is_identity": "NO",
        "is_nullable": "NO",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 2,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "timestamptz",
        "data_type": "timestamp with time zone",
        "udt_schema": "pg_catalog",
        "column_name": "started_at",
        "is_identity": "NO",
        "is_nullable": "NO",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 3,
        "numeric_precision": null,
        "datetime_precision": 6,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "timestamptz",
        "data_type": "timestamp with time zone",
        "udt_schema": "pg_catalog",
        "column_name": "ended_at",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 4,
        "numeric_precision": null,
        "datetime_precision": 6,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "result_status",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 5,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "int4",
        "data_type": "integer",
        "udt_schema": "pg_catalog",
        "column_name": "completion",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": 0,
        "column_default": null,
        "ordinal_position": 6,
        "numeric_precision": 32,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "notes",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 7,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "commit_hash",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 8,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "tests_summary",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 9,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "timestamptz",
        "data_type": "timestamp with time zone",
        "udt_schema": "pg_catalog",
        "column_name": "created_at",
        "is_identity": "NO",
        "is_nullable": "NO",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": "now()",
        "ordinal_position": 10,
        "numeric_precision": null,
        "datetime_precision": 6,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "goal_snapshot",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 11,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "preconditions_snapshot",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 12,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "architect_brief_snapshot",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 13,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "context_refs_snapshot",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 14,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "acceptance_criteria_snapshot",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 15,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "postconditions_snapshot",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 16,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "timestamptz",
        "data_type": "timestamp with time zone",
        "udt_schema": "pg_catalog",
        "column_name": "updated_at",
        "is_identity": "NO",
        "is_nullable": "NO",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": "now()",
        "ordinal_position": 17,
        "numeric_precision": null,
        "datetime_precision": 6,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      },
      {
        "udt_name": "text",
        "data_type": "text",
        "udt_schema": "pg_catalog",
        "column_name": "execution_environment",
        "is_identity": "NO",
        "is_nullable": "YES",
        "is_generated": "NEVER",
        "numeric_scale": null,
        "column_default": null,
        "ordinal_position": 18,
        "numeric_precision": null,
        "datetime_precision": null,
        "identity_generation": null,
        "generation_expression": null,
        "character_maximum_length": null
      }
    ],
    "constraints": [
      {
        "definition": "CHECK (completion IS NULL OR completion >= 0 AND completion <= 100)",
        "constraint_name": "storyboard_story_run_completion_check",
        "constraint_type": "CHECK",
        "referenced_table": null
      },
      {
        "definition": "NOT NULL created_at",
        "constraint_name": "storyboard_story_run_created_at_not_null",
        "constraint_type": "NOT NULL",
        "referenced_table": null
      },
      {
        "definition": "CHECK (execution_environment IS NULL OR (execution_environment = ANY (ARRAY['DEV'::text, 'PROD'::text, 'TEST'::text, 'LOCAL'::text])))",
        "constraint_name": "storyboard_story_run_execution_environment_check",
        "constraint_type": "CHECK",
        "referenced_table": null
      },
      {
        "definition": "NOT NULL id",
        "constraint_name": "storyboard_story_run_id_not_null",
        "constraint_type": "NOT NULL",
        "referenced_table": null
      },
      {
        "definition": "PRIMARY KEY (id)",
        "constraint_name": "storyboard_story_run_pkey",
        "constraint_type": "PRIMARY KEY",
        "referenced_table": null
      },
      {
        "definition": "CHECK (result_status IS NULL OR (result_status = ANY (ARRAY['Complete'::text, 'Partial'::text, 'Blocked'::text, 'Failed'::text, 'Deferred'::text, 'Hold'::text, 'Cancelled'::text])))",
        "constraint_name": "storyboard_story_run_result_status_check",
        "constraint_type": "CHECK",
        "referenced_table": null
      },
      {
        "definition": "NOT NULL started_at",
        "constraint_name": "storyboard_story_run_started_at_not_null",
        "constraint_type": "NOT NULL",
        "referenced_table": null
      },
      {
        "definition": "FOREIGN KEY (story_id) REFERENCES storyboard_story(id) ON DELETE CASCADE",
        "constraint_name": "storyboard_story_run_story_id_fkey",
        "constraint_type": "FOREIGN KEY",
        "referenced_table": "storyboard_story"
      },
      {
        "definition": "NOT NULL story_id",
        "constraint_name": "storyboard_story_run_story_id_not_null",
        "constraint_type": "NOT NULL",
        "referenced_table": null
      },
      {
        "definition": "NOT NULL updated_at",
        "constraint_name": "storyboard_story_run_updated_at_not_null",
        "constraint_type": "NOT NULL",
        "referenced_table": null
      }
    ],
    "indexes": [
      {
        "index_name": "idx_storyboard_story_run_story_id",
        "index_definition": "CREATE INDEX idx_storyboard_story_run_story_id ON public.storyboard_story_run USING btree (story_id)"
      },
      {
        "index_name": "storyboard_story_run_pkey",
        "index_definition": "CREATE UNIQUE INDEX storyboard_story_run_pkey ON public.storyboard_story_run USING btree (id)"
      }
    ]
  }
]
```

## Row Counts

```json
[
  {
    "table_name": "public.storyboard_story",
    "row_count": "155"
  },
  {
    "table_name": "public.storyboard_story_run",
    "row_count": "79"
  }
]
```

## Records Associated with ARCH-01 or SOP-1

The catalog-wide architecture/story-field search result follows. An empty array means no matching record was found.

```json
[]
```

## Requested Planned Architecture Records

Includes CRM-26, CRM-27, CRM-28, OPS-11A, OPS-11B, and OPS-11C.

```json
[
  {
    "story": {
      "id": "CRM-26",
      "workstream": "TXN",
      "title": "Executed P&S → Canonical Deal Projection + Workflow Advance",
      "priority": "High",
      "status": "Planned",
      "notes": "Created from 2026-08-24 deal/workflow architecture reconciliation. This is the narrow seam where the signed agreement becomes operational deal truth; do not parse or model legal prose.",
      "batch": null,
      "goal": "When a deal-linked Purchase & Sale becomes fully executed, promote only the recurring operational terms the application must reason about into canonical Deal facts and advance the existing P&S workflow task without creating a second business-state model.",
      "scope": "Build an idempotent application service/subscriber for PR-PNS execution. Read the immutable issued/signed form snapshot; map closingDate, inspectionDeadline, financingDeadline, financing type, appraisal applicability and other explicitly approved recurring operational fields to existing canonical commands/services. Complete/advance the correlated pns_executed workflow task and allow the existing mark_under_contract command-node to set deal.stage. Preserve all negotiated/legal prose only in the executed document. Do not start a new workflow; the existing workflow already starts at accepted offer.",
      "acceptance_criteria": "1) Replaying the same execution event is idempotent. 2) Existing canonical commands are reused for closing date, inspection deadline, financing deadline, financing type and appraisal applicability. 3) No legal prose is decomposed into Deal columns. 4) Existing workflow instance advances from pns_executed through mark_under_contract; no duplicate instance is created. 5) Failure to map an optional field leaves it unresolved and visible rather than inventing a value. 6) Targeted end-to-end test proves accepted offer → P&S executed → under_contract with canonical facts populated.",
      "dependencies": "CRM-14J, CRM-19, CRM-22, FORMS-01, FORMS-02, DOC-05",
      "created_at": "2026-08-24T12:52:56.493922+00:00",
      "updated_at": "2026-08-24T12:52:56.493922+00:00",
      "completion": 0,
      "rollup": false,
      "planned_start_at": null,
      "actual_start_at": null,
      "completed_at": null,
      "preconditions": null,
      "architect_brief": "CURRENT STATE: workflow_app/runtime.ts already starts RE_supermodel for accepted-offer deals; RE_supermodel-v1.xml already contains pns_preparation → pns_executed → mark_under_contract. Canonical command infrastructure already exists, including deal.set_closing_date, deal.set_inspection_deadline, deal.set_financing_deadline, deal.set_financing_type and deal.set_appraisal_required. PR-PNS.xml already captures the operational fields. Missing seam: no code currently promotes an executed PR-PNS snapshot into those commands or completes pns_executed automatically. Architecture rule: Forms XML describes agreement truth; workflow XML describes obligations created by that agreement; application owns canonical fact derivation and legality.",
      "context_refs": null,
      "postconditions": null,
      "architect_brief_updated_at": null,
      "operating_surface": null
    }
  },
  {
    "story": {
      "id": "CRM-27",
      "workstream": "TXN",
      "title": "Agreement Execution Evidence + Role-Complete Signature Gate",
      "priority": "High",
      "status": "Planned",
      "notes": "Created from 2026-08-24 reconciliation after discovering the current Forms send action submits one signer even though PR-PNS declares Buyer, Seller and Seller Broker signature groups.",
      "batch": null,
      "goal": "Define provider-neutral, role-complete evidence for when a P&S is actually fully executed so workflow progression never equates one successful signature request with an executed agreement.",
      "scope": "Define an application-level executed-agreement predicate over immutable transaction documents/signature evidence. Support BoldSign and manual/external execution evidence. For provider signing, required signature roles come from the template/signature contract and completion means every required role is satisfied. The deal workflow consumes only the neutral fully-executed result, never BoldSign-specific state. Do not make workflow_engine understand signature providers.",
      "acceptance_criteria": "1) A single signer completing does not mark a multi-party P&S fully executed. 2) Required roles are determined from the canonical form/template signature definition or explicit document contract. 3) BoldSign completion is normalized through the existing neutral signature seam. 4) A manually uploaded/external fully executed P&S can satisfy the same predicate with authorized human confirmation/evidence. 5) Fully executed emits or exposes one idempotent application-level trigger consumable by CRM-26. 6) Tests cover partial, complete, declined and replay cases.",
      "dependencies": "DOC-03, DOC-04, DOC-05, FORMS-02, CRM-26",
      "created_at": "2026-08-24T12:52:56.493922+00:00",
      "updated_at": "2026-08-24T12:52:56.493922+00:00",
      "completion": 0,
      "rollup": false,
      "planned_start_at": null,
      "actual_start_at": null,
      "completed_at": null,
      "preconditions": null,
      "architect_brief": "CURRENT STATE: PR-PNS.xml declares BUYER, SELLER and SELLER_BROKER signature groups. app/portal/forms/actions.ts sendFormForSignatureAction currently sends exactly one recipient. lib/signature/reconciliation.ts already consumes neutral SIGNATURE_REQUEST_COMPLETED events and stores the signed artifact. The missing concept is not another provider adapter; it is the application-level definition of FULLY EXECUTED across required roles and across provider/manual evidence. Keep this provider-neutral and document-centric.",
      "context_refs": null,
      "postconditions": null,
      "architect_brief_updated_at": null,
      "operating_surface": null
    }
  },
  {
    "story": {
      "id": "CRM-28",
      "workstream": "TXN",
      "title": "P&S Amendment → Canonical Term Delta + Timer Reschedule",
      "priority": "Medium-High",
      "status": "Planned",
      "notes": "Created from 2026-08-24 reconciliation. Existing PR-PNS-AMD template and deadline reschedule seam make this mostly wiring once CRM-26 execution mapping is proven.",
      "batch": null,
      "goal": "When a signed P&S Amendment changes an operational term, apply only the explicit structured deltas to canonical Deal facts and keep the same workflow instance/timer jobs alive.",
      "scope": "Extend the same execution-projection pattern used by CRM-26 to PR-PNS-AMD. Structured amended dates/terms issue existing canonical commands. Changed closing/inspection/financing dates reconcile the existing pending timer through the existing deadline-timer seam; no workflow restart. Broad amendment prose remains legal document text unless a recurring operational field is explicitly modeled.",
      "acceptance_criteria": "1) Signed amendment is idempotent. 2) Only populated/explicitly changed structured operational fields mutate canonical facts. 3) Same workflow instance survives. 4) Existing timer job is rescheduled rather than duplicated when a canonical date changes. 5) Unstructured amendment prose never silently mutates Deal state. 6) Test proves date extension on an active deal and replay safety.",
      "dependencies": "CRM-26, CRM-14C, CRM-22, FORMS-02",
      "created_at": "2026-08-24T12:52:56.493922+00:00",
      "updated_at": "2026-08-24T12:52:56.493922+00:00",
      "completion": 0,
      "rollup": false,
      "planned_start_at": null,
      "actual_start_at": null,
      "completed_at": null,
      "preconditions": null,
      "architect_brief": "CURRENT STATE: PR-PNS-AMD.xml already exists and exposes amended closingDate plus editable amendment prose. workflow_app/deadline-timer.ts already deterministically reschedules the SAME pending timer job when a canonical date changes. This story should reuse those seams, not create amendment-specific workflow infrastructure. Before implementation, decide whether inspectionDeadline/financingDeadline belong as structured amendment fields; if they are contractual deltas, add them declaratively to the form rather than parse prose.",
      "context_refs": null,
      "postconditions": null,
      "architect_brief_updated_at": null,
      "operating_surface": null
    }
  },
  {
    "story": {
      "id": "OPS-11A",
      "workstream": "ADMIN",
      "title": "Deal Attention Projection / Red Lights",
      "priority": "High",
      "status": "Planned",
      "notes": "Child of OPS-11 created from 2026-08-24 workflow architecture reconciliation. Reuse CRM-10 and workflow read projections; this is business observability, not engine diagnostics.",
      "batch": null,
      "goal": "Produce one deterministic business-attention projection that answers what can hurt a deal today without exposing workflow-engine internals.",
      "scope": "Project actionable deal items from canonical tasks, workflow active nodes, blockers, canonical deadlines, signature/document readiness and failed external work. Include deal, title, severity, owner/responsibility, due/age, source, reason and drill-down target. Derive due-soon/overdue warnings from existing contractual dates; do not create extra timer jobs merely for warning thresholds.",
      "acceptance_criteria": "1) Projection contains only business-actionable items. 2) Due-soon and overdue are deterministic from canonical dates/current state. 3) Workflow blocker/readiness information comes from existing definition-driven read services/facts. 4) No shadow deal status columns are introduced. 5) Items link to the correct deal/task/document/signature surface. 6) Tests cover healthy, due-soon, overdue, blocked-documents and failed-external cases.",
      "dependencies": "OPS-11, CRM-10, CRM-14H, CRM-21, CRM-22",
      "created_at": "2026-08-24T12:52:56.493922+00:00",
      "updated_at": "2026-08-24T12:52:56.493922+00:00",
      "completion": 0,
      "rollup": false,
      "planned_start_at": null,
      "actual_start_at": null,
      "completed_at": null,
      "preconditions": null,
      "architect_brief": "A large portion already exists for free: workflow_app/read-service.ts exposes currentNodes, active/completed milestones, blockerCount, responsibleParty, nextExpectedAction, openTaskCount and pendingTimerCount; CRM-10 already covers overdue/due-soon/open-work concepts. This child composes canonical truth into the red-lights page. Warning thresholds belong in the attention projection/watchdog, not as duplicate workflow timers.",
      "context_refs": null,
      "postconditions": null,
      "architect_brief_updated_at": null,
      "operating_surface": null
    }
  },
  {
    "story": {
      "id": "OPS-11B",
      "workstream": "ADMIN",
      "title": "Attention ACK / NACK / RESOLVED + Escalation Watchdog",
      "priority": "High",
      "status": "Planned",
      "notes": "Child of OPS-11 created from 2026-08-24 architecture session. Preserve the key invariant: acknowledging attention never satisfies the underlying business gate.",
      "batch": null,
      "goal": "Add a small operational attention protocol so the system can repeatedly watch unresolved risk while humans acknowledge ownership, report blockage, or confirm the underlying issue is resolved.",
      "scope": "Define attention lifecycle independently from business-gate lifecycle. Support ACK (seen/owned), NACK (blocked/escalate), RESOLVED (underlying canonical task/fact/evidence actually resolved). Add bounded polling/watchdog evaluation that re-emits/escalates attention as thresholds are crossed while the business gate remains unresolved. Reuse canonical tasks/facts as business truth; do not introduce a second workflow engine.",
      "acceptance_criteria": "1) ACK never advances a workflow gate. 2) NACK preserves unresolved business state and raises severity/escalation. 3) RESOLVED must route through the canonical task/fact command that actually satisfies the business condition. 4) Poller/watchdog is idempotent and bounded. 5) Repeated evaluations do not create duplicate active attention items. 6) Tests prove ACK-then-overdue escalation and true resolution stopping alerts.",
      "dependencies": "OPS-11A, CRM-14F, CRM-14J",
      "created_at": "2026-08-24T12:52:56.493922+00:00",
      "updated_at": "2026-08-24T12:52:56.493922+00:00",
      "completion": 0,
      "rollup": false,
      "planned_start_at": null,
      "actual_start_at": null,
      "completed_at": null,
      "preconditions": null,
      "architect_brief": "Model two independent dimensions: business gate = pending/satisfied/failed/waived/unresolved; attention = unacknowledged/acknowledged/escalated/resolved. Existing workflow timers handle contractual deadline firing; the watchdog handles human attention before/after those gates and may be driven by a Vercel Cron or operational worker. Do not overload canonical task completion to mean ACK.",
      "context_refs": null,
      "postconditions": null,
      "architect_brief_updated_at": null,
      "operating_surface": null
    }
  },
  {
    "story": {
      "id": "OPS-11C",
      "workstream": "ADMIN",
      "title": "Attention Ownership + Subscriber Delivery",
      "priority": "Medium-High",
      "status": "Planned",
      "notes": "Child of OPS-11 created from 2026-08-24 architecture reconciliation. The task table already has assigned_user_id and workflow responsibility metadata already resolves abstract roles; wire those before inventing new ownership models.",
      "batch": null,
      "goal": "Route actionable attention to the correct brokerage owner/subscribers and deliver it through existing communication capabilities without coupling workflow state to a transport.",
      "scope": "Resolve workflow responsibility hints to operational ownership using existing deal participants and broker assignment. Populate/reuse canonical task assigned_user_id where appropriate. Define subscriber resolution for deal owner/broker and optional participants. Send notifications through an application notification seam; email is V1 primary transport, with existing messaging capabilities reusable later. Delivery failure is attention state, not a workflow gate.",
      "acceptance_criteria": "1) Brokerage-owned workflow tasks resolve to the responsible app user when known. 2) External-SME responsibility remains visible without pretending the external party is an app user. 3) Notification recipients are derived from canonical deal/user/participant data. 4) Email delivery can fail/retry without changing business-gate truth. 5) No provider-specific notification logic enters workflow_engine. 6) Tests prove owner resolution and delivery idempotency.",
      "dependencies": "OPS-11A, OPS-11B, CRM-13, CRM-14J, CRM-25",
      "created_at": "2026-08-24T12:52:56.493922+00:00",
      "updated_at": "2026-08-24T12:53:41.087143+00:00",
      "completion": 0,
      "rollup": false,
      "planned_start_at": null,
      "actual_start_at": null,
      "completed_at": null,
      "preconditions": null,
      "architect_brief": "CURRENT STATE: task has assigned_user_id, but workflow_app/task-materialization.ts currently creates workflow-derived canonical tasks without assigning it. workflow_app/responsibility.ts already resolves XML responsibility hints to brokerage/client/seller/SME classes and deal_participant targets. Therefore ownership metadata largely exists; the missing work is binding brokerage responsibility to the actual app user and projecting subscribers. Email should be a delivery adapter, not business state.\n\nREUSE CRM-25 for durable outbound action/correlation rather than creating a second notification queue. OPS-11C owns subscriber/recipient resolution and business notification policy; CRM-25 owns provider-neutral durable outbound delivery/correlation mechanics.",
      "context_refs": null,
      "postconditions": null,
      "architect_brief_updated_at": null,
      "operating_surface": null
    }
  }
]
```

## Successfully Completed Stories with Architecture Records

Five completed stories were selected: DOC-01 through DOC-05.

```json
[
  {
    "story": {
      "id": "DOC-01",
      "workstream": "TXN",
      "title": "Canonical Transaction Document Model",
      "priority": "High",
      "status": "Complete",
      "notes": "Canonical transaction document model delivered (commit 33ad8b8; migration 027; db/transaction-document.ts). transaction_document is the DEAL-scoped canonical record: document_type/state/source are checked structural categories and document_type_label carries the SME long tail (mirrors deal_participant.role_label). File bytes live in the generic media asset store (media_type document): media.id is the draft/current bytes, signed_media_id + signed_at are the signed artifact, set together by CHECK and always a NEW media row — the draft bytes are never mutated. Source idempotency via a partial unique index on (deal_id, source_system, source_external_id) where source_external_id is not null. State transitions reuse the claim-first command-receipt pattern (migration 018): the same commandId executes its effect at most once and losers observe the stored winner result. No legacy property-scoped document migration, no workflow_engine involvement. Targeted tests: workflow_app/tests/transaction-document.test.ts green.",
      "batch": null,
      "goal": "Define the canonical transaction-document record that links a document to a deal (the transaction), classifies it by document type, tracks its lifecycle state, ownership and source/provenance, and preserves signed-artifact lineage — without embedding any signature-provider concern (DOC-03/DOC-04 own providers).",
      "scope": null,
      "acceptance_criteria": "The implementation worker must prove: (1) an additive migration adds transaction_document with deal_id, document_type(+label), state, source(+source_system/source_external_id), ownership (prepared_by_user_id, party_person_id), media_id, signed_media_id, signed_at, supersedes_document_id — CHECK constraints on type/state/source, indexes on (deal_id) and (deal_id, state), and a unique partial index (deal_id, source_system, source_external_id) where source_external_id is not null; (2) repository create/transition/read functions use command-receipt idempotency so a pending receipt never mutates state and replay returns the stored outcome; (3) no column/table is added to workflow_engine and no provider (BoldSign) column exists; (4) media and property_media are unchanged and property documents are unaffected; (5) signing a document stores a NEW media row and sets signed_media_id + signed_at without mutating the draft bytes; (6) automated tests cover type/state/source validation, deal scoping, idempotent transition, and signed-lineage integrity.",
      "dependencies": "Reuses existing primitives: media (002/003/004) for file bytes, deal (001) as the transaction identity, app_user/person and deal_participant (012) for ownership/party, and the claim-first command-receipt idempotency pattern (018, db/workflow-command-receipt.ts, db/deal-stage.ts setDealStage). No dependency on DOC-03/DOC-04.",
      "created_at": "2026-08-21T07:04:18.701955+00:00",
      "updated_at": "2026-08-24T01:38:26.717132+00:00",
      "completion": 100,
      "rollup": true,
      "planned_start_at": null,
      "actual_start_at": "2026-08-21T16:04:40.295447+00:00",
      "completed_at": "2026-08-24T01:38:26.717132+00:00",
      "preconditions": "- media stays the generic reusable asset store (media_type 'document'); it is NOT extended with transaction semantics.\n- deal.id (uuid) is the canonical transaction identity; a transaction document is deal-scoped.\n- Existing property-scoped documents (property_media role 'document') are unchanged and are NOT migrated into the transaction-document model.\n- workflow_engine remains domain-neutral; DOC-01 lives in the application/domain layer (db/ + workflow_app integration), never in workflow_engine, and never reuses the generic engine jobs table.",
      "architect_brief": "Proposed canonical model — one additive table transaction_document (deal-scoped; migration 027+):\n\ntransaction_document (\n  id uuid PK default gen_random_uuid(),\n  deal_id uuid NOT NULL references deal(id) on delete cascade,      -- the transaction\n  document_type text NOT NULL check (document_type in ('agreement','addendum','disclosure','title','financing','inspection','appraisal','closing','other')),\n  document_type_label text check (char_length <= 120),               -- long tail (mirrors deal_participant.role_label)\n  title text,\n  state text NOT NULL check (state in ('draft','ready','sent','signed','voided','superseded')),\n  source text NOT NULL check (source in ('upload','generated','imported','provider')),\n  source_system text,\n  source_external_id text,\n  prepared_by_user_id uuid references app_user(id),                  -- ownership (internal author)\n  party_person_id uuid references person(id),                        -- ownership (client/seller party)\n  media_id uuid references media(id),                                -- draft/current stored bytes\n  signed_media_id uuid references media(id),                         -- signed artifact (lineage; draft never mutated)\n  signed_at timestamptz,\n  supersedes_document_id uuid references transaction_document(id),   -- optional version lineage\n  created_at timestamptz default now(), updated_at timestamptz default now()\n);\nindexes: (deal_id), (deal_id, state); unique partial index (deal_id, source_system, source_external_id) where source_external_id is not null.\n\nBoundaries — transaction_document is application/domain data (db/ repository + workflow_app integration). It never enters workflow_engine; the generic engine jobs table is unrelated and not reused. Provider-specific signing (BoldSign) is excluded: DOC-01 reserves only signed-artifact lineage (signed_media_id, signed_at), never provider columns.\n\nReuse — file bytes stay in media (media_type 'document'); the canonical record references media.id. Ownership/parties reuse app_user/person/deal_participant. State transitions reuse claim-first command-receipt idempotency (claimReceipt/finalizeReceipt/replayOutcome, setDealStage) so retries/replays never double-apply.\n\nRejected alternatives — (1) extending media with document_type/state/ownership: pollutes the generic asset store shared by images/videos/properties/guides; (2) reusing property_media for deal documents: it owns property-scoped roles/ordering, not a document lifecycle; (3) a standalone document table with no deal link: a transaction document is deal-scoped; (4) embedding BoldSign/provider fields now: leaks provider concerns into the canonical model.\n\nImplementation sequence — DOC-01 (this model + repository + invariant tests) -> DOC-02 transaction packet (required document_types per deal stage/financing_type) -> DOC-03 provider-neutral signature seam -> DOC-04 BoldSign adapter -> DOC-05 signed-document reconciliation (signed artifact -> state=signed, signed_media_id, signed_at).\n\nKnown risks — closed document_type/state enums vs the long tail (mitigate with label/title + application-curated categories); BYTEA storage for large signed PDFs (existing media pattern: bytes in media, metadata in transaction_document); cross-row transition ordering cannot be enforced by a Postgres CHECK (enforce in the application command layer, like offer/deal_stage); reconciliation dedup (use command-receipt + the source idempotency key so webhook replays never duplicate signed documents).",
      "context_refs": "Inspected: db/migrations/001_initial_schema.sql (deal, person, app_user, interaction, task); 002_media.sql, 003_property_domain.sql, 004_document_media.sql (media + property_media roles, media_type document); 012_deal_participant.sql (party/ownership pattern), 013_showing.sql, 014_offer.sql, 020_deal_financing_type.sql; 018_workflow_command_receipt.sql + db/workflow-command-receipt.ts + db/deal-stage.ts (claim-first command idempotency); db/deals.ts, db/dossier.ts (no existing deal-document association); db/media-admin.ts (media audit projection); scripts/import-property-document.mjs (property document ingestion into media + property_media role=document); app/api/media/documents/[id]/route.ts (document serving); components/property/property-documents.tsx + lib/property-types.ts (PropertyDocument shape); workflow_engine/ARCHITECTURE_BOUNDARY.md + workflow_engine/scripts/schema.sql (generic jobs table, not to conflate); workflow_app/tests/command-receipt.test.ts (idempotency semantics); Story Board rows DOC-01..DOC-05.",
      "postconditions": "A canonical deal-scoped transaction_document record exists with type/state/ownership/source and signed-artifact lineage; media remains the generic asset store and property documents are untouched; DOC-02 packet, DOC-03 provider seam, DOC-04 BoldSign, and DOC-05 reconciliation all build on transaction_document without reworking the core model; workflow_engine remains domain-neutral with no document concept leaked in.",
      "architect_brief_updated_at": "2026-08-21T15:52:16.53651+00:00",
      "operating_surface": "NEXUS"
    }
  },
  {
    "story": {
      "id": "DOC-02",
      "workstream": "TXN",
      "title": "Transaction Packet",
      "priority": "High",
      "status": "Complete",
      "notes": "Transaction packet delivered (commit b0fdaec; workflow_app/transaction-packet.ts). A pure DERIVED projection with no packet table: given deal.stage plus the deal/workflow facts it determines which transaction document types are required, compares against the canonical transaction_document rows, and reports per deal present / missing / unresolved. Signature state is irrelevant to presence (any non-terminal document state counts as present), the packet never writes (no auto-creation), workflow_engine is untouched, and every jurisdiction knob is an input fact so the rule catalog stays jurisdiction-neutral. An unresolved gating fact is surfaced as unresolved — never coerced to a boolean and never fabricated into a required document. Long-tail requirements (CRIM clearance, HOA clearance, survey, closing statement, registry follow-up) reuse the other/closing categories plus curated labels matched exactly. Targeted tests: workflow_app/tests/transaction-packet.test.ts green; consumed by closing-document readiness (CRM-21).",
      "batch": null,
      "goal": "Define how a transaction packet determines the required documents for a deal from deal/workflow state and financing/deal characteristics, building on the DOC-01 transaction_document model — with no signing-provider concerns.",
      "scope": null,
      "acceptance_criteria": "The implementation worker must prove: (1) requiredTransactionDocumentTypes returns a deterministic required-document set from deal stage + facts; (2) unresolved Class C facts produce \"unresolved\", never a fabricated required document; (3) a packet completeness projection lists present/missing/unresolved per deal without auto-creating documents; (4) no signing/signature concept is introduced; (5) the mapping reuses transaction_document and does not duplicate it; (6) unit + SCOPED verification: targeted + directly adjacent tests only; typecheck/build only when warranted; no FULL regression or broad persistence harness.",
      "dependencies": "db/transaction-document.ts + db/migrations/027_transaction_document.sql (DOC-01 canonical model); workflow_app/facts.ts (DealWorkflowFacts + Class A/B/C fact classification); workflow_app/configuration.ts (CULEBRA_JURISDICTION_CONFIG jurisdiction defaults); workflow_app/definitions/RE_supermodel-v1.xml (deal stages/gates); db/deal-financing.ts (financing_type); db/deal.ts (stage).",
      "created_at": "2026-08-21T07:04:18.701955+00:00",
      "updated_at": "2026-08-24T01:38:26.717132+00:00",
      "completion": 100,
      "rollup": true,
      "planned_start_at": null,
      "actual_start_at": "2026-08-22T06:50:34.975767+00:00",
      "completed_at": "2026-08-24T01:38:26.717132+00:00",
      "preconditions": "- DOC-01 transaction_document is the canonical deal-scoped document record (document_type/state/source/ownership/signed lineage).\n- A packet must be DERIVED from canonical deal facts; unresolved Class C facts must not force documents and must not be invented.",
      "architect_brief": "CURRENT-STATE. transaction_document holds the canonical documents; there is no \"packet\" concept that maps required document types to a deal's current state. Deal facts (facts.ts) expose stage, financingApplicable, closingDateScheduled, and jurisdiction config (requiresNotario, requiresTitleCompany, requiresCrimClearance, requiresRegistryFollowup, requiresHoaClearance, requiresSurvey, inspectionApplicable, insuranceApplicable, appraisalApplicable [Class C unresolved]).\n\nRECOMMENDED MODEL. A packet is a PURE, DERIVED projection — no new required table, optionally a small editable catalog. Define a pure function requiredTransactionDocumentTypes(dealStage, facts) → the required document_type (+ document_type_label) set, then a completeness query over transaction_document that returns, per deal, which required types are present / missing / unresolved. The packet never auto-creates documents; creation is an explicit user/application action or a later DOC-05 reconciliation. The packet only answers \"what is required vs present\".\n\nMAPPING SKETCH. agreement (P&S/listing) at offer→under_contract; disclosure + title for title_work; financing documents gated by financingApplicable; inspection/appraisal gated by their applicability; tax/CRIM clearance gated by requiresCrimClearance; closing_documents + closing statement at closing; recording/registry follow-up gated by requiresRegistryFollowup; HOA/survey gated by their flags. Unresolved Class C facts (appraisalApplicable null) → do NOT emit a required document; mark \"unresolved\" and surface it to the operator — never invent.\n\nREPRESENTATION. Prefer a pure function (workflow_app or lib) over a new table. If operators must edit the required-document list, add a small deal-independent catalog table (stage + condition → document_type); start without it.\n\nBOUNDARY. DOC-02 is application/domain (workflow_app/lib), no signing, no workflow_engine. Reuse transaction_document; the XML node id remains the workflow state identity.\n\nREJECTED. (1) a packet table that duplicates transaction_document; (2) embedding signing/signature state; (3) inventing documents for unresolved facts; (4) hardcoding a single jurisdiction.\n\nIMPLEMENTATION SEQUENCE. (1) pure required-doc-type mapping + unit tests; (2) packet completeness projection (present/missing/unresolved) per deal; (3) optional editable catalog only if required; (4) portal surface (later, bounded).\n\nRISKS. Unresolved Class C facts must not block the packet (surface, don't block); long-tail document types (use document_type_label); the packet is derived and recomputed on stage/fact change (no stored staleness).",
      "context_refs": "db/transaction-document.ts (DOC-01: document_type/state/source/ownership/signed lineage); db/migrations/027_transaction_document.sql; workflow_app/facts.ts (DealWorkflowFacts: stage, financingApplicable, closingDateScheduled, requiresNotario/TitleCompany/CrimClearance/RegistryFollowup/HoaClearance/Survey, inspectionApplicable, insuranceApplicable, appraisalApplicable Class C); workflow_app/configuration.ts (jurisdiction defaults); workflow_app/definitions/RE_supermodel-v1.xml (offer_accepted→under_contract→title/tax/funds/closing/recording); db/deal-financing.ts + db/deal-stage.ts (stage/financing facts).",
      "postconditions": "A transaction packet is a derived, deterministic projection over transaction_document + deal facts; required documents are computed from stage/financing/jurisdiction without inventing unresolved facts; DOC-03 (signature seam) and DOC-05 (reconciliation) can build on this packet + DOC-01 without rework.\n\n- DOC-02 introduces no signing/signature state.",
      "architect_brief_updated_at": "2026-08-21T16:30:52.492215+00:00",
      "operating_surface": "NEXUS"
    }
  },
  {
    "story": {
      "id": "DOC-03",
      "workstream": "HARDEN",
      "title": "Signature Provider Seam",
      "priority": "Medium-High",
      "status": "Complete",
      "notes": "Signature Provider Seam delivered (commit c4d3939; migration 036; db/signature-request.ts + lib/signature). signature_request is the canonical PROVIDER-NEUTRAL record of a signing request against a transaction document: the neutral status model requested -> sent -> viewed -> signed -> completed with declined/voided/expired/error sinks; provider ids and provider state never reach this table (a DOC-04 provider table lives behind the seam). lib/signature owns the SignatureProvider interface (send/status/cancel/verifyWebhook), the provider registry, the fake provider, the neutral status mapping, the application router, and the signature.request.send / signature.request.status commands in the canonical command registry. Domain services never call the provider — provider observations arrive as neutral statuses mapped at the seam. Claim-first receipt idempotency exactly like DOC-01. Targeted tests: workflow_app/tests/signature-seam.test.ts green.",
      "batch": null,
      "goal": "Define the provider-neutral signing boundary (commands, contracts, events, status mapping) on top of DOC-01/DOC-02, so provider-specific signing concerns (BoldSign) stay behind a seam and out of canonical domain models.",
      "scope": null,
      "acceptance_criteria": "The implementation worker must prove: (1) a neutral SignatureProvider interface and command set exist with a neutral status model; (2) a canonical signature_request record references transaction_document and carries NO provider-specific fields; (3) send/status/cancel are idempotent (replay-safe) via receipts; (4) provider status maps to neutral status at the seam; (5) neutral events (sent/completed/declined/voided) carry correlation/causation; (6) transaction_document is untouched by intermediate provider state; (7) a fake provider proves the seam end-to-end; (8) SCOPED verification: targeted + directly adjacent tests only; typecheck/build only when warranted; no FULL regression or broad persistence harness.",
      "dependencies": "db/transaction-document.ts (DOC-01); DOC-02 transaction packet; lib/workflow/contracts.ts (CommandEnvelope/CommandResult/DomainEvent shapes); db/workflow-command-receipt.ts (idempotency); lib/workflow/command-inventory.ts (DomainCommand/idempotency-class pattern); docs/workflow-integration-contract.md (command seam).",
      "created_at": "2026-08-21T07:04:18.701955+00:00",
      "updated_at": "2026-08-24T01:38:26.717132+00:00",
      "completion": 100,
      "rollup": true,
      "planned_start_at": null,
      "actual_start_at": "2026-08-22T08:52:20.018845+00:00",
      "completed_at": "2026-08-24T01:38:26.717132+00:00",
      "preconditions": "- DOC-01 transaction_document is canonical; DOC-02 packet determines required documents.\n- The provider-neutral seam must exist before any BoldSign adapter.",
      "architect_brief": "RECOMMENDED MODEL. A provider-neutral signing seam at the application boundary, shaped like the existing workflow command seam (lib/workflow/contracts.ts + command-receipt idempotency).\n\nCONTRACTS/COMMANDS (neutral): signature.request.send (input: transactionDocumentId, recipients[] with neutral roles, signer ordering, optional message); signature.request.status (input: signatureRequestId); signature.request.cancel; signature.request.decline. All return a CommandResult and are idempotent via claim-first receipts (a send for the same transaction_document + active request returns the existing request, never a duplicate).\n\nPROVIDER INTERFACE: a neutral SignatureProvider interface — send(request), status(requestId), cancel(requestId), verifyWebhook(payload, signature) → {event, signatureRequestId}. Provider adapters (DOC-04 BoldSign) implement it; the router dispatches by configured provider, never by provider-specific command.\n\nNEUTRAL STATUS MODEL: requested → sent → viewed → signed → completed | declined | voided | expired | error. Store this neutral status on a canonical `signature_request` record that references transaction_document_id (NOT inside transaction_document). Provider-specific IDs/state live in a separate provider table (DOC-04) and never in transaction_document.\n\nEVENTS: signature.request.sent / .completed / .declined / .voided, carrying eventId/correlation/causation reusing the existing DomainEvent shape, so downstream consumers (DOC-05 reconciliation) subscribe to neutral events, not provider webhooks.\n\nSTATUS-MAPPING BOUNDARY: the seam maps provider status → neutral status; transaction_document reflects only the FINAL signed outcome via DOC-05 reconciliation (signed_media_id, signed_at) — never intermediate provider state.\n\nBOUNDARY. Canonical domain models stay provider-free; provider adapters sit behind SignatureProvider; command receipts give idempotent send/status; the application owns authority/validation.\n\nREJECTED. (1) BoldSign fields on transaction_document; (2) synchronous provider calls inside a domain service; (3) per-provider commands in the router; (4) webhook handlers writing straight to transaction_document.\n\nIMPLEMENTATION SEQUENCE. (1) neutral contracts + status model + signature_request table (migration); (2) SignatureProvider interface + a fake/test provider; (3) command-receipt idempotency + neutral events; (4) DOC-04 adapter plugs in behind the seam.\n\nRISKS. Provider semantics leaking through webhook payloads (normalize at the seam); status-model drift across providers; duplicate sends (enforce one active request per transaction_document).\n\nARCHITECTURE ADDENDUM 2026-08-22 — DOCUMENT HANDOFF: Forms/repository must consume this existing provider-neutral seam rather than invent a second signing integration. Send/sign operates on the exact immutable issued PDF version. Provider completion stores the returned signed PDF as a new immutable artifact associated with that same document version, with provider/audit evidence. Any business-term change requires a new document version and a new signature request; never regenerate or replace the PDF already sent for signature.\n\nSIGNATURE HANDOFF 2026-08-22: form issuance creates the exact immutable unsigned PDF artifact. DOC-03/04/05 must send that exact artifact to the provider-neutral signature seam, preserve the unsigned artifact/checksum, append the returned signed PDF and provider audit evidence as new immutable artifacts, and never regenerate or mutate the issued version after signature dispatch. Participant/signature-role structure must support multiple buyers, sellers, brokers and future lawyer/notary roles without hardcoded buyer1/buyer2 columns.",
      "context_refs": "db/transaction-document.ts + db/migrations/027_transaction_document.sql (DOC-01: document_type/state/source/ownership/signed lineage); db/workflow-command-receipt.ts (claim/finalize/replay idempotency); lib/workflow/contracts.ts (CommandEnvelope/CommandResult/DomainEvent/correlation-causation); lib/workflow/command-inventory.ts (DomainCommand idempotency classes); docs/workflow-integration-contract.md (command seam + subject model); workflow_app/tests/command-receipt.test.ts (idempotency precedent).",
      "postconditions": "A provider-neutral signing seam exists (contracts, signature_request record, neutral status model, events); BoldSign concepts are isolated behind the seam; DOC-04 can implement the adapter without touching canonical models; DOC-05 can reconcile signed artifacts from neutral events.\n\n- No provider-specific concepts (envelope IDs, provider roles, webhook payloads) may enter canonical domain models.",
      "architect_brief_updated_at": "2026-08-21T16:31:33.769376+00:00",
      "operating_surface": "TECH"
    }
  },
  {
    "story": {
      "id": "DOC-04",
      "workstream": "HARDEN",
      "title": "BoldSign Integration",
      "priority": "Medium-High",
      "status": "Complete",
      "notes": "BoldSign Integration delivered (commit e8616e5; migration 037; db/bold-sign-request.ts + lib/signature/boldsign). bold_sign_request holds EVERYTHING BoldSign-specific behind the DOC-03 seam: the envelope id, provider document/file ids, the last RAW BoldSign status, and an observable last_error with retryable/non-retryable classification; provider ids never cross to signature_request or transaction_document. Send idempotency: one row per canonical request plus a partial unique index on envelope_id, so a provider envelope is never persisted twice. bold_sign_webhook_event is the webhook replay dedupe (unique provider_event_id) AND the durable enqueue record for the DOC-05 async reconciler; a replayed webhook inserts nothing (ON CONFLICT DO NOTHING). lib/signature/boldsign (adapter/client/config/errors/events/webhook) composes provider calls with these writes; raw statuses map to the neutral model only at the DOC-03 seam (lib/signature/status-mapping.ts). Targeted tests: workflow_app/tests/boldsign-adapter.test.ts green.",
      "batch": null,
      "goal": "Analyze the concrete BoldSign adapter responsibilities — send, status, completion, webhook handling, retries/idempotency, provider IDs, error handling, credentials — assuming the DOC-03 provider seam is the boundary, without leaking BoldSign semantics into transaction_document.",
      "scope": null,
      "acceptance_criteria": "The implementation worker must prove: (1) the adapter implements DOC-03 SignatureProvider with send/status/cancel/webhook verification; (2) provider ids/state live only in bold_sign_request, never in transaction_document or signature_request; (3) send is idempotent (no duplicate envelopes) and webhook handling dedupes replays; (4) provider status maps to neutral status and completion emits a neutral event; (5) credentials come from config/env and are never hardcoded or logged; (6) errors map to neutral 'error' with retryable classification; (7) a fake BoldSign server exercises send/status/webhook/replay/error; (8) SCOPED verification: targeted + directly adjacent tests only; typecheck/build only when warranted; no FULL regression or broad persistence harness.",
      "dependencies": "DOC-03 provider seam (SignatureProvider interface, signature_request record, neutral status/events); DOC-01 transaction_document; db/workflow-command-receipt.ts (idempotency); lib/workflow/contracts.ts (DomainEvent); configuration/env (credentials); db/migrations for a provider-specific table.",
      "created_at": "2026-08-21T07:04:18.701955+00:00",
      "updated_at": "2026-08-24T01:38:26.717132+00:00",
      "completion": 100,
      "rollup": true,
      "planned_start_at": null,
      "actual_start_at": "2026-08-22T09:11:49.279388+00:00",
      "completed_at": "2026-08-24T01:38:26.717132+00:00",
      "preconditions": "- DOC-03 seam exists and is the only boundary the adapter touches.\n- BoldSign credentials/configuration come from env/config — never hardcoded.",
      "architect_brief": "RECOMMENDED. A BoldSign adapter module implementing the DOC-03 SignatureProvider, owning ALL provider specifics; the seam and canonical models see only neutral types.\n\nRESPONSIBILITIES. (1) send — map the neutral signature request to a BoldSign envelope/template; persist provider envelope id + document ids in a provider-specific table (bold_sign_request) keyed by signature_request_id (one row per request); read credentials from config/env. (2) status — poll/query BoldSign envelope status and map BoldSign's status enum to the neutral status (requested/sent/viewed/signed/completed/declined/voided/expired/error). (3) completion — on completed, emit the neutral signature.request.completed event (DOC-05 reconciles the signed artifact). (4) webhook — verify the BoldSign webhook signature (HMAC), normalize the payload to a neutral event, dedupe by provider event id (unique key), acknowledge fast and reconcile asynchronously. (5) retries/idempotency — send idempotent via command receipt + provider document id unique key; webhook idempotent via provider event id; transient provider errors retried with capped exponential backoff and timeouts. (6) provider IDs — stored ONLY in bold_sign_request, never in transaction_document. (7) error handling — map provider errors to neutral 'error' status with retryable/non-retryable classification and observable last_error. (8) credentials/config — BOLDSIGN_API_KEY, BOLDSIGN_BASE_URL, BOLDSIGN_TEMPLATE_ID, BOLDSIGN_WEBHOOK_SECRET from config/env; never hardcoded, never logged.\n\nBOUNDARY. BoldSign types/strings are confined to the adapter; the seam emits only neutral events; transaction_document is untouched by intermediate provider state; signature_request stays provider-free.\n\nREJECTED. (1) provider fields on transaction_document or signature_request; (2) webhook writing straight to transaction_document; (3) hardcoded credentials; (4) synchronous blocking webhook handling; (5) per-provider commands in the router (dispatch via DOC-03 seam).\n\nIMPLEMENTATION SEQUENCE. (1) config + typed BoldSign client wrapper; (2) send/status/cancel adapter + bold_sign_request table; (3) webhook verification + normalization + idempotent enqueue; (4) status polling + completion event emission; (5) tests against a fake BoldSign server (send/status/webhook/replay/error); (6) SCOPED verification: targeted + directly adjacent tests only; typecheck/build only when warranted; no FULL regression or broad persistence harness.\n\nRISKS. Webhook replay (dedupe by provider event id); webhook signature verification correctness; BoldSign API version drift; secret handling/logging; retry storms (cap attempts + backoff); provider id leakage into canonical rows.",
      "context_refs": "DOC-03 seam (SignatureProvider + neutral status/events); db/transaction-document.ts + db/migrations/027 (DOC-01, to be kept provider-free); db/workflow-command-receipt.ts (claim/finalize/replay); lib/workflow/contracts.ts (DomainEvent); workflow_app/configuration.ts (config pattern); docs/workflow-integration-contract.md; workflow_app/tests/command-receipt.test.ts (idempotency precedent); docs/auth-security-model.md (secret handling posture).",
      "postconditions": "A BoldSign adapter sits behind the DOC-03 seam, owning send/status/completion/webhook/retries/idempotency/provider IDs/errors/credentials; canonical domain models remain provider-free; DOC-05 can reconcile signed artifacts from neutral completion events.\n\n- Provider-specific state is isolated from canonical domain models (transaction_document, signature_request).",
      "architect_brief_updated_at": "2026-08-21T16:32:13.965846+00:00",
      "operating_surface": "TECH"
    }
  },
  {
    "story": {
      "id": "DOC-05",
      "workstream": "TXN",
      "title": "Signed Document Reconciliation",
      "priority": "Medium",
      "status": "Complete",
      "notes": "Signed Document Reconciliation delivered (commit a49fca9; db/signature-reconciliation.ts + lib/signature/reconciliation.ts). A neutral signature.request.completed event reconciles into canonical transaction_document: the signed artifact is downloaded ONCE via the DOC-04 adapter (an external side effect that runs OUTSIDE the transaction, before the claim) into a NEW media row, then the document moves sent -> signed through the DOC-01 transition with signed_media_id/signed_at set; the draft media row is never mutated. Idempotency: a claim-first receipt keyed by the neutral event id (signature.reconcile:<eventId>) makes a replay return replayed:true with no re-download, no duplicate media and no double transition; provider-webhook dedupe happens at DOC-04 before any neutral event exists; the already-signed guard replays; a partial failure rolls back and the retry reconciles exactly once. The domain service uses DOC-01 transitions and never writes provider/signature state to canonical tables. Targeted tests: workflow_app/tests/signature-reconciliation.test.ts green.",
      "batch": null,
      "goal": "Define how completed signed artifacts reconcile into canonical transaction_document state safely and idempotently, building on DOC-01 signed-artifact lineage and the DOC-03/DOC-04 provider flow — preserving draft/original bytes, appending signed artifacts, and handling replay/webhook duplication.",
      "scope": null,
      "acceptance_criteria": "The implementation worker must prove: (1) a completed event appends a NEW signed media row and sets signed_media_id/signed_at with the state transition sent→signed; (2) the original/draft media row is preserved byte-for-byte; (3) a replayed completed event is a no-op (no duplicate signed media, no double transition, replayed:true); (4) webhook duplication (same provider event id twice) reconciles exactly once; (5) out-of-order completion resolves correctly; (6) partial failure (media created, transition failed) recovers idempotently; (7) no provider/signature state lands on transaction_document; (8) SCOPED verification: targeted + directly adjacent tests only; typecheck/build only when warranted; no FULL regression or broad persistence harness.",
      "dependencies": "db/transaction-document.ts (DOC-01: signed_media_id/signed_at lineage + state transitions); DOC-03 (signature_request + neutral events); DOC-04 (webhook dedupe + signature.request.completed event + artifact download); db/workflow-command-receipt.ts (idempotency); media abstraction (media + property_media for document roles).",
      "created_at": "2026-08-21T07:04:18.701955+00:00",
      "updated_at": "2026-08-24T01:38:26.717132+00:00",
      "completion": 100,
      "rollup": true,
      "planned_start_at": null,
      "actual_start_at": "2026-08-22T09:32:37.313697+00:00",
      "completed_at": "2026-08-24T01:38:26.717132+00:00",
      "preconditions": "- DOC-01 established signed-artifact lineage (a new media row + transaction_document.signed_media_id/signed_at).\n- DOC-03/DOC-04 produce a neutral signature.request.completed event carrying the signed artifact reference (provider doc id).",
      "architect_brief": "RECOMMENDED. A reconciliation handler that subscribes to NEUTRAL signature.request.completed events — never provider webhooks directly. For each completed signature request:\n1. IDEMPOTENCY CLAIM: claim a command receipt keyed by the neutral event id (and provider event id as the unique source key) so replays are no-ops.\n2. RESOLVE: load the transaction_document via signature_request.transaction_document_id; verify it exists and is in a signable state (draft/sent); if already signed (signed_media_id set), treat as replayed and return.\n3. APPEND SIGNED ARTIFACT: create a NEW media row (document/pdf) holding the signed bytes (downloaded once via the DOC-04 adapter), leaving the original/draft media row untouched; then set transaction_document.signed_media_id = new media id and signed_at = now() through DOC-01's transition (state sent→signed), version increment.\n4. PRESERVE ORIGINAL BYTES: never mutate or delete the draft/original media row; the signed artifact is a distinct media row forming the lineage chain original → signed.\n5. REPLAY/WEBHOOK DUPLICATION: dedupe via provider event id + command receipt; a replayed completed event reads the final receipt and returns replayed:true without re-appending (no duplicate signed media, no double transition).\n6. ORDERING: tolerate out-of-order completion (completed before send status was observed) by resolving the signature_request directly and treating completed as terminal regardless of intermediate status.\n\nBOUNDARY. Reconciliation is application/domain (workflow_app or db); it uses DOC-01 transitions, never writes provider state, and touches provider tables only via DOC-04 for the one-time artifact download. transaction_document gains only signed_media_id/signed_at (no provider/signature state).\n\nREJECTED. (1) overwriting draft bytes in place; (2) reconciling from provider webhooks directly (must go through neutral events); (3) non-idempotent append (duplicate signed media); (4) putting signature/signature-request state on transaction_document beyond signed_media_id/signed_at.\n\nIMPLEMENTATION SEQUENCE. (1) neutral completion event handler + receipt idempotency; (2) media append + DOC-01 state transition; (3) replay/duplication + out-of-order tests; (4) SCOPED verification: targeted + directly adjacent tests only; typecheck/build only when warranted; no FULL regression or broad persistence harness.\n\nRISKS. Duplicate signed media on replay (receipt + unique provider event id); original bytes loss (append-only media); partial failure between media create and transition (idempotent retry must detect existing signed_media_id); signed artifact download failure (retry, keep state sent).",
      "context_refs": "db/transaction-document.ts + db/migrations/027_transaction_document.sql (DOC-01: state sent/signed, signed_media_id, signed_at, transitionTransactionDocumentState); DOC-03 (signature_request + neutral completed event); DOC-04 (provider event id dedupe + artifact download); db/workflow-command-receipt.ts (claim/finalize/read/replay); media schema (media + property_media roles); workflow_app/tests/transaction-document.test.ts + command-receipt.test.ts.",
      "postconditions": "Completed signed artifacts reconcile into canonical transaction_document idempotently with append-only lineage; draft/original bytes are preserved; replay and webhook duplication are safe; DOC-01 remains the canonical transaction-document model without provider leakage.\n\n- Reconciliation must be idempotent and must preserve draft/original bytes.",
      "architect_brief_updated_at": "2026-08-21T16:32:55.245085+00:00",
      "operating_surface": "NEXUS"
    }
  }
]
```

## Associated Story Run Records

Includes every run associated with the requested planned stories and the five completed sample stories.

```json
[
  {
    "story_run": {
      "id": "9b5f0de5-524e-4bd4-bc5e-b02726364432",
      "story_id": "DOC-01",
      "started_at": "2026-08-21T16:04:40.440263+00:00",
      "ended_at": "2026-08-21T16:04:40.623481+00:00",
      "result_status": "Complete",
      "completion": 100,
      "notes": "DOC-01 canonical transaction document model implemented and verified.\nAdded migration 027 (transaction_document: deal-scoped, document_type(+label), state, source(+source_system/source_external_id), ownership (prepared_by_user_id, party_person_id), media_id, signed_media_id, signed_at, supersedes_document_id; CHECK constraints on type/state/source; signed_pair + signed_distinct lineage invariants; indexes (deal_id), (deal_id, state), unique partial (deal_id, source_system, source_external_id) where source_external_id is not null).\nRepository db/transaction-document.ts: createTransactionDocument (source-idempotent for external documents), transitionTransactionDocumentState (claim-first command-receipt idempotency; draft->ready->sent->signed with signed lineage; signed transition requires a NEW media row), listTransactionDocumentsByDeal, getTransactionDocument. workflow_engine untouched; no provider (BoldSign) columns.\nTests: workflow_app/tests/transaction-document.test.ts (9 tests: create/validation/type-label/signed-pair, source idempotency, full lifecycle with draft-bytes preservation, disallowed/unknown/invalid transitions, signed-lineage requirement, pending-receipt no-mutation, completed-receipt replay).\nMigration 027 applied to DEV; live DEV smoke passed (create/transitions/signed lineage/idempotent replay/external-source idempotency/list/cascade cleanup; TMP rows cleaned up).",
      "commit_hash": "33ad8b8bfa308de337e2bc6c969748d2a6c274d6",
      "tests_summary": "pnpm test: 266/266 passed (9 new DOC-01); tsc --noEmit clean; next build --webpack clean; git diff --check clean; live DEV smoke passed.",
      "created_at": "2026-08-21T16:04:40.440263+00:00",
      "goal_snapshot": "Define the canonical transaction-document record that links a document to a deal (the transaction), classifies it by document type, tracks its lifecycle state, ownership and source/provenance, and preserves signed-artifact lineage — without embedding any signature-provider concern (DOC-03/DOC-04 own providers).",
      "preconditions_snapshot": "- media stays the generic reusable asset store (media_type 'document'); it is NOT extended with transaction semantics.\n- deal.id (uuid) is the canonical transaction identity; a transaction document is deal-scoped.\n- Existing property-scoped documents (property_media role 'document') are unchanged and are NOT migrated into the transaction-document model.\n- workflow_engine remains domain-neutral; DOC-01 lives in the application/domain layer (db/ + workflow_app integration), never in workflow_engine, and never reuses the generic engine jobs table.",
      "architect_brief_snapshot": "Proposed canonical model — one additive table transaction_document (deal-scoped; migration 027+):\n\ntransaction_document (\n  id uuid PK default gen_random_uuid(),\n  deal_id uuid NOT NULL references deal(id) on delete cascade,      -- the transaction\n  document_type text NOT NULL check (document_type in ('agreement','addendum','disclosure','title','financing','inspection','appraisal','closing','other')),\n  document_type_label text check (char_length <= 120),               -- long tail (mirrors deal_participant.role_label)\n  title text,\n  state text NOT NULL check (state in ('draft','ready','sent','signed','voided','superseded')),\n  source text NOT NULL check (source in ('upload','generated','imported','provider')),\n  source_system text,\n  source_external_id text,\n  prepared_by_user_id uuid references app_user(id),                  -- ownership (internal author)\n  party_person_id uuid references person(id),                        -- ownership (client/seller party)\n  media_id uuid references media(id),                                -- draft/current stored bytes\n  signed_media_id uuid references media(id),                         -- signed artifact (lineage; draft never mutated)\n  signed_at timestamptz,\n  supersedes_document_id uuid references transaction_document(id),   -- optional version lineage\n  created_at timestamptz default now(), updated_at timestamptz default now()\n);\nindexes: (deal_id), (deal_id, state); unique partial index (deal_id, source_system, source_external_id) where source_external_id is not null.\n\nBoundaries — transaction_document is application/domain data (db/ repository + workflow_app integration). It never enters workflow_engine; the generic engine jobs table is unrelated and not reused. Provider-specific signing (BoldSign) is excluded: DOC-01 reserves only signed-artifact lineage (signed_media_id, signed_at), never provider columns.\n\nReuse — file bytes stay in media (media_type 'document'); the canonical record references media.id. Ownership/parties reuse app_user/person/deal_participant. State transitions reuse claim-first command-receipt idempotency (claimReceipt/finalizeReceipt/replayOutcome, setDealStage) so retries/replays never double-apply.\n\nRejected alternatives — (1) extending media with document_type/state/ownership: pollutes the generic asset store shared by images/videos/properties/guides; (2) reusing property_media for deal documents: it owns property-scoped roles/ordering, not a document lifecycle; (3) a standalone document table with no deal link: a transaction document is deal-scoped; (4) embedding BoldSign/provider fields now: leaks provider concerns into the canonical model.\n\nImplementation sequence — DOC-01 (this model + repository + invariant tests) -> DOC-02 transaction packet (required document_types per deal stage/financing_type) -> DOC-03 provider-neutral signature seam -> DOC-04 BoldSign adapter -> DOC-05 signed-document reconciliation (signed artifact -> state=signed, signed_media_id, signed_at).\n\nKnown risks — closed document_type/state enums vs the long tail (mitigate with label/title + application-curated categories); BYTEA storage for large signed PDFs (existing media pattern: bytes in media, metadata in transaction_document); cross-row transition ordering cannot be enforced by a Postgres CHECK (enforce in the application command layer, like offer/deal_stage); reconciliation dedup (use command-receipt + the source idempotency key so webhook replays never duplicate signed documents).",
      "context_refs_snapshot": "Inspected: db/migrations/001_initial_schema.sql (deal, person, app_user, interaction, task); 002_media.sql, 003_property_domain.sql, 004_document_media.sql (media + property_media roles, media_type document); 012_deal_participant.sql (party/ownership pattern), 013_showing.sql, 014_offer.sql, 020_deal_financing_type.sql; 018_workflow_command_receipt.sql + db/workflow-command-receipt.ts + db/deal-stage.ts (claim-first command idempotency); db/deals.ts, db/dossier.ts (no existing deal-document association); db/media-admin.ts (media audit projection); scripts/import-property-document.mjs (property document ingestion into media + property_media role=document); app/api/media/documents/[id]/route.ts (document serving); components/property/property-documents.tsx + lib/property-types.ts (PropertyDocument shape); workflow_engine/ARCHITECTURE_BOUNDARY.md + workflow_engine/scripts/schema.sql (generic jobs table, not to conflate); workflow_app/tests/command-receipt.test.ts (idempotency semantics); Story Board rows DOC-01..DOC-05.",
      "acceptance_criteria_snapshot": "The implementation worker must prove: (1) an additive migration adds transaction_document with deal_id, document_type(+label), state, source(+source_system/source_external_id), ownership (prepared_by_user_id, party_person_id), media_id, signed_media_id, signed_at, supersedes_document_id — CHECK constraints on type/state/source, indexes on (deal_id) and (deal_id, state), and a unique partial index (deal_id, source_system, source_external_id) where source_external_id is not null; (2) repository create/transition/read functions use command-receipt idempotency so a pending receipt never mutates state and replay returns the stored outcome; (3) no column/table is added to workflow_engine and no provider (BoldSign) column exists; (4) media and property_media are unchanged and property documents are unaffected; (5) signing a document stores a NEW media row and sets signed_media_id + signed_at without mutating the draft bytes; (6) automated tests cover type/state/source validation, deal scoping, idempotent transition, and signed-lineage integrity.",
      "postconditions_snapshot": "A canonical deal-scoped transaction_document record exists with type/state/ownership/source and signed-artifact lineage; media remains the generic asset store and property documents are untouched; DOC-02 packet, DOC-03 provider seam, DOC-04 BoldSign, and DOC-05 reconciliation all build on transaction_document without reworking the core model; workflow_engine remains domain-neutral with no document concept leaked in.",
      "updated_at": "2026-08-21T16:04:40.623481+00:00",
      "execution_environment": null
    }
  },
  {
    "story_run": {
      "id": "9333a47d-5f8a-4478-bde7-1016149489ad",
      "story_id": "DOC-02",
      "started_at": "2026-08-22T06:50:35.151987+00:00",
      "ended_at": "2026-08-22T06:56:03.213731+00:00",
      "result_status": "Complete",
      "completion": 100,
      "notes": "2026-08-22 06:50 — run started via deepseek-harness (external deepseek-pending-1787381435206)\n2026-08-22 06:50 — deepseek harness running (external deepseek-pending-1787381435206)\n2026-08-22 06:50 — deepseek harness running (external session-cf5296df-f731-4277-880f-f7ec025d180a)\nDeepSeek Harness run completed.\n\nAssistant output:\nSDLC story **DOC-02: Transaction Packet** is implemented, verified under the scoped runtime policy, and committed locally (not pushed).\n\n## What was built\n\n**New module — `workflow_app/transaction-packet.ts`** (application/domain layer, no `workflow_engine`, no signing):\n\n1. **Pure required-document mapping** — `requiredTransactionDocumentTypes(dealStage, facts)` returns the deterministic required set (document_type + curated `document_type_label`) from deal stage + `PacketFacts`. The single source of truth is an in-code rule catalog `PACKET_RULES` (stage + condition → document type) — no packet table, no migration:\n   - `offer` → agreement (P&S/listing)\n   - `under_contract` → agreement, disclosure, title, plus financing / inspection / appraisal / tax‑CRIM clearance / HOA / survey gated by `financingApplicable`, `inspectionApplicable`, `appraisalApplicable`, `requiresCrimClearance`, `requiresHoaClearance`, `requiresSurvey`\n   - `closed` → closing documents, closing statement, registry/recording follow-up gated by `requiresRegistryFollowup`\n   - Pre-packet/unknown stages deterministically require nothing.\n\n2. **Unresolved facts** — `unresolvedPacketGates(dealStage, facts)` surfaces null gating facts (e.g. `appraisalApplicable: null`) as unresolved, **never** as a required document; `requiredTransactionDocumentTypes` filters null gates out.\n\n3. **Completeness projection** — pure `buildTransactionPacket(dealId, stage, facts, documents)` and the read seam `getTransactionPacketForDeal(dealId, stage, facts, execute?)` (reads `transaction_document` via the DOC-01 `listTransactionDocumentsByDeal`) list each required item as `present` / `missing` / `unresolved` with `documentIds`, counts, and `complete`. It never writes, never auto-creates, and presence is type/label-based — a `draft` counts as present, `voided`/`superseded` do not; signature state is never consulted.\n\n**New tests — `workflow_app/tests/transaction-packet.test.ts`**: 14 tests covering determinism per stage, fact gating, long-tail label reuse (CRIM/HOA/survey/closing statement), unresolved-never-fabricated, present/missing/unresolved projection, per-deal scoping, no-auto-create, and no-signing semantics.\n\n## Acceptance criteria — evidence\n\n1. ✅ `requiredTransactionDocumentTypes` deterministic set from stage + facts (tests: determinism, gating, HOA/survey).\n2. ✅ Unresolved Class C facts → `unresolved`, never fabricated (tests: no `appraisal`/`financing` in required set; `unresolvedPacketGates` surfaces the fact).\n3. ✅ Completeness projection lists present/missing/unresolved per deal without auto-creating (test asserts document count unchanged after the seam).\n4. ✅ No signing/signature concept introduced — module never reads `signedMediaId`/`signedAt`; `draft` and `signed` both count as present.\n5. ✅ Reuses `transaction_document` — items are enum `documentType` + curated label (directly creatable via `createTransactionDocument`); no duplicate table, no schema change.\n6. ✅ Scoped verification only (policy compliant; none of the forbidden commands run): `npx tsx --test workflow_app/tests/transaction-packet.test.ts` → 14/14 pass; directly adjacent `workflow_app/tests/transaction-document.test.ts` → 9/9 pass; `npx tsc --noEmit` → clean (exit 0). No full regression, no persistence harness.\n\n## Implementation sequence status\n\n(1) pure mapping + unit tests ✅ · (2) completeness projection ✅ · (3) editable catalog — intentionally **not** added (in-code catalog, per \"start without it\") · (4) portal surface — deferred (later, bounded), out of scope.\n\n## Commit\n\n`b0fdaec` — `feat: DOC-02 transaction packet — derived required-document projection` (2 files, +721). Working tree clean; nothing pushed; no production data or schema mutated. (The `tsconfig.tsbuildinfo` cache churn from the typecheck was reverted to keep the commit scoped.)\n\nSession transcript: /Users/lisapenfieldicloud.com/.dsh/sessions/--Users-lisapenfieldicloud.com-Documents-Culebraluxe-web--/session-cf5296df-f731-4277-880f-f7ec025d180a",
      "commit_hash": "b0fdaec5097470100788137e44b2d7a6a38ddef1",
      "tests_summary": "dsh exit code 0",
      "created_at": "2026-08-22T06:50:35.151987+00:00",
      "goal_snapshot": "Define how a transaction packet determines the required documents for a deal from deal/workflow state and financing/deal characteristics, building on the DOC-01 transaction_document model — with no signing-provider concerns.",
      "preconditions_snapshot": "- DOC-01 transaction_document is the canonical deal-scoped document record (document_type/state/source/ownership/signed lineage).\n- A packet must be DERIVED from canonical deal facts; unresolved Class C facts must not force documents and must not be invented.",
      "architect_brief_snapshot": "CURRENT-STATE. transaction_document holds the canonical documents; there is no \"packet\" concept that maps required document types to a deal's current state. Deal facts (facts.ts) expose stage, financingApplicable, closingDateScheduled, and jurisdiction config (requiresNotario, requiresTitleCompany, requiresCrimClearance, requiresRegistryFollowup, requiresHoaClearance, requiresSurvey, inspectionApplicable, insuranceApplicable, appraisalApplicable [Class C unresolved]).\n\nRECOMMENDED MODEL. A packet is a PURE, DERIVED projection — no new required table, optionally a small editable catalog. Define a pure function requiredTransactionDocumentTypes(dealStage, facts) → the required document_type (+ document_type_label) set, then a completeness query over transaction_document that returns, per deal, which required types are present / missing / unresolved. The packet never auto-creates documents; creation is an explicit user/application action or a later DOC-05 reconciliation. The packet only answers \"what is required vs present\".\n\nMAPPING SKETCH. agreement (P&S/listing) at offer→under_contract; disclosure + title for title_work; financing documents gated by financingApplicable; inspection/appraisal gated by their applicability; tax/CRIM clearance gated by requiresCrimClearance; closing_documents + closing statement at closing; recording/registry follow-up gated by requiresRegistryFollowup; HOA/survey gated by their flags. Unresolved Class C facts (appraisalApplicable null) → do NOT emit a required document; mark \"unresolved\" and surface it to the operator — never invent.\n\nREPRESENTATION. Prefer a pure function (workflow_app or lib) over a new table. If operators must edit the required-document list, add a small deal-independent catalog table (stage + condition → document_type); start without it.\n\nBOUNDARY. DOC-02 is application/domain (workflow_app/lib), no signing, no workflow_engine. Reuse transaction_document; the XML node id remains the workflow state identity.\n\nREJECTED. (1) a packet table that duplicates transaction_document; (2) embedding signing/signature state; (3) inventing documents for unresolved facts; (4) hardcoding a single jurisdiction.\n\nIMPLEMENTATION SEQUENCE. (1) pure required-doc-type mapping + unit tests; (2) packet completeness projection (present/missing/unresolved) per deal; (3) optional editable catalog only if required; (4) portal surface (later, bounded).\n\nRISKS. Unresolved Class C facts must not block the packet (surface, don't block); long-tail document types (use document_type_label); the packet is derived and recomputed on stage/fact change (no stored staleness).",
      "context_refs_snapshot": "db/transaction-document.ts (DOC-01: document_type/state/source/ownership/signed lineage); db/migrations/027_transaction_document.sql; workflow_app/facts.ts (DealWorkflowFacts: stage, financingApplicable, closingDateScheduled, requiresNotario/TitleCompany/CrimClearance/RegistryFollowup/HoaClearance/Survey, inspectionApplicable, insuranceApplicable, appraisalApplicable Class C); workflow_app/configuration.ts (jurisdiction defaults); workflow_app/definitions/RE_supermodel-v1.xml (offer_accepted→under_contract→title/tax/funds/closing/recording); db/deal-financing.ts + db/deal-stage.ts (stage/financing facts).",
      "acceptance_criteria_snapshot": "The implementation worker must prove: (1) requiredTransactionDocumentTypes returns a deterministic required-document set from deal stage + facts; (2) unresolved Class C facts produce \"unresolved\", never a fabricated required document; (3) a packet completeness projection lists present/missing/unresolved per deal without auto-creating documents; (4) no signing/signature concept is introduced; (5) the mapping reuses transaction_document and does not duplicate it; (6) unit + SCOPED verification: targeted + directly adjacent tests only; typecheck/build only when warranted; no FULL regression or broad persistence harness.",
      "postconditions_snapshot": "A transaction packet is a derived, deterministic projection over transaction_document + deal facts; required documents are computed from stage/financing/jurisdiction without inventing unresolved facts; DOC-03 (signature seam) and DOC-05 (reconciliation) can build on this packet + DOC-01 without rework.\n\n- DOC-02 introduces no signing/signature state.",
      "updated_at": "2026-08-22T06:56:03.213731+00:00",
      "execution_environment": "DEV"
    }
  },
  {
    "story_run": {
      "id": "c5a27d71-7afd-4e02-b488-4d4bc91c7f5f",
      "story_id": "DOC-03",
      "started_at": "2026-08-22T08:52:20.179197+00:00",
      "ended_at": "2026-08-22T09:06:35.720157+00:00",
      "result_status": "Complete",
      "completion": 100,
      "notes": "2026-08-22 08:52 — run started via deepseek-harness (external deepseek-pending-1787388740267)\n2026-08-22 08:52 — deepseek harness running (external deepseek-pending-1787388740267)\n2026-08-22 08:52 — deepseek harness running (external session-0b1a16da-a876-486a-9365-2ede492af433)\nDeepSeek Harness run completed.\n\nAssistant output:\n## DOC-03 — Signature Provider Seam: Complete\n\nCommitted locally as `c4d3939` (not pushed). Working tree clean.\n\n### What was built\n\n**Neutral contracts + status model** — `lib/signature/contracts.ts`\n- Four neutral commands: `signature.request.send` / `.status` / `.cancel` / `.decline` (stable identifiers in `lib/commands/command-types.ts`, re-exported by the seam).\n- Neutral status model `requested → sent → viewed → signed → completed | declined | voided | expired | error` with a legal-transition table, active/terminal partitions, neutral recipient roles (`signer`/`approver`), and the neutral webhook event vocabulary.\n\n**Canonical `signature_request` table** — `db/migrations/036_signature_request.sql`\n- References `transaction_document_id`; carries **only** neutral status/message/author — zero provider fields. A partial unique index enforces **one active request per transaction document** (duplicate-send DB backstop).\n\n**Canonical service** — `db/signature-request.ts`\n- Claim-first command-receipt idempotency (same pattern as `db/transaction-document.ts`) for send/status/cancel/decline; `INSERT … ON CONFLICT … DO NOTHING` + re-select means a send for a document with an active request **returns the existing request, never a duplicate**; status changes are legal-transition compare-and-set. **No provider call anywhere in this file** (rejected design #2 respected).\n\n**Provider seam** — `lib/signature/provider.ts` (interface), `lib/signature/status-mapping.ts` (provider status → neutral; unknown fails closed to `error`), `lib/signature/provider-registry.ts` (configured-provider dispatch), `lib/signature/fake-provider.ts` (fake proving the seam), `lib/signature/application.ts` (the router: dispatches by configured provider, provider calls run strictly **after** command commit; webhook entrypoint normalizes at the seam first).\n\n**Neutral events** — `SIGNATURE_REQUEST_SENT/_COMPLETED/_DECLINED/_VOIDED` added to the existing `DomainEventType`, emitted by the thin command handlers via `createDomainEventFromCommand` with correlation/causation from the envelope; registered in `lib/commands/register.ts`.\n\n### Acceptance criteria — proven in `workflow_app/tests/signature-seam.test.ts` (18 tests)\n1. Neutral `SignatureProvider` interface + command set + neutral status model ✓\n2. Canonical record references `transaction_document`, carries **no** provider fields (test asserts the exact stored column set) ✓\n3. send/status/cancel idempotent via receipts (same-commandId replay; active-request return; same-status no-op) ✓\n4. Provider status → neutral mapped at the seam (mapping table, `provider.status()` returns neutral, send/webhook observations mapped) ✓\n5. Neutral events carry correlation/causation (asserted on `eventId`/`correlationId`/`causationId`) ✓\n6. `transaction_document` untouched by intermediate provider state (test runs a full webhook completion and asserts `state`/`signed_at`/`signed_media_id` unchanged) ✓\n7. Fake provider proves the seam end-to-end (send→sent, delivery failure→error, webhook→completed, status poll, cancel) ✓\n\n### Verification (SCOPED per runtime policy)\n- `workflow_app/tests/signature-seam.test.ts` — 18/18 pass\n- Directly adjacent: `command-layer`, `command-receipt`, `command-inventory`, `transaction-document` — 36/36 pass (54 total)\n- `tsc --noEmit` — clean; `pnpm exec next build --webpack` — clean; `git diff --check` — clean\n- No full regression, no persistence harness, no production data/schema mutation (migration 036 is recorded for DEV, not executed)\n\n### Files changed\n- **New:** `db/migrations/036_signature_request.sql`, `db/signature-request.ts`, `lib/signature/{contracts,status-mapping,provider,provider-registry,fake-provider,application}.ts`, `lib/commands/signature/signature-commands.ts`, `workflow_app/tests/signature-seam.test.ts`\n- **Modified:** `lib/commands/command-types.ts`, `lib/commands/register.ts`, `lib/workflow/contracts.ts` (additive event/aggregate types), `tsconfig.tsbuildinfo` (tsc artifact, tracked per prior story convention)\n\nDOC-04 (BoldSign adapter) now plugs in behind `SignatureProvider` + `mapProviderStatus` with no canonical-model changes.\n\nSession transcript: /Users/lisapenfieldicloud.com/.dsh/sessions/--Users-lisapenfieldicloud.com-Documents-Culebraluxe-web--/session-0b1a16da-a876-486a-9365-2ede492af433",
      "commit_hash": "c4d393987c81a9c331776ad91a3aa171d3569b2e",
      "tests_summary": "dsh exit code 0",
      "created_at": "2026-08-22T08:52:20.179197+00:00",
      "goal_snapshot": "Define the provider-neutral signing boundary (commands, contracts, events, status mapping) on top of DOC-01/DOC-02, so provider-specific signing concerns (BoldSign) stay behind a seam and out of canonical domain models.",
      "preconditions_snapshot": "- DOC-01 transaction_document is canonical; DOC-02 packet determines required documents.\n- The provider-neutral seam must exist before any BoldSign adapter.",
      "architect_brief_snapshot": "RECOMMENDED MODEL. A provider-neutral signing seam at the application boundary, shaped like the existing workflow command seam (lib/workflow/contracts.ts + command-receipt idempotency).\n\nCONTRACTS/COMMANDS (neutral): signature.request.send (input: transactionDocumentId, recipients[] with neutral roles, signer ordering, optional message); signature.request.status (input: signatureRequestId); signature.request.cancel; signature.request.decline. All return a CommandResult and are idempotent via claim-first receipts (a send for the same transaction_document + active request returns the existing request, never a duplicate).\n\nPROVIDER INTERFACE: a neutral SignatureProvider interface — send(request), status(requestId), cancel(requestId), verifyWebhook(payload, signature) → {event, signatureRequestId}. Provider adapters (DOC-04 BoldSign) implement it; the router dispatches by configured provider, never by provider-specific command.\n\nNEUTRAL STATUS MODEL: requested → sent → viewed → signed → completed | declined | voided | expired | error. Store this neutral status on a canonical `signature_request` record that references transaction_document_id (NOT inside transaction_document). Provider-specific IDs/state live in a separate provider table (DOC-04) and never in transaction_document.\n\nEVENTS: signature.request.sent / .completed / .declined / .voided, carrying eventId/correlation/causation reusing the existing DomainEvent shape, so downstream consumers (DOC-05 reconciliation) subscribe to neutral events, not provider webhooks.\n\nSTATUS-MAPPING BOUNDARY: the seam maps provider status → neutral status; transaction_document reflects only the FINAL signed outcome via DOC-05 reconciliation (signed_media_id, signed_at) — never intermediate provider state.\n\nBOUNDARY. Canonical domain models stay provider-free; provider adapters sit behind SignatureProvider; command receipts give idempotent send/status; the application owns authority/validation.\n\nREJECTED. (1) BoldSign fields on transaction_document; (2) synchronous provider calls inside a domain service; (3) per-provider commands in the router; (4) webhook handlers writing straight to transaction_document.\n\nIMPLEMENTATION SEQUENCE. (1) neutral contracts + status model + signature_request table (migration); (2) SignatureProvider interface + a fake/test provider; (3) command-receipt idempotency + neutral events; (4) DOC-04 adapter plugs in behind the seam.\n\nRISKS. Provider semantics leaking through webhook payloads (normalize at the seam); status-model drift across providers; duplicate sends (enforce one active request per transaction_document).",
      "context_refs_snapshot": "db/transaction-document.ts + db/migrations/027_transaction_document.sql (DOC-01: document_type/state/source/ownership/signed lineage); db/workflow-command-receipt.ts (claim/finalize/replay idempotency); lib/workflow/contracts.ts (CommandEnvelope/CommandResult/DomainEvent/correlation-causation); lib/workflow/command-inventory.ts (DomainCommand idempotency classes); docs/workflow-integration-contract.md (command seam + subject model); workflow_app/tests/command-receipt.test.ts (idempotency precedent).",
      "acceptance_criteria_snapshot": "The implementation worker must prove: (1) a neutral SignatureProvider interface and command set exist with a neutral status model; (2) a canonical signature_request record references transaction_document and carries NO provider-specific fields; (3) send/status/cancel are idempotent (replay-safe) via receipts; (4) provider status maps to neutral status at the seam; (5) neutral events (sent/completed/declined/voided) carry correlation/causation; (6) transaction_document is untouched by intermediate provider state; (7) a fake provider proves the seam end-to-end; (8) SCOPED verification: targeted + directly adjacent tests only; typecheck/build only when warranted; no FULL regression or broad persistence harness.",
      "postconditions_snapshot": "A provider-neutral signing seam exists (contracts, signature_request record, neutral status model, events); BoldSign concepts are isolated behind the seam; DOC-04 can implement the adapter without touching canonical models; DOC-05 can reconcile signed artifacts from neutral events.\n\n- No provider-specific concepts (envelope IDs, provider roles, webhook payloads) may enter canonical domain models.",
      "updated_at": "2026-08-22T09:06:35.720157+00:00",
      "execution_environment": "DEV"
    }
  },
  {
    "story_run": {
      "id": "caa3fc89-95b5-4791-a0ff-90a8c207616a",
      "story_id": "DOC-04",
      "started_at": "2026-08-22T09:11:49.458001+00:00",
      "ended_at": "2026-08-22T09:27:23.271945+00:00",
      "result_status": "Complete",
      "completion": 100,
      "notes": "2026-08-22 09:11 — run started via deepseek-harness (external deepseek-pending-1787389909572)\n2026-08-22 09:11 — deepseek harness running (external deepseek-pending-1787389909572)\n2026-08-22 09:11 — deepseek harness running (external session-f9f298af-7279-4426-97a4-576ce49a92f3)\nDeepSeek Harness run completed.\n\nAssistant output:\nDOC-04 complete and committed (`e8616e5`). Working tree clean.\n\n## What was built\n\nA self-contained **BoldSign adapter module** (`lib/signature/boldsign/`) implementing the DOC-03 `SignatureProvider` seam. All BoldSign vocabulary is confined behind the seam; the canonical models stay provider-free.\n\n| Responsibility | Implementation |\n|---|---|\n| **send** | Maps neutral recipients → BoldSign template roles (`POST /v1/template/send`), persists **envelope id + provider document ids** in `bold_sign_request` keyed by `signature_request_id` (one row per request). Idempotent: an existing provider row returns the existing envelope — **no duplicate envelope**, backed by a partial unique index on `envelope_id`. |\n| **status** | Polls `GET /v1/document/properties`, caches the RAW BoldSign status + file ids in the provider table, returns the status **already mapped to neutral** at the seam. |\n| **cancel** | Best-effort `POST /v1/document/revoke` strictly after the seam commits neutral `voided`. |\n| **webhook** | Verifies the BoldSign HMAC signature (`X-BoldSign-Signature: t=…, s0=…` over the **raw body**, SHA-256, constant-time compare, timestamp-tolerance replay protection — faithful to [BoldSign's verify-webhook docs](https://developers.boldsign.com/webhooks/verify-webhook-events/)), normalizes to a neutral event + neutral request id (resolved via the provider table), and **dedupes by provider event id** (unique key in `bold_sign_webhook_event`) — the same row is the DOC-05 async reconciler's durable enqueue record. |\n| **retries/idempotency** | Transient errors (network/timeout/408/429/5xx) retried with capped exponential backoff + per-attempt timeout; send idempotent via command receipt + provider envelope unique key; webhook idempotent via provider event id. |\n| **errors** | Provider failures map to neutral `'error'` with retryable/non-retryable classification and **observable `last_error`** in the provider table (never on canonical rows). |\n| **credentials** | `BOLDSIGN_API_KEY`, `BOLDSIGN_BASE_URL`, `BOLDSIGN_TEMPLATE_ID`, `BOLDSIGN_WEBHOOK_SECRET` from config/env only, fail-closed; errors name **keys, never values**; nothing is logged. |\n\n**Boundary honored** — provider ids/state live only in `bold_sign_request` / `bold_sign_webhook_event` (migration 037); `transaction_document` is untouched by intermediate provider state (final signed outcome stays DOC-05's job); `signature_request` carries zero provider fields (test-asserted).\n\n## Files changed\n\n- **New:** `db/migrations/037_bold_sign_provider.sql`, `db/bold-sign-request.ts`, `lib/signature/boldsign/{config,errors,webhook,events,client,adapter,index}.ts`, `workflow_app/tests/boldsign-adapter.test.ts`\n- **Modified:** `lib/signature/status-mapping.ts` (BoldSign `DocumentStatus` branch at the documented DOC-04 extension point), `tsconfig.tsbuildinfo`\n\n## Verification (SCOPED per runtime policy)\n\n- **New suite** `boldsign-adapter.test.ts` — **25/25 pass**, driving a real in-process **fake BoldSign HTTP server** (auth header, envelope send/properties/revoke, failure injection 500/422/timeout, HMAC-signed webhooks): send, status, cancel, webhook verify, **replay dedupe**, error classification, e2e through the seam (send → webhook lifecycle → `SIGNATURE_REQUEST_COMPLETED`; `transaction_document` untouched).\n- **Directly adjacent** — signature-seam / transaction-document / command-layer / command-receipt / command-inventory: **54/54 pass** (total 79).\n- `tsc --noEmit` **clean**; `git diff --check` **clean**.\n- No full regression, no persistence harness. `next build` **not run** — the new modules and the additive `status-mapping` change are not in the app build graph (no app wiring changed); tsc covers all touched code.\n\n## Notes\n\n- **BoldSign statuses** (authoritative from [BoldSign's OpenAPI/docs](https://developers.boldsign.com/documents/document-details-and-status/)): `InProgress→sent`, `Completed→completed`, `Declined→declined`, `Expired→expired`, `Revoked→voided`, `Draft/Scheduled→requested`; unknown fails closed to `error`.\n- **Webhook normalization** maps lifecycle events exactly (Sent/Viewed/Signed/Completed/Declined/Revoked/Expired/SendFailed); benign notifications (Reminder/Reassigned/…) fall back to the document status **only when it is an active neutral state** — never fabricated into a terminal outcome; unmappable payloads fail closed.\n- The neutral chain's completion edge is `signed→completed`, so a status **poll** of a completed envelope lands legally only after the webhook-observed intermediate states (tested e2e); the webhook `Completed` path completes directly.\n- Not in scope (documented, not wired): registering the adapter in `SignatureProviderRegistry` at app startup (`createBoldSignProvider()` factory is provided in `lib/signature/boldsign/index.ts`), the webhook HTTP route, and DOC-05 signed-artifact reconciliation.\n\nSession transcript: /Users/lisapenfieldicloud.com/.dsh/sessions/--Users-lisapenfieldicloud.com-Documents-Culebraluxe-web--/session-f9f298af-7279-4426-97a4-576ce49a92f3",
      "commit_hash": "e8616e5f25215f24b7f99c44907c1cdda071a5dc",
      "tests_summary": "dsh exit code 0",
      "created_at": "2026-08-22T09:11:49.458001+00:00",
      "goal_snapshot": "Analyze the concrete BoldSign adapter responsibilities — send, status, completion, webhook handling, retries/idempotency, provider IDs, error handling, credentials — assuming the DOC-03 provider seam is the boundary, without leaking BoldSign semantics into transaction_document.",
      "preconditions_snapshot": "- DOC-03 seam exists and is the only boundary the adapter touches.\n- BoldSign credentials/configuration come from env/config — never hardcoded.",
      "architect_brief_snapshot": "RECOMMENDED. A BoldSign adapter module implementing the DOC-03 SignatureProvider, owning ALL provider specifics; the seam and canonical models see only neutral types.\n\nRESPONSIBILITIES. (1) send — map the neutral signature request to a BoldSign envelope/template; persist provider envelope id + document ids in a provider-specific table (bold_sign_request) keyed by signature_request_id (one row per request); read credentials from config/env. (2) status — poll/query BoldSign envelope status and map BoldSign's status enum to the neutral status (requested/sent/viewed/signed/completed/declined/voided/expired/error). (3) completion — on completed, emit the neutral signature.request.completed event (DOC-05 reconciles the signed artifact). (4) webhook — verify the BoldSign webhook signature (HMAC), normalize the payload to a neutral event, dedupe by provider event id (unique key), acknowledge fast and reconcile asynchronously. (5) retries/idempotency — send idempotent via command receipt + provider document id unique key; webhook idempotent via provider event id; transient provider errors retried with capped exponential backoff and timeouts. (6) provider IDs — stored ONLY in bold_sign_request, never in transaction_document. (7) error handling — map provider errors to neutral 'error' status with retryable/non-retryable classification and observable last_error. (8) credentials/config — BOLDSIGN_API_KEY, BOLDSIGN_BASE_URL, BOLDSIGN_TEMPLATE_ID, BOLDSIGN_WEBHOOK_SECRET from config/env; never hardcoded, never logged.\n\nBOUNDARY. BoldSign types/strings are confined to the adapter; the seam emits only neutral events; transaction_document is untouched by intermediate provider state; signature_request stays provider-free.\n\nREJECTED. (1) provider fields on transaction_document or signature_request; (2) webhook writing straight to transaction_document; (3) hardcoded credentials; (4) synchronous blocking webhook handling; (5) per-provider commands in the router (dispatch via DOC-03 seam).\n\nIMPLEMENTATION SEQUENCE. (1) config + typed BoldSign client wrapper; (2) send/status/cancel adapter + bold_sign_request table; (3) webhook verification + normalization + idempotent enqueue; (4) status polling + completion event emission; (5) tests against a fake BoldSign server (send/status/webhook/replay/error); (6) SCOPED verification: targeted + directly adjacent tests only; typecheck/build only when warranted; no FULL regression or broad persistence harness.\n\nRISKS. Webhook replay (dedupe by provider event id); webhook signature verification correctness; BoldSign API version drift; secret handling/logging; retry storms (cap attempts + backoff); provider id leakage into canonical rows.",
      "context_refs_snapshot": "DOC-03 seam (SignatureProvider + neutral status/events); db/transaction-document.ts + db/migrations/027 (DOC-01, to be kept provider-free); db/workflow-command-receipt.ts (claim/finalize/replay); lib/workflow/contracts.ts (DomainEvent); workflow_app/configuration.ts (config pattern); docs/workflow-integration-contract.md; workflow_app/tests/command-receipt.test.ts (idempotency precedent); docs/auth-security-model.md (secret handling posture).",
      "acceptance_criteria_snapshot": "The implementation worker must prove: (1) the adapter implements DOC-03 SignatureProvider with send/status/cancel/webhook verification; (2) provider ids/state live only in bold_sign_request, never in transaction_document or signature_request; (3) send is idempotent (no duplicate envelopes) and webhook handling dedupes replays; (4) provider status maps to neutral status and completion emits a neutral event; (5) credentials come from config/env and are never hardcoded or logged; (6) errors map to neutral 'error' with retryable classification; (7) a fake BoldSign server exercises send/status/webhook/replay/error; (8) SCOPED verification: targeted + directly adjacent tests only; typecheck/build only when warranted; no FULL regression or broad persistence harness.",
      "postconditions_snapshot": "A BoldSign adapter sits behind the DOC-03 seam, owning send/status/completion/webhook/retries/idempotency/provider IDs/errors/credentials; canonical domain models remain provider-free; DOC-05 can reconcile signed artifacts from neutral completion events.\n\n- Provider-specific state is isolated from canonical domain models (transaction_document, signature_request).",
      "updated_at": "2026-08-22T09:27:23.271945+00:00",
      "execution_environment": "DEV"
    }
  },
  {
    "story_run": {
      "id": "f4aeacb4-5d4b-4c0e-b29f-2db00a04e0cf",
      "story_id": "DOC-05",
      "started_at": "2026-08-22T09:32:37.497016+00:00",
      "ended_at": "2026-08-22T09:50:06.847379+00:00",
      "result_status": "Complete",
      "completion": 100,
      "notes": "2026-08-22 09:32 — run started via deepseek-harness (external deepseek-pending-1787391157694)\n2026-08-22 09:32 — deepseek harness running (external deepseek-pending-1787391157694)\n2026-08-22 09:32 — deepseek harness running (external session-ce5dda30-0c67-4f26-9655-ce8bb6afd5ea)\nDeepSeek Harness run completed.\n\nAssistant output:\nDOC-05 is complete and committed locally (`a49fca9`, not pushed). Report:\n\n## What was built\n\n**Reconciliation flow** (neutral `signature.request.completed` → canonical `transaction_document` state):\n\n1. **`db/signature-reconciliation.ts`** — the canonical domain service. For each completed event it:\n   - **Claims a command receipt keyed by the neutral event id** (`signature.reconcile:<eventId>`) — a replayed event reads the winner's receipt and returns `replayed:true` (no re-download, no re-append, no double transition).\n   - **Resolves** the `transaction_document` via `signature_request.transaction_document_id` (never from the payload), treating `completed` as terminal regardless of intermediate status (out-of-order tolerant).\n   - **Appends** the signed bytes (downloaded **once** via DOC-04) as a **NEW `media` row** (`media_type='document'`) — the draft/original row is never mutated — then runs **DOC-01's transition** `sent→signed` with `signed_media_id`/`signed_at` (`updated_at=now()` is the version bump). Draft/ready docs are advanced through the legal chain (`draft→ready→sent`) first, each step receipt-backed.\n   - **Atomic append**: media insert + transition + receipt finalize commit in ONE transaction. A transition failure rolls the whole thing back (no stray signed media), and the retry's **already-signed guard** (`signed_media_id` set) makes it exactly-once — the partial-failure recovery the brief requires.\n2. **`lib/signature/reconciliation.ts`** — the neutral-event subscriber (`onCompletedEvent`), which only accepts `SIGNATURE_REQUEST_COMPLETED` and wires the DOC-04 download in.\n3. **Router wiring** (`lib/signature/application.ts`) — subscribes **strictly after commit** on the send/refreshStatus/handleWebhook status paths; `handleWebhook` now surfaces `reconciliation`. Provider payloads never reach the reconciler.\n4. **DOC-04 extension** — `SignatureProvider.downloadSignedArtifact` (neutral `SignedArtifactDownload` bytes/filename/mimeType); BoldSign adapter resolves its envelope through the provider table; client `downloadDocument` (`GET /v1/document/download`, base64 per BoldSign's documented download shape); fake provider implements it with test helpers.\n\n## Files changed\n- New: `db/signature-reconciliation.ts`, `lib/signature/reconciliation.ts`, `workflow_app/tests/signature-reconciliation.test.ts`\n- Modified: `lib/signature/application.ts`, `lib/signature/contracts.ts`, `lib/signature/provider.ts`, `lib/signature/fake-provider.ts`, `lib/signature/boldsign/adapter.ts`, `lib/signature/boldsign/client.ts`, `workflow_app/tests/boldsign-adapter.test.ts` (+ tracked `tsconfig.tsbuildinfo`)\n\n## Verification (SCOPED, per runtime policy)\n- New targeted file `signature-reconciliation.test.ts` — **12 tests** proving all 8 acceptance criteria: append + `sent→signed` transition; byte-for-byte original preservation; replay no-op (`replayed:true`, no duplicate media, no double transition); webhook duplication reconciles exactly once (router dedupe + already-signed guard); out-of-order draft/ready resolution; partial-failure rollback + idempotent retry; download failure keeps state `sent`; zero provider/signature state on `transaction_document`.\n- Directly adjacent: `boldsign-adapter.test.ts` (incl. new download test), `signature-seam.test.ts`, `transaction-document.test.ts` — **65/65 pass**.\n- `tsc --noEmit` clean; `git diff --check` clean. No full regression, no persistence harness, no `next build` (changed modules aren't in the app build graph; no app wiring changed — same rationale as DOC-04).\n\n## Boundary compliance\nAll four rejected designs avoided: no in-place draft overwrite (append-only media), no reconciliation from provider webhooks directly (neutral events only), no non-idempotent append (receipt + already-signed guard + atomic tx), no signature/provider state on `transaction_document` beyond `signed_media_id`/`signed_at`. Provider tables are touched only through the DOC-04 download; no schema/migration was needed.\n\nOne architectural note: the subscriber is wired post-commit in the application router because the durable outbox loop remains deferred (CRM-14I); `bold_sign_webhook_event` stays the future async-enqueue record, and the same `onCompletedEvent` interface is the durable-queue entry point. BoldSign download endpoint shape per [BoldSign's download-as-base64 guide](https://developers.boldsign.com/how-to-guides/download-document-in-base64-format/?region=us).\n\nSession transcript: /Users/lisapenfieldicloud.com/.dsh/sessions/--Users-lisapenfieldicloud.com-Documents-Culebraluxe-web--/session-ce5dda30-0c67-4f26-9655-ce8bb6afd5ea",
      "commit_hash": "a49fca95f0e5ad23f3141231fa2050da718bbc38",
      "tests_summary": "dsh exit code 0",
      "created_at": "2026-08-22T09:32:37.497016+00:00",
      "goal_snapshot": "Define how completed signed artifacts reconcile into canonical transaction_document state safely and idempotently, building on DOC-01 signed-artifact lineage and the DOC-03/DOC-04 provider flow — preserving draft/original bytes, appending signed artifacts, and handling replay/webhook duplication.",
      "preconditions_snapshot": "- DOC-01 established signed-artifact lineage (a new media row + transaction_document.signed_media_id/signed_at).\n- DOC-03/DOC-04 produce a neutral signature.request.completed event carrying the signed artifact reference (provider doc id).",
      "architect_brief_snapshot": "RECOMMENDED. A reconciliation handler that subscribes to NEUTRAL signature.request.completed events — never provider webhooks directly. For each completed signature request:\n1. IDEMPOTENCY CLAIM: claim a command receipt keyed by the neutral event id (and provider event id as the unique source key) so replays are no-ops.\n2. RESOLVE: load the transaction_document via signature_request.transaction_document_id; verify it exists and is in a signable state (draft/sent); if already signed (signed_media_id set), treat as replayed and return.\n3. APPEND SIGNED ARTIFACT: create a NEW media row (document/pdf) holding the signed bytes (downloaded once via the DOC-04 adapter), leaving the original/draft media row untouched; then set transaction_document.signed_media_id = new media id and signed_at = now() through DOC-01's transition (state sent→signed), version increment.\n4. PRESERVE ORIGINAL BYTES: never mutate or delete the draft/original media row; the signed artifact is a distinct media row forming the lineage chain original → signed.\n5. REPLAY/WEBHOOK DUPLICATION: dedupe via provider event id + command receipt; a replayed completed event reads the final receipt and returns replayed:true without re-appending (no duplicate signed media, no double transition).\n6. ORDERING: tolerate out-of-order completion (completed before send status was observed) by resolving the signature_request directly and treating completed as terminal regardless of intermediate status.\n\nBOUNDARY. Reconciliation is application/domain (workflow_app or db); it uses DOC-01 transitions, never writes provider state, and touches provider tables only via DOC-04 for the one-time artifact download. transaction_document gains only signed_media_id/signed_at (no provider/signature state).\n\nREJECTED. (1) overwriting draft bytes in place; (2) reconciling from provider webhooks directly (must go through neutral events); (3) non-idempotent append (duplicate signed media); (4) putting signature/signature-request state on transaction_document beyond signed_media_id/signed_at.\n\nIMPLEMENTATION SEQUENCE. (1) neutral completion event handler + receipt idempotency; (2) media append + DOC-01 state transition; (3) replay/duplication + out-of-order tests; (4) SCOPED verification: targeted + directly adjacent tests only; typecheck/build only when warranted; no FULL regression or broad persistence harness.\n\nRISKS. Duplicate signed media on replay (receipt + unique provider event id); original bytes loss (append-only media); partial failure between media create and transition (idempotent retry must detect existing signed_media_id); signed artifact download failure (retry, keep state sent).",
      "context_refs_snapshot": "db/transaction-document.ts + db/migrations/027_transaction_document.sql (DOC-01: state sent/signed, signed_media_id, signed_at, transitionTransactionDocumentState); DOC-03 (signature_request + neutral completed event); DOC-04 (provider event id dedupe + artifact download); db/workflow-command-receipt.ts (claim/finalize/read/replay); media schema (media + property_media roles); workflow_app/tests/transaction-document.test.ts + command-receipt.test.ts.",
      "acceptance_criteria_snapshot": "The implementation worker must prove: (1) a completed event appends a NEW signed media row and sets signed_media_id/signed_at with the state transition sent→signed; (2) the original/draft media row is preserved byte-for-byte; (3) a replayed completed event is a no-op (no duplicate signed media, no double transition, replayed:true); (4) webhook duplication (same provider event id twice) reconciles exactly once; (5) out-of-order completion resolves correctly; (6) partial failure (media created, transition failed) recovers idempotently; (7) no provider/signature state lands on transaction_document; (8) SCOPED verification: targeted + directly adjacent tests only; typecheck/build only when warranted; no FULL regression or broad persistence harness.",
      "postconditions_snapshot": "Completed signed artifacts reconcile into canonical transaction_document idempotently with append-only lineage; draft/original bytes are preserved; replay and webhook duplication are safe; DOC-01 remains the canonical transaction-document model without provider leakage.\n\n- Reconciliation must be idempotent and must preserve draft/original bytes.",
      "updated_at": "2026-08-22T09:50:06.847379+00:00",
      "execution_environment": "DEV"
    }
  }
]
```
