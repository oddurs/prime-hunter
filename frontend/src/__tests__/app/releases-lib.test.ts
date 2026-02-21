/**
 * @file Tests for the releases library validation functions
 * @module __tests__/app/releases-lib
 *
 * Validates the `validateArtifacts` function from the releases library,
 * which is used by the Releases page when publishing new worker binary
 * releases. The function validates artifact metadata (os, arch, download
 * URL, SHA-256 checksum, optional signature URL) before submission.
 * Tests cover valid artifact acceptance, non-array rejection, invalid URL
 * scheme rejection (must be https), and malformed SHA-256 rejection
 * (must be exactly 64 hex characters).
 *
 * @see {@link ../../app/releases/lib} Source module (validateArtifacts)
 */
import { describe, expect, it } from "vitest";

import { validateArtifacts } from "@/app/releases/lib";

/** Valid 64-character hex SHA-256 hash for test fixtures. */
const GOOD_SHA = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

// Tests validateArtifacts: input validation for worker release artifact
// metadata before it is sent to the /api/releases API endpoint.
describe("releases validateArtifacts", () => {
  /** Verifies a well-formed artifact list with all required fields passes validation. */
  it("accepts a valid artifact list", () => {
    const result = validateArtifacts([
      {
        os: "linux",
        arch: "x86_64",
        url: "https://example.com/worker.tar.gz",
        sha256: GOOD_SHA,
        sig_url: "https://example.com/worker.tar.gz.sig",
      },
    ]);
    expect(result.ok).toBe(true);
  });

  /** Verifies non-array payloads (e.g. plain objects) are rejected with an error. */
  it("rejects non-array payloads", () => {
    const result = validateArtifacts({ os: "linux" });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toContain("array");
    }
  });

  /**
   * Verifies URL scheme validation (must be https, not ftp) and SHA-256
   * format validation (must be exactly 64 hex characters).
   */
  it("rejects invalid url and sha256", () => {
    const badUrl = validateArtifacts([
      {
        os: "linux",
        arch: "x86_64",
        url: "ftp://example.com/worker.tar.gz",
        sha256: GOOD_SHA,
      },
    ]);
    expect(badUrl.ok).toBe(false);

    const badSha = validateArtifacts([
      {
        os: "linux",
        arch: "x86_64",
        url: "https://example.com/worker.tar.gz",
        sha256: "abc123",
      },
    ]);
    expect(badSha.ok).toBe(false);
  });
});
