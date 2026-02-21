/**
 * @file Tests for the utility functions module
 * @module __tests__/lib/utils
 *
 * Validates the `cn` utility function, which combines `clsx` (conditional
 * class joining) with `tailwind-merge` (Tailwind CSS class deduplication).
 * This function is used by every component in the codebase via shadcn/ui.
 * Tests cover basic merging, conditional class inclusion, Tailwind conflict
 * resolution (e.g. "p-4" + "p-2" = "p-2"), null/undefined handling, and
 * empty input edge cases.
 *
 * @see {@link ../../lib/utils} Source module
 */
import { describe, it, expect } from "vitest";
import { cn } from "@/lib/utils";

// Tests the cn utility: class name merging with Tailwind-aware deduplication.
// This is the foundation of all component styling in the dashboard.
describe("cn", () => {
  /** Verifies basic string concatenation of two class names. */
  it("merges class names", () => {
    expect(cn("foo", "bar")).toBe("foo bar");
  });

  /** Verifies falsy values (false, undefined) are excluded from the output. */
  it("handles conditional classes", () => {
    expect(cn("base", false && "hidden", "visible")).toBe("base visible");
  });

  /** Verifies tailwind-merge resolves conflicting utilities (last wins). */
  it("deduplicates conflicting tailwind classes", () => {
    // tailwind-merge should resolve p-4 vs p-2 to the last one
    expect(cn("p-4", "p-2")).toBe("p-2");
  });

  /** Verifies undefined and null values are safely ignored. */
  it("handles undefined and null", () => {
    expect(cn("base", undefined, null, "end")).toBe("base end");
  });

  /** Verifies empty or no arguments return an empty string. */
  it("handles empty inputs", () => {
    expect(cn()).toBe("");
    expect(cn("")).toBe("");
  });
});
