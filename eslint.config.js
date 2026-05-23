// eslint.config.js — monorepo ESLint flat config (Phase 2.5 per docs/ui-plan.md)
//
// Svelte 5 runes migration ratchet:
//   Phase 2 exit  → packages/ui/src/lib/components/**  → error  ← ACTIVE
//   Phase 3 exit  → packages/ui/src/lib/features/**    → error  (uncomment below)
//   Phase 4 exit  → apps/**                            → error  (uncomment below)
//
// Run:  pnpm lint:svelte
// CI:   included in `just verify`

import sveltePlugin from 'eslint-plugin-svelte';
import tsPlugin from '@typescript-eslint/eslint-plugin';
import tsParser from '@typescript-eslint/parser';
import globals from 'globals';

// eslint-plugin-svelte v3 ships flat configs under configs['flat/recommended']
const svelteBase = sveltePlugin.configs['flat/recommended'];

export default [
  // ── Ignore generated / built output ─────────────────────────────────────
  {
    ignores: [
      '**/node_modules/**',
      '**/.svelte-kit/**',
      '**/dist/**',
      '**/build/**',
      'packages/ui/src/lib/tokens.css',
      'packages/ui/tokens/tokens.d.ts',
    ],
  },

  // ── Base JS/TS config ────────────────────────────────────────────────────
  {
    files: ['**/*.{js,mjs,ts}'],
    plugins: { '@typescript-eslint': tsPlugin },
    languageOptions: {
      parser: tsParser,
      globals: { ...globals.browser, ...globals.node },
    },
  },

  // ── Global Svelte floor: spread the recommended flat config ──────────────
  // Applies to all .svelte files; warns on reactive-reassign globally so the
  // full migration debt is visible in every pnpm lint:svelte run.
  //
  // Rules explicitly set to 'warn' here are Phase 3/4 migration targets —
  // they will be promoted to 'error' as each directory exits migration.
  ...svelteBase.map(cfg => ({
    ...cfg,
    files: ['**/*.svelte'],
    plugins: {
      ...(cfg.plugins ?? {}),
      '@typescript-eslint': tsPlugin,   // makes @typescript-eslint/* disable comments valid
    },
    languageOptions: {
      ...cfg.languageOptions,
      parserOptions: {
        ...(cfg.languageOptions?.parserOptions ?? {}),
        parser: tsParser,
        extraFileExtensions: ['.svelte'],
      },
      globals: { ...globals.browser },
    },
    rules: {
      ...(cfg.rules ?? {}),
      // ── Ratchet floor (all .svelte) ──────────────────────────────────────
      'svelte/no-reactive-reassign': 'warn',          // promoted to error per-dir below

      // ── Phase 3 targets (warn globally; error when features/ exits) ──────
      'svelte/require-each-key':             'warn',  // {#each} without key
      'svelte/no-useless-children-snippet':  'warn',  // snippet passed but not rendered
      'svelte/prefer-svelte-reactivity':     'warn',  // Set/Map → SvelteSet/SvelteMap

      // ── Phase 4 targets (warn globally; error when apps/ exits) ──────────
      'svelte/no-navigation-without-resolve':    'warn',  // SvelteKit 2 navigation
      'svelte/no-immutable-reactive-statements': 'warn',  // $: with immutable refs
    },
  })),

  // ── Phase 2 exit: components/ ratchet → ERROR ────────────────────────────
  // <svelte:options runes={true}> on every file is the compile-time gate;
  // this ESLint rule catches the most common runtime drift (reactive reassign).
  {
    files: ['packages/ui/src/lib/components/**/*.svelte'],
    rules: {
      'svelte/no-reactive-reassign': 'error',
    },
  },

  // ── Phase 3 exit placeholder ─────────────────────────────────────────────
  // Uncomment once packages/ui/src/lib/features/ migration lands:
  // { files: ['packages/ui/src/lib/features/**/*.svelte'],
  //   rules: {
  //     'svelte/no-reactive-reassign':          'error',
  //     'svelte/require-each-key':              'error',
  //     'svelte/no-useless-children-snippet':   'error',
  //     'svelte/prefer-svelte-reactivity':      'error',
  //   } },

  // ── Phase 4 exit placeholder ─────────────────────────────────────────────
  // Uncomment once apps/ migration lands:
  // { files: ['apps/**/*.svelte'],
  //   rules: {
  //     'svelte/no-reactive-reassign':              'error',
  //     'svelte/no-navigation-without-resolve':     'error',
  //     'svelte/no-immutable-reactive-statements':  'error',
  //   } },
];
