/**
 * @file Security tests: detect leaked secrets in frontend source
 * @module __tests__/security/no-secrets
 *
 * Guards against OWASP A07:2021 (Security Misconfiguration) and
 * A02:2021 (Cryptographic Failures) by scanning all frontend source files
 * for patterns that indicate leaked secrets. This is a static analysis
 * test that runs as part of the unit test suite.
 *
 * Attack vectors guarded against:
 * - Supabase service role key exposure (grants admin DB access)
 * - Hardcoded passwords/secrets in source files
 * - Private key material (RSA, EC) committed to version control
 * - Third-party API keys (Stripe, AWS, GitHub, Slack) in source
 * - .env files with non-public secrets committed to the repo
 *
 * NEXT_PUBLIC_ environment variables are intentionally excluded from
 * detection as they are designed to be client-visible (e.g. Supabase
 * anon key). The test only flags server-side/private secrets.
 *
 * @see https://owasp.org/Top10/A07_2021-Security_Misconfiguration/
 * @see https://owasp.org/Top10/A02_2021-Cryptographic_Failures/
 */
import { describe, it, expect } from "vitest";
import * as fs from "fs";
import * as path from "path";

/**
 * Regex patterns that match common secret formats. Each pattern targets
 * a specific credential type that should never appear in source files.
 * The patterns are designed to minimize false positives while catching
 * real leaks.
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

// Files excluded from scanning: test infrastructure, dependencies, build output.
// Test files themselves may contain example secret-like patterns in assertions.
const SKIP_PATTERNS = [
  /node_modules/,
  /\.test\./,
  /__tests__/,
  /__mocks__/,
  /package-lock\.json/,
  /\.next/,
  /out\//,
];

/** Recursively collects source file paths, skipping excluded directories. */
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

// OWASP A07/A02 — scans all frontend source and config files for secret patterns.
// Violations cause the test to fail with the file path and secret type identified.
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

  /**
   * Scans all .ts/.tsx/.js/.jsx/.json/.env/.yaml source files for regex
   * patterns matching known secret formats. Reports all violations as an
   * array of "file: secret type" strings that must be empty.
   */
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

  /**
   * Verifies that any committed .env files do not contain non-NEXT_PUBLIC_
   * secrets (KEY, SECRET, PASSWORD, TOKEN assignments). The .env files
   * should ideally be gitignored; this test catches cases where they slip
   * through.
   */
  it("no .env files with secrets are committed", () => {
    const envFiles = rootFiles.filter((f) => {
      const content = fs.readFileSync(f, "utf-8");
      // Check for non-NEXT_PUBLIC_ secrets
      return /^(?!#)(?!NEXT_PUBLIC_).*(?:KEY|SECRET|PASSWORD|TOKEN)\s*=/m.test(content);
    });

    expect(envFiles).toEqual([]);
  });
});
