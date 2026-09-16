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
