// ---------------------------------------------------------------------------
// ESLint flat config (CulebraLuxe).
//
// WHY THIS FILE EXISTS AT ALL: `package.json` has carried `"lint": "eslint ."` since the initial
// commit (`0dd54454`), but eslint was never a dependency and there was no config, so the script could
// never run — it failed with `eslint: command not found`, then (once eslint WAS installed on
// 2026-09-16) with `couldn't find an eslint.config.* file`, because ESLint 9 dropped `.eslintrc`
// support. A script that cannot run is not a gate. This is the config that makes it one.
//
// SCOPE: the application and engine sources (`app`, `components`, `lib`, `services`, `db`,
// `agent-runtime`, `workflow_app`, `scripts`, `testv2`, `ui`, root config files). Generated,
// vendored and not-ours directories are ignored below rather than "cleaned up" — lint has no
// business reporting on a checkout it does not own.
//
// RULESET: `typescript-eslint` recommended, type-UNAWARE on purpose. Type-aware linting
// (`recommendedTypeChecked`) needs a full project service, costs minutes per run, and would make
// `pnpm lint` too slow to be a habit. It is available later as its own decision.
// ---------------------------------------------------------------------------

import js from '@eslint/js'
import tseslint from 'typescript-eslint'

/**
 * Node globals for the plain `.mjs` scripts in `scripts/`.
 *
 * Listed explicitly rather than pulling in the `globals` package: these scripts run under plain node,
 * the set is small and stable, and spelling it out keeps this config readable. `no-undef` is LEFT ON for
 * JavaScript files — on a JS file it is the rule that catches a typo'd identifier; on a TS file it is not
 * (see the TS block below).
 */
const NODE_GLOBALS = {
  process: 'readonly',
  console: 'readonly',
  Buffer: 'readonly',
  global: 'readonly',
  module: 'readonly',
  exports: 'writable',
  require: 'readonly',
  __dirname: 'readonly',
  __filename: 'readonly',
  setTimeout: 'readonly',
  clearTimeout: 'readonly',
  setInterval: 'readonly',
  clearInterval: 'readonly',
  setImmediate: 'readonly',
  queueMicrotask: 'readonly',
  fetch: 'readonly',
  Headers: 'readonly',
  Request: 'readonly',
  Response: 'readonly',
  FormData: 'readonly',
  AbortController: 'readonly',
  AbortSignal: 'readonly',
  URL: 'readonly',
  URLSearchParams: 'readonly',
  TextEncoder: 'readonly',
  TextDecoder: 'readonly',
  structuredClone: 'readonly',
  performance: 'readonly',
  crypto: 'readonly',
  atob: 'readonly',
  btoa: 'readonly',
}

export default tseslint.config(
  {
    ignores: [
      'node_modules/**',
      '.next/**',
      '.vercel/**',
      'output/**',
      'public/**',
      // Vendored / separate projects that happen to live in this tree.
      // `.venv` is a Python virtualenv created 2026-09-20 for YAML tooling; it ships vendored JS we do not own, and
      // without this line ESLint walks it and fails on `self` in pip's bundled urllib3 worker.
      '.venv/**',
      'venv/**',
      'gsd-core/**',
      'praxis/**',
      'claude-orchestrate/**',
      'OCR/**',
      'grok/**',
      'whatsapp/**',
      // Not ours: untracked working directories that predate this config.
      'data/**',
      'skills/**',
      'apple-messages-export/**',
      'contact-export/**',
      // Engine scratch/runtime state.
      '.forge/**',
      '.forge-context/**',
      '**/*.d.ts',
      // Generated wasm-bindgen glue (`scripts/rust-ui-build.sh` writes it, the release wasm build owns it). It
      // references the browser's own globals - Element, WebAssembly, window - which `no-undef` cannot see, and it is
      // not ours to restyle. Ignored for the same reason as the vendored directories above.
      'lib/rust-ui/ui.js',
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ['**/*.ts', '**/*.tsx'],
    rules: {
      /**
       * `no-undef` IS OFF FOR TYPESCRIPT, DELIBERATELY.
       *
       * typescript-eslint's own guidance: TypeScript already refuses an undefined identifier at compile
       * time, and the rule produces false positives on types, enums and global augmentations. Leaving it on
       * produced 533 findings here — every one of them a phantom, and a gate full of phantoms is one people
       * learn to ignore. `tsc --noEmit` is the identifier check for these files; it already runs.
       */
      'no-undef': 'off',
    },
  },
  // ---------------------------------------------------------------------------
  // THE WEBSITE'S SCOPE BOUNDARY.
  //
  // `legacy/` is out of scope for the primary website. It is retired TypeScript, moved out of the
  // main tree, and kept only because callers still route through it. The direction of travel is one
  // way: the website reaches the domain through the Rust API (`lib/rust-api`), or a Rust route is
  // added for what it needs - it does not reach into `legacy/`.
  //
  // This rule does not ask anyone to fix the files that still import it. It freezes them. There is no
  // way to write a rule like this and have it pass on day one, so the current offenders live in
  // `eslint-suppressions.json` as the burndown: `pnpm lint` stays green, and the list may only shrink.
  //
  //   npx eslint . --prune-suppressions   # after removing an import, drop its stale entry
  //
  // Scope is the website's own files. `legacy/` is not policed by this rule - it is out of scope.
  // ---------------------------------------------------------------------------
  {
    files: [
      'app/**/*.{ts,tsx}',
      'components/**/*.{ts,tsx}',
      'lib/**/*.{ts,tsx}',
      'middleware.ts',
      'instrumentation.ts',
      'auth.ts',
      'neon.ts',
    ],
    rules: {
      'no-restricted-imports': [
        'error',
        {
          patterns: [
            {
              group: ['@/legacy/*', '@/legacy', '**/legacy/db/*', '**/legacy/services/*', '**/legacy/workflow_app/*'],
              message:
                'legacy/ is out of scope for the primary website. Call the Rust API (lib/rust-api) instead, or add the Rust route this needs. If the import is unavoidable for now, it is already in eslint-suppressions.json - that list may only shrink.',
            },
          ],
        },
      ],
    },
  },
  // THE ROWS ROUTES ARE THE ONE ALLOWED SEAM.
  //
  // They exist to read the read models on behalf of the Rust screens, so they import `legacy/` by design - that is
  // what they are for. This is an override rather than a suppression on purpose: a suppression is debt that gets
  // pruned the moment the rule stops firing, and this is not debt, it is the boundary working. The import lives here
  // and nowhere else; when this feed moves into rust/server (see docs/layers/SERVICES.md), the exception goes with it.
  {
    files: ['app/api/portal/rust-ui/rows/route.ts', 'app/api/rust-ui/public-rows/route.ts'],
    rules: {
      'no-restricted-imports': 'off',
    },
  },
  {
    files: ['**/*.{mjs,cjs,js,jsx}'],
    languageOptions: {
      globals: NODE_GLOBALS,
    },
  },
  {
    files: ['**/*.{ts,tsx,mjs,cjs,js,jsx}'],
    languageOptions: {
      ecmaVersion: 'latest',
      sourceType: 'module',
    },
    rules: {
      // A leading underscore is the established "deliberately unused" marker in this repo (esp. in
      // adapter hooks that must keep a signature). Honouring it keeps the rule about signal.
      '@typescript-eslint/no-unused-vars': [
        'error',
        {
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_',
          caughtErrorsIgnorePattern: '^_',
        },
      ],
    },
  },
  {
    // Tests and probes legitimately reach for `any`/`never` to build fixtures and fakes.
    files: ['**/*.test.ts', '**/tests/**/*.ts', 'testv2/**/*.ts'],
    rules: {
      '@typescript-eslint/no-explicit-any': 'off',
    },
  },
)
