/**
 * Unit tests for dokploy/lib/compose-vars.mjs
 * Covers: extractComposeVars, renderProjectRefs
 */

import { writeFileSync, unlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { extractComposeVars, renderProjectRefs } from "../../../../dokploy/lib/compose-vars.mjs";

function writeTmp(content: string): string {
  const path = join(tmpdir(), `test-compose-${Date.now()}-${Math.random().toString(36).slice(2)}.yml`);
  writeFileSync(path, content, "utf8");
  return path;
}

describe("extractComposeVars", () => {
  test("extracts simple ${VAR} references", () => {
    const path = writeTmp(`
services:
  db:
    environment:
      POSTGRES_PASSWORD: \${POSTGRES_PASSWORD}
      POSTGRES_USER: \${POSTGRES_USER}
`);
    try {
      const vars = extractComposeVars(path);
      expect(vars).toContain("POSTGRES_PASSWORD");
      expect(vars).toContain("POSTGRES_USER");
    } finally {
      unlinkSync(path);
    }
  });

  test("extracts ${VAR:-default} references", () => {
    const path = writeTmp(`
services:
  app:
    image: \${APP_IMAGE:-myapp:latest}
`);
    try {
      expect(extractComposeVars(path)).toContain("APP_IMAGE");
    } finally {
      unlinkSync(path);
    }
  });

  test("extracts ${VAR:?error} references", () => {
    const path = writeTmp(`
services:
  app:
    environment:
      KEY: \${REQUIRED_KEY:?must be set}
`);
    try {
      expect(extractComposeVars(path)).toContain("REQUIRED_KEY");
    } finally {
      unlinkSync(path);
    }
  });

  test("deduplicates repeated references", () => {
    const path = writeTmp(`
services:
  a:
    environment:
      FOO: \${MY_VAR}
  b:
    environment:
      BAR: \${MY_VAR}
`);
    try {
      const vars = extractComposeVars(path);
      expect(vars.filter((v) => v === "MY_VAR")).toHaveLength(1);
    } finally {
      unlinkSync(path);
    }
  });

  test("returns sorted list", () => {
    const path = writeTmp(`
services:
  a:
    environment:
      Z_VAR: \${Z_VAR}
      A_VAR: \${A_VAR}
      M_VAR: \${M_VAR}
`);
    try {
      const vars = extractComposeVars(path);
      expect(vars).toEqual([...vars].sort());
    } finally {
      unlinkSync(path);
    }
  });

  test("returns empty array for compose with no variables", () => {
    const path = writeTmp(`
services:
  a:
    image: nginx:latest
`);
    try {
      expect(extractComposeVars(path)).toEqual([]);
    } finally {
      unlinkSync(path);
    }
  });

  test("ignores lowercase var-like patterns", () => {
    const path = writeTmp(`
# \${not_a_var} — only uppercase
services:
  a:
    environment:
      UPPER: \${UPPER_CASE}
`);
    try {
      const vars = extractComposeVars(path);
      expect(vars).toContain("UPPER_CASE");
      expect(vars).not.toContain("not_a_var");
    } finally {
      unlinkSync(path);
    }
  });
});

describe("renderProjectRefs", () => {
  test("renders ${{project.VAR}} references", () => {
    const result = renderProjectRefs(["FOO", "BAR"]);
    expect(result).toContain("FOO=${{project.FOO}}");
    expect(result).toContain("BAR=${{project.BAR}}");
  });

  test("ends with newline", () => {
    expect(renderProjectRefs(["A"])).toMatch(/\n$/);
  });

  test("returns one line per var", () => {
    const lines = renderProjectRefs(["A", "B", "C"]).trim().split("\n");
    expect(lines).toHaveLength(3);
  });

  test("empty array returns just a newline", () => {
    expect(renderProjectRefs([])).toBe("\n");
  });
});
