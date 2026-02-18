import { describe, it, expect } from "vitest";
import * as fs from "fs";
import * as path from "path";

/**
 * Scan frontend source files for patterns that look like leaked secrets.
 * This test catches accidentally committed API keys, private keys, passwords, etc.
 *
 * Note: NEXT_PUBLIC_ env vars are intentionally public (Supabase anon key).
 * We only flag patterns that look like private/server-side secrets.
 */

const SECRET_PATTERNS = [
  // Private API keys (not NEXT_PUBLIC_ which are intentionally public)
  { pattern: /(?<!NEXT_PUBLIC_)SUPABASE_SERVICE_ROLE_KEY\s*[:=]/i, name: "Supabase service role key" },
  { pattern: /(?<!\/\/.*)(?:password|passwd|secret)\s*[:=]\s*["'][^"']{8,}/i, name: "Hardcoded password/secret" },
  { pattern: /-----BEGIN (?:RSA |EC )?PRIVATE KEY-----/, name: "Private key" },
  { pattern: /(?:sk_live|sk_test)_[a-zA-Z0-9]{20,}/, name: "Stripe secret key" },
  { pattern: /AKIA[0-9A-Z]{16}/, name: "AWS access key" },
  { pattern: /(?:ghp|gho|ghu|ghs|ghr)_[a-zA-Z0-9]{36,}/, name: "GitHub token" },
  { pattern: /xox[bporas]-[0-9]{10,}-[a-zA-Z0-9]+/, name: "Slack token" },
];

// Files to skip (test files, node_modules, lock files)
const SKIP_PATTERNS = [
  /node_modules/,
  /\.test\./,
  /__tests__/,
  /__mocks__/,
  /package-lock\.json/,
  /\.next/,
  /out\//,
];

function collectSourceFiles(dir: string): string[] {
  const files: string[] = [];
  if (!fs.existsSync(dir)) return files;

  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (SKIP_PATTERNS.some((p) => p.test(fullPath))) continue;

    if (entry.isDirectory()) {
      files.push(...collectSourceFiles(fullPath));
    } else if (/\.(ts|tsx|js|jsx|json|env|yaml|yml)$/.test(entry.name)) {
      files.push(fullPath);
    }
  }
  return files;
}

describe("No secrets in frontend source", () => {
  const frontendDir = path.resolve(__dirname, "../../..");
  const sourceFiles = collectSourceFiles(path.join(frontendDir, "src"));
  // Also check root config files
  const rootFiles = [
    path.join(frontendDir, ".env"),
    path.join(frontendDir, ".env.local"),
    path.join(frontendDir, ".env.production"),
  ].filter((f) => fs.existsSync(f));

  const allFiles = [...sourceFiles, ...rootFiles];

  it("source files do not contain secret patterns", () => {
    const violations: string[] = [];

    for (const filePath of allFiles) {
      const content = fs.readFileSync(filePath, "utf-8");
      for (const { pattern, name } of SECRET_PATTERNS) {
        if (pattern.test(content)) {
          const relative = path.relative(frontendDir, filePath);
          violations.push(`${relative}: ${name}`);
        }
      }
    }

    expect(violations).toEqual([]);
  });

  it("no .env files with secrets are committed", () => {
    // .env files should not exist in source (they should be gitignored)
    const envFiles = rootFiles.filter((f) => {
      const content = fs.readFileSync(f, "utf-8");
      // Check for non-NEXT_PUBLIC_ secrets
      return /^(?!#)(?!NEXT_PUBLIC_).*(?:KEY|SECRET|PASSWORD|TOKEN)\s*=/m.test(content);
    });

    expect(envFiles).toEqual([]);
  });
});
