/**
 * Unit tests for dokploy/lib/dotenv.mjs
 * Covers: parseDotenv, renderDotenv, isSecret
 */

import { parseDotenv, renderDotenv, isSecret } from "../../../../dokploy/lib/dotenv.mjs";

describe("parseDotenv", () => {
  test("parses bare KEY=VALUE", () => {
    expect(parseDotenv("FOO=bar\nBAZ=qux\n")).toEqual({ FOO: "bar", BAZ: "qux" });
  });

  test("strips double-quoted values", () => {
    expect(parseDotenv('A="hello world"')).toEqual({ A: "hello world" });
  });

  test("strips single-quoted values", () => {
    expect(parseDotenv("A='hello world'")).toEqual({ A: "hello world" });
  });

  test("ignores blank lines and # comments", () => {
    const text = `
# This is a comment
FOO=1

BAR=2
`;
    expect(parseDotenv(text)).toEqual({ FOO: "1", BAR: "2" });
  });

  test("ignores lines without =", () => {
    expect(parseDotenv("NOEQUAL\nFOO=bar")).toEqual({ FOO: "bar" });
  });

  test("handles empty file", () => {
    expect(parseDotenv("")).toEqual({});
  });

  test("handles value with embedded = signs", () => {
    expect(parseDotenv("A=a=b=c")).toEqual({ A: "a=b=c" });
  });

  test("trims key whitespace", () => {
    expect(parseDotenv("  KEY  =value")).toEqual({ KEY: "value" });
  });
});

describe("renderDotenv", () => {
  test("preserves comment lines", () => {
    const prior = "# My env\nFOO=old\n";
    const result = renderDotenv({ FOO: "new" }, prior);
    expect(result).toContain("# My env");
    expect(result).toContain("FOO=new");
  });

  test("appends new keys at end", () => {
    const prior = "A=1\n";
    const result = renderDotenv({ A: "1", B: "2" }, prior);
    const lines = result.split("\n").filter(Boolean);
    expect(lines[0]).toBe("A=1");
    expect(lines[lines.length - 1]).toBe("B=2");
  });

  test("preserves unknown keys from prior text unchanged", () => {
    const prior = "A=1\nUNKNOWN=preserve-me\n";
    const result = renderDotenv({ A: "2" }, prior);
    expect(result).toContain("UNKNOWN=preserve-me");
  });

  test("replaces existing key with new value", () => {
    const prior = "PASSWORD=old\n";
    const result = renderDotenv({ PASSWORD: "new" }, prior);
    expect(result).toContain("PASSWORD=new");
    expect(result).not.toContain("PASSWORD=old");
  });

  test("handles empty prior text", () => {
    const result = renderDotenv({ A: "1" }, "");
    expect(result).toContain("A=1");
  });
});

describe("isSecret", () => {
  test.each([
    ["PASSWORD", true],
    ["POSTGRES_PASSWORD", true],
    ["SECRET_KEY", true],
    ["LAGO_SECRET_KEY_BASE", true],
    ["API_TOKEN", true],
    ["JWT_PRIVATE_KEY", true],
    ["RUSTFS_IAM_ENC_KEY", true],
    ["APP_DOMAIN", false],
    ["ZITADEL_ISSUER", false],
    ["POSTGRES_USER", false],
    ["POSTGRES_DB", false],
  ])("isSecret(%s) === %s", (key, expected) => {
    expect(isSecret(key)).toBe(expected);
  });
});
