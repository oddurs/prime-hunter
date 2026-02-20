import { describe, expect, it } from "vitest";

import { validateArtifacts } from "@/app/releases/lib";

const GOOD_SHA = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

describe("releases validateArtifacts", () => {
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

  it("rejects non-array payloads", () => {
    const result = validateArtifacts({ os: "linux" });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toContain("array");
    }
  });

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
