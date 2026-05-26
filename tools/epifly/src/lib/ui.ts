/**
 * Terminal UI helpers for epifly CLI.
 * Thin wrappers over picocolors + Node console.
 */

import pc from "picocolors";

// ── Banner ───────────────────────────────────────────────────────────────────

export function banner(title: string): void {
  const line = "─".repeat(64);
  console.log(pc.dim(line));
  console.log(`  ${pc.bold(pc.cyan("epifly"))} ${pc.dim("›")} ${pc.bold(title)}`);
  console.log(pc.dim(line));
}

// ── Status lines ─────────────────────────────────────────────────────────────

export function ok(msg: string): void {
  console.log(`${pc.green("✓")} ${msg}`);
}

export function warn(msg: string): void {
  console.warn(`${pc.yellow("⚠")} ${pc.yellow(msg)}`);
}

export function err(msg: string): void {
  console.error(`${pc.red("✗")} ${pc.red(msg)}`);
}

export function info(msg: string): void {
  console.log(`${pc.blue("·")} ${msg}`);
}

export function section(title: string): void {
  console.log();
  console.log(pc.bold(pc.dim(`▼ ${title}`)));
}

// ── Table ────────────────────────────────────────────────────────────────────

export function table(rows: Array<[string, string, string?]>): void {
  const col1 = Math.max(...rows.map(([a]) => a.length), 0);
  const col2 = Math.max(...rows.map(([, b]) => b.length), 0);
  for (const [a, b, c] of rows) {
    const status = c === "ok" ? pc.green("ok") : c === "error" ? pc.red("error") : c ? pc.dim(c) : "";
    console.log(`  ${a.padEnd(col1)}  ${b.padEnd(col2)}  ${status}`);
  }
}

// ── Fatal exit ────────────────────────────────────────────────────────────────

export function fatal(msg: string, hint?: string): never {
  err(msg);
  if (hint) console.error(pc.dim(`  ${hint}`));
  process.exit(1);
}

// ── Spinner helper (no dep, just dots) ────────────────────────────────────────

export function makeSpinner(label: string): { stop: (result?: string) => void } {
  let stopped = false;
  process.stdout.write(`  ${pc.dim("·")} ${label} `);
  const t = setInterval(() => {
    if (!stopped) process.stdout.write(".");
  }, 400);
  return {
    stop(result?: string) {
      stopped = true;
      clearInterval(t);
      if (result) {
        process.stdout.write(` ${result}\n`);
      } else {
        process.stdout.write(" done\n");
      }
    },
  };
}
