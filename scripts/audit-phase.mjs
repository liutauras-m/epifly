#!/usr/bin/env node
/**
 * scripts/audit-phase.mjs  (Phase 1.4)
 *
 * Runs every CI gate applicable to a given phase and emits a single
 * PR-ready artifact bundle at test-results/audit-phase-N/.
 *
 * Usage:
 *   node scripts/audit-phase.mjs <phase>   e.g. node scripts/audit-phase.mjs 3
 *
 * Outputs:
 *   test-results/audit-phase-N/
 *     summary.md      — one-page Markdown summary (paste into PR description)
 *     gates.json      — machine-readable pass/fail per gate
 *     screenshots/    — symlink or copy of relevant visual test results
 *     axe/            — symlink or copy of axe reports
 *
 * Design goal: drop contributor cost of the per-phase audit gate (Principle #12)
 * from "assemble 6 artifacts by hand" to `pnpm audit:phase 3`.
 */

import { execSync }                             from 'node:child_process';
import { mkdirSync, writeFileSync, existsSync } from 'node:fs';
import { join }                                  from 'node:path';
import { fileURLToPath }                         from 'node:url';

const ROOT  = join(fileURLToPath(import.meta.url), '..', '..');
const phase = process.argv[2];

if (!phase || !/^\d+(\.\d+)?$/.test(phase)) {
  console.error('Usage: node scripts/audit-phase.mjs <phase>   e.g.: node scripts/audit-phase.mjs 3');
  process.exit(1);
}

const OUT_DIR = join(ROOT, `test-results/audit-phase-${phase}`);
mkdirSync(OUT_DIR, { recursive: true });

const now = new Date().toISOString();

// ── Gate registry ─────────────────────────────────────────────────────────────
// Each entry: { id, label, cmd, phases: Set<string|RegExp> }
// `phases` lists which plan phases activate this gate.
// All gates listed for a given phase are run; others are skipped.

const GATES = [
  // Always-on from Phase 0
  {
    id: 'contracts',
    label: 'UI contracts (import graph, landmark, brand vocab)',
    cmd: 'pnpm ui:contracts',
    phases: /^[0-9]/,          // all phases
  },
  {
    id: 'tokens',
    label: 'Design token audit (no raw hex/px literals)',
    cmd: 'pnpm ui:tokens:check',
    phases: /^[0-9]/,
  },
  {
    id: 'no-local',
    label: 'No app-local components check',
    cmd: 'pnpm ui:no-local',
    phases: /^[0-9]/,
  },
  {
    id: 'exports',
    label: 'Exports contract test',
    cmd: 'pnpm test:exports',
    phases: /^[0-9]/,
  },
  // Phase 2+ gates
  {
    id: 'lint',
    label: 'Lint (svelte-check + biome)',
    cmd: 'pnpm lint',
    phases: /^[2-9]/,
  },
  {
    id: 'unit',
    label: 'Unit tests (vitest)',
    cmd: 'pnpm test',
    phases: /^[2-9]/,
  },
  // Phase 6+ gates
  {
    id: 'motion-purpose',
    label: 'Motion purpose tags',
    cmd: 'pnpm ui:motion:purpose',
    phases: /^[6-9]/,
  },
  {
    id: 'motion-durations',
    label: 'Motion duration limits (≤ 400ms per-transition)',
    cmd: 'pnpm ui:motion:durations',
    phases: /^[6-9]/,
  },
  // Phase 8 gates (full sign-off)
  {
    id: 'e2e',
    label: 'Playwright E2E (web)',
    cmd: 'pnpm test:e2e',
    phases: /^8/,
  },
];

// ── Run gates ─────────────────────────────────────────────────────────────────

function run(cmd) {
  try {
    const out = execSync(cmd, { cwd: ROOT, stdio: 'pipe', encoding: 'utf-8' });
    return { ok: true, output: out };
  } catch (e) {
    return { ok: false, output: (e.stdout ?? '') + '\n' + (e.stderr ?? '') };
  }
}

const activeGates = GATES.filter((g) => {
  if (g.phases instanceof RegExp) return g.phases.test(phase);
  return g.phases.has(phase);
});

console.log(`\n── Phase ${phase} audit ── ${activeGates.length} gates ────────────────────\n`);

const results = [];
for (const gate of activeGates) {
  process.stdout.write(`  Running: ${gate.label} … `);
  const { ok, output } = run(gate.cmd);
  console.log(ok ? '✓' : '✗ FAILED');
  results.push({ id: gate.id, label: gate.label, cmd: gate.cmd, pass: ok, output });
}

// ── Summary markdown ──────────────────────────────────────────────────────────

const passed  = results.filter((r) => r.pass).length;
const failed  = results.filter((r) => !r.pass).length;
const allPass = failed === 0;

const summaryLines = [
  `# Phase ${phase} Audit — ${now}`,
  '',
  `**Status:** ${allPass ? '✅ All gates pass' : `❌ ${failed} gate(s) failed`}`,
  '',
  '## Gate results',
  '',
  '| Gate | Status |',
  '|---|---|',
  ...results.map((r) => `| ${r.label} | ${r.pass ? '✅ Pass' : '❌ Fail'} |`),
  '',
];

if (!allPass) {
  summaryLines.push('## Failures', '');
  for (const r of results.filter((r) => !r.pass)) {
    summaryLines.push(`### ${r.label}`, '```', r.output.trim().slice(0, 2000), '```', '');
  }
}

summaryLines.push(
  '## Next steps',
  '',
  '- Attach screenshots from `test-results/visual-web/` (web) and `test-results/visual-ios/` (iOS) to this PR.',
  '- Run `node scripts/cross-platform-diff.mjs` to verify web ↔ iOS diff ≤ 2%.',
  '- Confirm VoiceOver / axe landmark audit per `docs/ui-landmarks.md`.',
  `- Merge only when all Phase ${phase} exit criteria are met (see docs/ui-plan.md Phase ${phase}).`,
  '',
);

const summary = summaryLines.join('\n');
writeFileSync(join(OUT_DIR, 'summary.md'), summary, 'utf-8');
writeFileSync(
  join(OUT_DIR, 'gates.json'),
  JSON.stringify({ phase, timestamp: now, passed, failed, gates: results }, null, 2),
  'utf-8',
);

// ── Print summary ─────────────────────────────────────────────────────────────

console.log(`\n${summary}`);
console.log(`✓ Artifact bundle: ${OUT_DIR}/`);

process.exit(allPass ? 0 : 1);
