#!/usr/bin/env bash
# The layout: rust/ -> the three tiers, one suite at the root, the container files under devops/.
#
# WHY THIS IS A SCRIPT AND NOT A ONE-OFF. Three lane worktrees carry the same tree on an older commit, and each one has
# to end up here too. Everything below is a `git mv` (history follows the file), a manifest line, or a path constant, and
# every step checks before it acts, so running it twice is a no-op rather than an error.
#
# TWO PARTS. Part 1 moves the crate directories into the tiers. Part 2 is what the tree that landed also needed: the
# contract suite is one crate at the repository root (`tests/`, with `tests/tests/` holding the cases the three crates
# used to keep beside themselves and `tests/src/` holding the harness they run on), the container files moved to
# `devops/`, and every reference to a path under `rust/` — in CI, in the scripts, in the hooks, in the container files,
# in the maps — was re-spelled. `scripts/validate-move-script.sh` re-reads every `fix` line here and applies it to the
# original file from the base commit, so a rule that has gone stale is loud instead of silent.
#
# The layout it produces:
#
#   web/              the web tier. The HTTP surface crate, with ui/ and auth/ nested inside it.
#   web/ui/           the browser application
#   web/auth/         Google authentication
#   middle/model/     the model and the business rules (was domain)
#   middle/services/  the use cases (was service)
#   middle/workflow/  the workflow engine
#   middle/apis/      the outbound clients (was integrations)
#   db/               the SQL, the loads, and the adapter crate (was rust/core/db, merged into the existing db/)
#   cli/ forge/       entry points
#   tests/            the contract suite: tests/tests/ the cases, tests/src/ the harness
#   devops/           the container files (was deploy/, plus the two that lived in rust/)
#   .config/          the nextest profile, at the workspace root because that is where nextest looks
#
# Nothing is deleted except three dead things: a three-line scaffold (web/src/main.rs, whose binary is src/bin/web.rs and
# two binaries cannot both be called `web`), the `.dockerignore` that guarded a context rooted at `rust/`, and one empty
# placeholder file (`data/skills/svar-react`, with no reader anywhere in the tree).
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

MOVE_LOG=()
say() { printf '%s\n' "$*"; }

moved() { # moved <from> <to>
  local from="$1" to="$2"
  if [ ! -e "$from" ]; then
    say "skip    $from (already moved)"
    return 0
  fi
  if [ -e "$to" ]; then
    say "ERROR   $to already exists, refusing to overwrite"; exit 1
  fi
  mkdir -p "$(dirname "$to")"
  git mv "$from" "$to"
  say "move    $from -> $to"
}

swapped() { # swapped <manifest> <old-line> <new-line>
  local manifest="$1" from="$2" to="$3"
  [ -f "$manifest" ] || { say "ERROR   $manifest is missing"; exit 1; }
  if grep -qF "$from" "$manifest"; then
    FROM="$from" TO="$to" perl -pi -e 'BEGIN { $f = $ENV{FROM}; $t = $ENV{TO}; } s/^\Q$f\E$/$t/m' "$manifest"
    say "line    $manifest: $from  ->  $to"
  elif ! grep -qF "$to" "$manifest"; then
    say "ERROR   $manifest has neither: $from"; exit 1
  fi
}

# ---------------------------------------------------------------- the workspace manifest
moved rust/Cargo.toml Cargo.toml
moved rust/Cargo.lock Cargo.lock

# ---------------------------------------------------------------- tier 1: web
moved rust/server web
moved rust/ui web/ui
moved rust/core/auth web/auth

# ---------------------------------------------------------------- tier 2: middle
moved rust/core/domain middle/model
moved rust/core/service middle/services
moved rust/core/workflow middle/workflow
moved rust/integrations middle/apis

# ---------------------------------------------------------------- tier 3: db (the crate joins the data it already has)
if [ -d rust/core/db ]; then
  if [ ! -e db/Cargo.toml ]; then moved rust/core/db/Cargo.toml db/Cargo.toml; fi
  if [ ! -e db/src ]; then moved rust/core/db/src db/src; fi
  if [ -d rust/core/db/tests ] && [ ! -e db/tests ]; then moved rust/core/db/tests db/tests; fi
  rmdir rust/core/db 2>/dev/null || true
fi

# ---------------------------------------------------------------- the entry points
moved rust/cli cli
moved rust/forge forge
moved rust/experiments/pool-bench experiments/pool-bench
rmdir rust/core rust/experiments 2>/dev/null || true

# ---------------------------------------------------------------- the workspace, and the tier each crate sits in
perl -0pi -e 's{members = \[[^\]]*\]}{members = [
    "web",
    "web/ui",
    "web/auth",
    "middle/model",
    "middle/services",
    "middle/workflow",
    "middle/apis",
    "db",
    "cli",
    "forge",
    "rust/test-harness",
]}s' Cargo.toml
say "members Cargo.toml: the tier layout"

# ---------------------------------------------------------------- the crate names, and every path between them
# `server` -> `web`: the tier took the word, so the HTTP surface crate carries it and its binary is `web` too.
swapped web/Cargo.toml 'name = "server"' 'name = "web"'
swapped web/Cargo.toml 'auth = { path = "../core/auth" }' 'auth = { path = "auth" }'
swapped web/Cargo.toml 'db = { path = "../core/db" }' 'db = { path = "../db" }'
swapped web/Cargo.toml 'domain = { path = "../core/domain" }' 'model = { path = "../middle/model" }'
swapped web/Cargo.toml 'integrations = { path = "../integrations" }' 'apis = { path = "../middle/apis" }'
swapped web/Cargo.toml 'service = { path = "../core/service" }' 'services = { path = "../middle/services" }'

swapped web/ui/Cargo.toml 'domain = { path = "../core/domain" }' 'model = { path = "../../middle/model" }'
swapped web/auth/Cargo.toml 'domain = { path = "../domain" }' 'model = { path = "../../middle/model" }'

# `domain` -> `model`: the crate is the model, and a package named `core` would shadow std's own `core`.
swapped middle/model/Cargo.toml 'name = "domain"' 'name = "model"'

swapped middle/services/Cargo.toml 'name = "service"' 'name = "services"'
swapped middle/services/Cargo.toml 'domain = { path = "../domain" }' 'model = { path = "../model" }'

swapped middle/workflow/Cargo.toml 'domain = { path = "../domain" }' 'model = { path = "../model" }'
swapped middle/workflow/Cargo.toml 'db = { path = "../db" }' 'db = { path = "../../db" }'

# `integrations` -> `apis`: they are the clients that call other people's APIs.
swapped middle/apis/Cargo.toml 'name = "integrations"' 'name = "apis"'
swapped middle/apis/Cargo.toml 'domain = { path = "../core/domain" }' 'model = { path = "../model" }'
swapped middle/apis/Cargo.toml 'db = { path = "../core/db" }' 'db = { path = "../../db" }'
swapped middle/apis/Cargo.toml 'service = { path = "../core/service" }' 'services = { path = "../services" }'

swapped db/Cargo.toml 'domain = { path = "../domain" }' 'model = { path = "../middle/model" }'

swapped forge/Cargo.toml 'domain = { path = "../core/domain" }' 'model = { path = "../middle/model" }'
swapped forge/Cargo.toml 'db = { path = "../core/db" }' 'db = { path = "../db" }'
swapped forge/Cargo.toml 'workflow = { path = "../core/workflow" }' 'workflow = { path = "../middle/workflow" }'
swapped forge/Cargo.toml 'integrations = { path = "../integrations" }' 'apis = { path = "../middle/apis" }'
swapped forge/Cargo.toml 'service = { path = "../core/service" }' 'services = { path = "../middle/services" }'

swapped cli/Cargo.toml 'db = { path = "../core/db" }' 'db = { path = "../db" }'
swapped cli/Cargo.toml 'domain = { path = "../core/domain" }' 'model = { path = "../middle/model" }'
swapped cli/Cargo.toml 'integrations = { path = "../integrations" }' 'apis = { path = "../middle/apis" }'
swapped cli/Cargo.toml 'server = { path = "../server" }' 'web = { path = "../web" }'
swapped cli/Cargo.toml 'service = { path = "../core/service" }' 'services = { path = "../middle/services" }'
swapped cli/Cargo.toml 'workflow = { path = "../core/workflow" }' 'workflow = { path = "../middle/workflow" }'

# The harness moves last (it and the other test directories become the one `tests/` suite), so it keeps its depth and
# only its neighbours move out from under it.
swapped rust/test-harness/Cargo.toml 'db = { path = "../core/db" }' 'db = { path = "../../db" }'
swapped rust/test-harness/Cargo.toml 'domain = { path = "../core/domain" }' 'model = { path = "../../middle/model" }'
swapped rust/test-harness/Cargo.toml 'service = { path = "../core/service" }' 'services = { path = "../../middle/services" }'
swapped rust/test-harness/Cargo.toml 'ui = { path = "../ui" }' 'ui = { path = "../../web/ui" }'
swapped rust/test-harness/Cargo.toml 'workflow = { path = "../core/workflow" }' 'workflow = { path = "../../middle/workflow" }'
swapped rust/test-harness/Cargo.toml 'forge = { path = "../forge" }' 'forge = { path = "../../forge" }'

# ---------------------------------------------------------------- the binary: one name, not two
if [ -f web/src/main.rs ]; then
  # A three-line scaffold that prints "no production routes are wired yet", and the package-named binary. The real
  # server is src/bin/http.rs; after the rename both would want to be called `web`.
  git rm -q -f web/src/main.rs
  say "remove  web/src/main.rs (dead scaffold)"
fi
if [ -f web/src/bin/http.rs ]; then
  git mv web/src/bin/http.rs web/src/bin/web.rs
  say "move    web/src/bin/http.rs -> web/src/bin/web.rs"
fi

# ---------------------------------------------------------------- the form templates now live inside the model crate
if [ -d lib/forms/templates ] && [ ! -e middle/model/forms/templates ]; then
  mkdir -p middle/model/forms
  git mv lib/forms/templates middle/model/forms/templates
  say "move    lib/forms/templates -> middle/model/forms/templates"
fi
rmdir lib/forms 2>/dev/null || true

# ---------------------------------------------------------------- the path constants the moves invalidated
if [ -f middle/model/src/forms_template.rs ]; then
  perl -pi -e 's{"\.\./\.\./\.\./lib/forms/templates"}{"forms/templates"}' middle/model/src/forms_template.rs
fi
if [ -f middle/model/src/forms_template/parse_section.rs ]; then
  perl -pi -e 's{^pub const DEFAULT_TEMPLATES_DIR: &str = "lib/forms/templates";$}{pub const DEFAULT_TEMPLATES_DIR: &str = "forms/templates";}' \
    middle/model/src/forms_template/parse_section.rs
  perl -pi -e 's{^        \.join\("\.\./\.\./\.\."\)$}{        .join("../..")}' \
    middle/model/src/forms_template/parse_section.rs
fi
if [ -f web/src/vault/pdf.rs ]; then
  perl -pi -e 's{"\.\./\.\./public/brand/CLLOGO\.png"}{"../public/brand/CLLOGO.png"}' web/src/vault/pdf.rs
fi
if [ -f web/src/vault/artifact.rs ]; then
  perl -pi -e 's{"\.\./\.\./lib/forms/templates"}{"../middle/model/forms/templates"}' web/src/vault/artifact.rs
fi
if [ -f web/src/vault/forms_render.rs ]; then
  perl -pi -e 's{"\.\./\.\./lib/forms/templates"}{"../middle/model/forms/templates"}' web/src/vault/forms_render.rs
fi

# ---------------------------------------------------------------- every reference to a crate or a path that moved
# Tracked files only, and never this script: it has to keep naming the paths it moves, or a second run would find
# nothing to do and a lane worktree would silently stay on the old tree.
refs() { # refs <perl-expression> <what>
  local expression="$1" what="$2" list
  # Text files only: a search-and-replace over a committed image or wasm binary is a corruption waiting to happen.
  # The list goes through a file because a NUL-separated list cannot survive a shell variable.
  list=$(mktemp)
  git ls-files -z \
    | grep -zv '^scripts/restructure-domain-layout\.sh$' \
    | grep -zE '\.(rs|toml|sh|mjs|cjs|js|jsx|ts|tsx|json|ya?ml|md|css|html|plist|py|sql|xml|txt|zsh|swift|rb)$|(^|/)Dockerfile(\.[^/]*)?$|(^|/)\.(gitignore|dockerignore|env\.example)$|(^|/)Makefile$' > "$list" || true
  if [ ! -s "$list" ]; then say "refs    $what (no text files)"; rm -f "$list"; return 0; fi
  xargs -0 perl -pi -e "$expression" < "$list"
  rm -f "$list"
  say "refs    $what"
}

refs 's{(?<!crate::)(?<!self::)(?<!super::)\bdomain::}{model::}g' 'domain:: -> model::'
refs 's{(?<!crate::)(?<!self::)(?<!super::)\bservice::}{services::}g' 'service:: -> services::'
# `roles::service` is a LOCAL module (forge's role services), not the `services` crate, so the rule above must not see
# it: the negative lookbehind cannot express a two-segment prefix, so the false positive is undone below instead.
refs 's{\broles::services\b}{roles::service}g' 'roles::services -> roles::service (a local module)'
for manifest in forge/src/lib.rs forge/src/roles/mod.rs web/src/projects/mod.rs; do
  # All three declare/re-export a LOCAL module called `service`, which is what a bare `pub use service::…` in them
  # names (a crate-root item shadows the extern prelude), so the sweep's rewrite is undone here.
  [ -f "$manifest" ] || continue
  perl -pi -e 's|^pub use services::|pub use service::|' "$manifest"
  say "revert  $manifest: pub use services:: -> pub use service::"
done
# The alias import carries no `::`, so `service::` rules never saw it: `use service as service_kernel` aliases the
# services crate, and it has to follow the crate's new name.
refs 's{^((?:pub )?use) service as }{$1 services as }' 'use service as -> use services as'
refs 's{(?<!crate::)(?<!self::)(?<!super::)\bintegrations::}{apis::}g' 'integrations:: -> apis::'
refs 's{(?<!crate::)(?<!self::)(?<!super::)\bserver::}{web::}g' 'server:: -> web::'
# The bare re-exports (`pub use domain;` in db, workflow and auth) carry no `::`, so they need their own rules.
refs 's{^(pub use|use) domain;$}{$1 model;}' 'bare domain re-export -> model'
refs 's{^(pub use|use) service;$}{$1 services;}' 'bare service re-export -> services'
refs 's{^(pub use|use) integrations;$}{$1 apis;}' 'bare integrations re-export -> apis'
refs 's{^(pub use|use) server;$}{$1 web;}' 'bare server re-export -> web'
refs 's{\B-p domain\b}{-p model}g' '-p domain -> -p model'
refs 's{\B-p service\b}{-p services}g' '-p service -> -p services'
refs 's{\B-p integrations\b}{-p apis}g' '-p integrations -> -p apis'
refs 's{\B-p server\b}{-p web}g' '-p server -> -p web'
refs 's{--bin http\b}{--bin web}g' '--bin http -> --bin web'
refs 's{\brust/Cargo\.toml}{Cargo.toml}g' 'rust/Cargo.toml -> Cargo.toml'
refs 's{\brust/core/domain\b}{middle/model}g' 'rust/core/domain -> middle/model'
refs 's{\brust/core/service\b}{middle/services}g' 'rust/core/service -> middle/services'
refs 's{\brust/core/workflow\b}{middle/workflow}g' 'rust/core/workflow -> middle/workflow'
refs 's{\brust/core/auth\b}{web/auth}g' 'rust/core/auth -> web/auth'
refs 's{\brust/core/db\b}{db}g' 'rust/core/db -> db'
refs 's{\brust/integrations\b}{middle/apis}g' 'rust/integrations -> middle/apis'
refs 's{\brust/server\b}{web}g' 'rust/server -> web'
refs 's{\brust/ui\b}{web/ui}g' 'rust/ui -> web/ui'
refs 's{\brust/forge\b}{forge}g' 'rust/forge -> forge'
refs 's{\brust/cli\b}{cli}g' 'rust/cli -> cli'
refs 's{\brust/experiments/pool-bench\b}{experiments/pool-bench}g' 'rust/experiments -> experiments'
refs 's{\blib/forms/templates\b}{middle/model/forms/templates}g' 'lib/forms/templates -> middle/model/forms/templates'
# Crate-relative paths INSIDE the test constants: a harness test names `"ui/src"`, not `"rust/ui/src"`, because it was
# already standing in the workspace root. The rules above only saw the `rust/`-prefixed spellings, so the bare ones are
# re-rooted here. Longest first, or `"server/` would rewrite the `"web/` a previous rule just produced.
refs 's{"core/domain/}{"middle/model/}g' 'core/domain/ -> middle/model/'
refs 's{"core/domain}{"middle/model}g' 'core/domain -> middle/model (also the escaped form inside a Rust string)'
refs 's{"core/db/}{"db/}g' 'core/db/ -> db/'
refs 's{"core/service/}{"middle/services/}g' 'core/service/ -> middle/services/'
refs 's{"core/workflow/}{"middle/workflow/}g' 'core/workflow/ -> middle/workflow/'
refs 's{"core/auth/}{"web/auth/}g' 'core/auth/ -> web/auth/'
refs 's{"integrations/}{"middle/apis/}g' 'integrations/ -> middle/apis/'
refs 's{"ui/}{"web/ui/}g' 'ui/ -> web/ui/'
refs 's{"server/}{"web/}g' 'server/ -> web/'
# The same re-rooting in the guard constants and test roots that a `git mv` cannot see: crate-relative paths are
# strings in code, and the crate depth changed for every one of them. Targeted per file on purpose — a blanket
# `.join("../..")` rule would be wrong for the crates that now live TWO levels down in a tier (web/ui, middle/model).
fix() { # fix <file> <perl-expression>
  local file="$1" expression="$2"
  [ -f "$file" ] || { say "ERROR   $file is missing"; exit 1; }
  EXPRESSION="$expression" perl -0pi -e '$e = $ENV{EXPRESSION}; eval "\$_ =~ $e"; die "$@" if $@;' "$file"
  say "fix     $file"
}

fix web/tests/signature_routes.rs 's{\.join\("\.\./\.\."\)}{.join("..")}g'
fix web/tests/release_gate_routes.rs 's{"\.\./\.\./scripts/deploy-prod\.sh"}{"../scripts/deploy-prod.sh"}g'
fix db/tests/forge_seam_hold_dev.rs 's{join\("\.\./\.\./\.\."\)}{join("..")}g'
fix db/tests/forge_work_claim_dev.rs 's{join\("\.\./\.\./\.\."\)}{join("..")}g'
fix cli/src/launchd/agent_worker.rs 's{\.parent\(\)\n(\s*)\.and_then\(Path::parent\)}{.parent()}g'
fix cli/src/launchd/agent_worker.rs 's{cli sits two levels below the repository root}{cli sits one level below the repository root}g'
fix cli/src/forge/manifest.rs 's{\.parent\(\)\n(\s*)\.and_then\(\|path\| path\.parent\(\)\)}{.parent()}g'
fix forge/tests/handbook_engine_guards.rs 's{\.parent\(\)\n(\s*)\.and_then\(Path::parent\)}{.parent()}g'
fix forge/tests/handbook_engine_guards.rs 's{forge sits under rust/ under the repository root}{forge sits at the repository root}g'
fix forge/tests/opencode_v2_agents.rs 's{\.parent\(\)\n(\s*)\.and_then\(Path::parent\)}{.parent()}g'
fix web/src/vault/forms_render.rs 's{\.\./\.\./public/brand/CLLOGO\.png}{../public/brand/CLLOGO.png}g'
fix cli/src/forge/test_section.rs 's{"rust/Cargo\.lock"}{"Cargo.lock"}g'
fix cli/src/forge/repo_guards.rs 's{const RESIDUE_ROOTS: \[&str; 4\] = \["rust", "scripts", "\.githooks", "package\.json"\];}{const RESIDUE_ROOTS: [&str; 9] = [\n    "web",\n    "middle",\n    "db",\n    "cli",\n    "forge",\n    "rust",\n    "scripts",\n    ".githooks",\n    "package.json",\n];}'
fix cli/src/forge/repo_guards.rs 's{let files = tracked_files\(root, &\["rust"\]\);}{let files = tracked_files(root, &["web", "middle", "db", "cli", "forge", "rust"]);}'
fix cli/src/forge/citations.rs 's{const SOURCE_ROOTS: \[&str; 12\] = \[[\s\S]*?\];}{const SOURCE_ROOTS: [&str; 8] = [\n    "web",\n    "middle",\n    "db",\n    "cli",\n    "forge",\n    "rust",\n    "scripts",\n    "lib",\n];}'
fix forge/src/engine/assay.rs 's{const PRODUCTION_ROOTS: \[&str; 6\] = \[[\s\S]*?\];}{const PRODUCTION_ROOTS: [&str; 5] = ["web/", "middle/", "db/", "cli/", "forge/"];}'
# The harness resolves the tree from its own manifest, and it is the one crate that did NOT move: its walkers have to
# start at the repository root, which is now where every crate lives. Two exact block replacements rather than one
# pattern with a character class: `[^}]` inside a brace-delimited substitution ends the pattern early.
fix rust/test-harness/src/source.rs 's{pub fn repo_root\(\) -> PathBuf \{\n    rust_root\(\)\n        \.parent\(\)\n        \.expect\("the workspace lives in rust/ under the repository root"\)\n        \.to_path_buf\(\)\n\}}{pub fn repo_root() -> PathBuf {\n    Path::new(env!("CARGO_MANIFEST_DIR"))\n        .parent()\n        .and_then(Path::parent)\n        .expect("the harness lives in rust/test-harness, two levels below the repository root")\n        .to_path_buf()\n}}'
fix rust/test-harness/src/source.rs 's{pub fn rust_root\(\) -> PathBuf \{\n    Path::new\(env!\("CARGO_MANIFEST_DIR"\)\)\n        \.parent\(\)\n        \.expect\("the harness lives in rust/test-harness"\)\n        \.to_path_buf\(\)\n\}}{pub fn rust_root() -> PathBuf {\n    repo_root()\n}}'
fix rust/test-harness/tests/harness_self_test.rs 's{let rust_root = std::path::Path::new\(env!\("CARGO_MANIFEST_DIR"\)\)\n\s*\.parent\(\)\n\s*\.expect\("the harness lives under rust/"\)}{let rust_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))\n        .parent()\n        .and_then(|path| path.parent())\n        .expect("the harness lives in rust/test-harness, below the repository root")}'
fix rust/test-harness/tests/arch_boundary__002__no_mvi_screen_performs_direct_db_access.rs 's{std::path::Path::new\(env!\("CARGO_MANIFEST_DIR"\)\)\n\s*\.parent\(\)\n\s*\.expect\("the harness lives under rust/"\)}{std::path::Path::new(env!("CARGO_MANIFEST_DIR"))\n        .parent()\n        .and_then(std::path::Path::parent)\n        .expect("the harness lives in rust/test-harness, below the repository root")}'
# The crate names inside the guard lists and the section table: a rename the `::` rules cannot see, because these are
# bare string literals naming a crate rather than a path into one.
fix rust/test-harness/tests/arch_boundary__003__ui_cannot_import_db_crate.rs 's{"server",}{"web",}g; s{"service",}{"services",}g; s{"integrations",}{"apis",}g; s{key == "domain"}{key == "model"}; s{it lists domain}{it lists model}'
fix rust/test-harness/tests/arch_boundary__004__domain_cannot_depend_on_server_ui_integrations.rs 's{"server",}{"web",}g; s{"service",}{"services",}g; s{"integrations",}{"apis",}g; s{Some\("server"\.to_string\(\)\)}{Some("web".to_string())}'
fix cli/src/forge/test_section.rs 's{"domain"}{"model"}g; s{"server"}{"web"}g; s{"service"}{"services"}g; s{"integrations"}{"apis"}g'

# ================================================================ PART 2: the suite at the root, the container files, the sweep
#
# WHY PART 2 EXISTS. Part 1 produced the tiers. The tree that landed is not only the tiers: the contract suite became
# one crate at the repository root, the container files moved to devops/, and every reference to a path under `rust/`
# — in two CI workflows that compiled from it, in the scripts, in the hooks, in the container files and in the maps —
# was re-spelled. A lane that ran Part 1 alone would have a tree that compiles and a CI that runs cargo in a directory
# that no longer exists.
#
# The `refs` sweeps below are global by design: they cover the dated records too. A lane's diff is allowed to be tidier
# than the original commit's — the tree it produces is the same tree, which is what this script is for.

# ---------------------------------------------------------------- 2a. the suite: rust/test-harness/ -> tests/
moved rust/test-harness tests
for owner in web db forge; do
  if [ -d "$owner/tests" ]; then
    for file in "$owner"/tests/*.rs; do
      [ -e "$file" ] || continue
      git mv "$file" "tests/tests/$(basename "$file")"
      say "move    $file -> tests/tests/$(basename "$file")"
    done
    rmdir "$owner/tests" 2>/dev/null || true
  fi
done
swapped Cargo.toml '    "rust/test-harness",' '    "tests",'
swapped tests/Cargo.toml 'db = { path = "../../db" }' 'db = { path = "../db" }'
swapped tests/Cargo.toml 'model = { path = "../../middle/model" }' 'model = { path = "../middle/model" }'
swapped tests/Cargo.toml 'services = { path = "../../middle/services" }' 'services = { path = "../middle/services" }'
swapped tests/Cargo.toml 'ui = { path = "../../web/ui" }' 'ui = { path = "../web/ui" }'
swapped tests/Cargo.toml 'workflow = { path = "../../middle/workflow" }' 'workflow = { path = "../middle/workflow" }'
swapped tests/Cargo.toml 'forge = { path = "../../forge" }' 'forge = { path = "../forge" }'

# The suite sits ONE level below the repository root now (it was two, inside rust/test-harness), so each helper that
# walked up to the root loses a parent. The texts these rules match are the ones Part 1 wrote.
fix tests/src/source.rs 's{pub fn repo_root\(\) -> PathBuf \{\n    Path::new\(env!\("CARGO_MANIFEST_DIR"\)\)\n        \.parent\(\)\n        \.and_then\(Path::parent\)\n        \.expect\("the harness lives in rust/test-harness, two levels below the repository root"\)\n        \.to_path_buf\(\)\n\}}{pub fn repo_root() -> PathBuf {\n    Path::new(env!("CARGO_MANIFEST_DIR"))\n        .parent()\n        .expect("the suite lives in tests/, one level below the repository root")\n        .to_path_buf()\n}}'
fix tests/tests/harness_self_test.rs 's{\.parent\(\)\n        \.and_then\(\|path\| path\.parent\(\)\)\n        \.expect\("the harness lives in rust/test-harness, below the repository root"\)}{.parent()\n        .expect("the suite lives in tests/, one level below the repository root")}'
fix tests/tests/arch_boundary__002__no_mvi_screen_performs_direct_db_access.rs 's{\.parent\(\)\n        \.and_then\(std::path::Path::parent\)\n        \.expect\("the harness lives in rust/test-harness, below the repository root"\)}{.parent()\n        .expect("the suite lives in tests/, one level below the repository root")}'
fix tests/tests/durable_completion_ledger.rs 's{include_str!\("\.\./src/bin/forge\.rs"\)}{include_str!("../../forge/src/bin/forge.rs")}'
fix tests/tests/arch_boundary__010__entitlement_owns_action_screen_authorization.rs 's{relative\.starts_with\("rust/test-harness/"\)}{relative.starts_with("tests/")}'
fix tests/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs 's{const SELF: &str = "rust/test-harness/tests/}{const SELF: &str = "tests/tests/}'
fix tests/Cargo.toml 's{^(forge = \{ path = "\.\./forge" \})$}{$1\nweb = { path = "../web" }\ntokio-util.workspace = true}m'


# ---------------------------------------------------------------- 2b. the container files, and the end of rust/
moved deploy devops
moved rust/Dockerfile devops/Dockerfile
moved rust/Dockerfile.vercel devops/Dockerfile.vercel
moved rust/.config .config
moved rust/README.md docs/rust/README.md
moved rust/FORGE_CUTOVER.md docs/rust/FORGE_CUTOVER.md
moved rust/WORKFLOW_FORGE_DONE.md docs/rust/WORKFLOW_FORGE_DONE.md
if [ -f rust/.dockerignore ]; then
  git rm -q rust/.dockerignore
  say "remove  rust/.dockerignore (the context it ignored is the repository root now)"
fi
if [ -f data/skills/svar-react ]; then
  git rm -q data/skills/svar-react
  say "remove  data/skills/svar-react (an empty placeholder with no reader)"
fi
rmdir rust data/skills data 2>/dev/null || true

# ---------------------------------------------------------------- 2c. the sweep
refs 's{^[ \t]+working-directory: rust\r?\n}{}mg' 'CI: no step compiles from rust/'
refs 's{rust/test-harness/tests/}{tests/tests/}g' 'rust/test-harness/tests -> tests/tests'
refs 's{rust/test-harness}{tests}g' 'rust/test-harness -> tests'
refs 's{\brust/Dockerfile\.vercel}{devops/Dockerfile.vercel}g' 'rust/Dockerfile.vercel -> devops/Dockerfile.vercel'
refs 's{\brust/Dockerfile}{devops/Dockerfile}g' 'rust/Dockerfile -> devops/Dockerfile'
refs 's{\brust/core/auth\b}{web/auth}g' 'rust/core/auth -> web/auth'
refs 's{\brust/core/domain\b}{middle/model}g' 'rust/core/domain -> middle/model'
refs 's{\brust/core/service\b}{middle/services}g' 'rust/core/service -> middle/services'
refs 's{\brust/core/workflow\b}{middle/workflow}g' 'rust/core/workflow -> middle/workflow'
refs 's{\brust/core/db\b}{db}g' 'rust/core/db -> db'
refs 's{\brust/integrations\b}{middle/apis}g' 'rust/integrations -> middle/apis'
refs 's{\brust/server\b}{web}g' 'rust/server -> web'
refs 's{\brust/ui\b}{web/ui}g' 'rust/ui -> web/ui'
refs 's{\brust/experiments\b}{experiments}g' 'rust/experiments -> experiments'
refs 's{\brust/cli\b}{cli}g' 'rust/cli -> cli'
refs 's{\brust/forge\b}{forge}g' 'rust/forge -> forge'
refs 's{\brust/Cargo\.lock\b}{Cargo.lock}g' 'rust/Cargo.lock -> Cargo.lock'
refs 's{\brust/target\b}{target}g' 'rust/target -> target'
refs 's{\bdeploy/Dockerfile\.build\b}{devops/Dockerfile.build}g' 'deploy/Dockerfile.build -> devops/Dockerfile.build'
refs 's{\bdeploy/Dockerfile\.runtime\b}{devops/Dockerfile.runtime}g' 'deploy/Dockerfile.runtime -> devops/Dockerfile.runtime'
refs 's{\bdeploy/rust-api\b}{devops/rust-api}g' 'deploy/rust-api -> devops/rust-api'


# The files a bare sweep cannot fix: the CI steps that READ a path rather than name one, the container COPYs, the
# pre-push hook, and the guards whose subject is a crate rather than a string.
fix .github/workflows/gates.yml 's{workspaces: rust}{workspaces: .}; s{path: rust/target/nextest/ci/junit\.xml}{path: target/nextest/ci/junit.xml}; s{and rust/target\. The toolchain}{and target. The toolchain}; s{Fix with: cd rust && cargo fmt --all}{Fix with: cargo fmt --all}; s{core/domain/src/\(applemail\|apple_messages\)}{middle/model/src/(applemail|apple_messages)}g; s{core/domain/src/applemail, core/domain/src/apple_messages}{middle/model/src/applemail, middle/model/src/apple_messages}; s{core/domain/src/forms_font_metrics\.rs}{middle/model/src/forms_font_metrics.rs}; s{-p web --test service_harness_dev}{-p test-harness --test service_harness_dev}; s{-p web --test command_runtime_dev}{-p test-harness --test command_runtime_dev}; s{-p web --test service_atomicity_dev}{-p test-harness --test service_atomicity_dev}; s{-p web --test mq_runtime_dev}{-p test-harness --test mq_runtime_dev}'
fix .dockerignore 's{^!rust$}{!Cargo.toml\n!Cargo.lock\n!web\n!middle\n!db\n!cli\n!forge\n!tests}m; s{^rust/target$}{target}m'
fix Dockerfile 's{^COPY rust \./rust$}{COPY Cargo.toml Cargo.lock ./\nCOPY web middle db cli forge tests ./}m; s{/build/rust/target/release/http}{/build/target/release/web}'
fix devops/Dockerfile.build 's{^COPY rust \./rust$}{COPY Cargo.toml Cargo.lock ./\nCOPY web middle db cli forge tests ./}m; s{target=/build/rust/target}{target=/build/target}; s{cp rust/target/}{cp target/}; s{release/http}{release/web}'
fix devops/Dockerfile 's{^COPY rust \./rust\nWORKDIR /build/rust$}{COPY Cargo.toml Cargo.lock ./\nCOPY web middle db cli forge tests ./}m; s{/build/rust/target/release/http}{/build/target/release/web}'
fix devops/Dockerfile.vercel 's{/build/target/release/http}{/build/target/release/web}'
fix .githooks/pre-push 's{\$root/rust/Cargo\.toml}{\$root/Cargo.toml}g; s{rust/Cargo\.lock}{Cargo.lock}g; s{rust/ui}{web/ui}g; s{^      if has_prefix "rust/" "\$files"; then rust_changed=1; fi$}{      for prefix in "web/" "middle/" "db/" "cli/" "forge/" "tests/" "Cargo.toml" "Cargo.lock"; do\n        if has_prefix "\$prefix" "\$files"; then rust_changed=1; fi\n      done}m'
fix package.json 's{"start": "rust/target/release/http"}{"start": "target/release/web"}'
fix scripts/rust-ui-build.sh 's{\$root/rust/target}{\$root/target}'
fix scripts/rust-dev-boot-smoke.sh 's{rust/target/debug/http}{target/debug/web}'
fix scripts/build-all.sh 's{\(cd rust && (cargo [^)]*)\)}{$1}g; s{rust/target/release/http}{target/release/web}; s{"  server  "}{"  web     "}'
fix scripts/ops/gate/slice-check.sh 's{\(cd rust && (cargo [^)]*)\)}{$1}g'
fix scripts/rust-container-preflight.sh 's{\$ROOT_DIR/rust:/work:ro}{\$ROOT_DIR:/work:ro}; s{rust/Cargo\.lock}{Cargo.lock}g'
fix scripts/vercel-provision-rust-project.sh 's{cd "\$ROOT_DIR/rust"}{cd "\$ROOT_DIR"}'

# The guards whose subject is the tree itself: the crate lists in citations and sections, the basename resolver's
# source roots, and the two fences that read a file by a path spelled from the repository root.
fix cli/src/forge/citations.rs 's{^    "rust",$}{    "tests",}m'
fix cli/src/forge/test_section.rs 's{path\.starts_with\("rust/test-harness/"\)}{path.starts_with("tests/")}; s{"rust/test-harness/src/}{"tests/src/}g; s{"rust/test-harness/tests/}{"tests/tests/}g; s{^        "rust",$}{        "tests",}m'
fix cli/src/forge/repo_guards.rs 's{"rust/test-harness/src/git\.rs"}{"tests/src/git.rs"}; s{^    "rust",$}{    "tests",}m; s{&\["web", "middle", "db", "cli", "forge", "rust"\]}{&["web", "middle", "db", "cli", "forge", "tests"]}g; s{tracked_files\(root, &\["rust"\]\)}{tracked_files(root, &["web", "middle", "db", "cli", "forge", "tests"])}; s{path\.starts_with\("rust/test-harness/"\)}{path.starts_with("tests/")}'
fix cli/src/forge/lint.rs 's{"rust/a/thing\.rs"}{"web/a/thing.rs"}g; s{"rust/b/thing\.rs"}{"web/b/thing.rs"}; s{path_exists\("rust/a/\*\.rs"\)}{path_exists("web/a/*.rs")}; s{path_exists\("rust/a/\*\.ts"\)}{path_exists("web/a/*.ts")}'
fix middle/model/src/forms_template.rs 's{the API with `rust/` as its working directory}{the API from the repository root}'
fix web/src/site.rs 's{else `public/` from the repository root or from `rust/`\.}{else `public/` - looked for in the working directory and then one level above it.}'
fix AGENTS.md 's{are Rust under `rust/`}{are Rust in the tiers (`web/`, `middle/`, `db/`) with the entry points (`cli/`, `forge/`) beside them}'

# ---------------------------------------------------------------- the lockfile
# `Cargo.lock` moved to the workspace root and still names the packages that were renamed. One metadata call rewrites it
# in place: no build, and no re-resolution, because the crates.io entries are already there.
if command -v cargo >/dev/null 2>&1; then
  if cargo metadata --format-version 1 > /dev/null 2>&1; then
    say "lock    Cargo.lock refreshed for the new crate names"
  else
    say "note    Cargo.lock was not refreshed (cargo missing or offline): run cargo check"
  fi
fi

say ""
say "the three-tier layout is in place. Verify with: cargo check --workspace --all-targets"



