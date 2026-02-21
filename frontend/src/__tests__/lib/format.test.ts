/**
 * @file Tests for the format utility module
 * @module __tests__/lib/format
 *
 * Validates the formatting utility functions used across all dashboard
 * components. Covers:
 * - `numberWithCommas`: thousand-separator formatting for digit counts
 *   and statistics. Edge cases: 0, 999 (no comma), 1000+ (commas).
 * - `formToSlug`: maps prime form names to URL slugs for routing.
 *   Known forms map directly; newer forms map to "new-forms"; unknown
 *   forms are lowercased.
 * - `formatTime`: ISO 8601 timestamp to locale string conversion.
 * - `formatUptime`: seconds to human-readable "Xh Ym" format.
 *
 * @see {@link ../../lib/format} Source module
 */
import { describe, it, expect } from "vitest";
import { numberWithCommas, formToSlug, formatTime, formatUptime } from "@/lib/format";

// Tests numberWithCommas: adds thousand separators to integers.
// Rule: commas are inserted every 3 digits from the right.
describe("numberWithCommas", () => {
  /** Verifies numbers below 1000 render without commas. */
  it("formats small numbers unchanged", () => {
    expect(numberWithCommas(42)).toBe("42");
    expect(numberWithCommas(0)).toBe("0");
    expect(numberWithCommas(999)).toBe("999");
  });

  /** Verifies comma insertion at thousand, million, and billion boundaries. */
  it("adds commas for thousands", () => {
    expect(numberWithCommas(1000)).toBe("1,000");
    expect(numberWithCommas(1234567)).toBe("1,234,567");
    expect(numberWithCommas(1000000000)).toBe("1,000,000,000");
  });
});

// Tests formToSlug: maps prime form display names to URL-safe slugs.
// Known Tier 1 forms map directly; newer forms collapse to "new-forms".
describe("formToSlug", () => {
  /** Verifies Tier 1 forms (Factorial, Palindromic, KBN) map to lowercase slugs. */
  it("maps known forms", () => {
    expect(formToSlug("Factorial")).toBe("factorial");
    expect(formToSlug("Palindromic")).toBe("palindromic");
    expect(formToSlug("Kbn")).toBe("kbn");
    expect(formToSlug("KBN")).toBe("kbn");
  });

  /** Verifies case-insensitive matching for already-lowercased form names. */
  it("maps lowercase variants", () => {
    expect(formToSlug("factorial")).toBe("factorial");
    expect(formToSlug("palindromic")).toBe("palindromic");
    expect(formToSlug("kbn")).toBe("kbn");
  });

  /**
   * Verifies all 13 newer prime forms (added after initial release) collapse
   * to the "new-forms" slug for the consolidated routing approach.
   */
  it("maps newer forms to new-forms", () => {
    const newForms = [
      "primorial", "cullen", "woodall", "cullen_woodall",
      "wagstaff", "carol", "kynea", "carol_kynea",
      "twin", "sophie_germain", "repunit", "gen_fermat",
      "near_repdigit",
    ];
    for (const form of newForms) {
      expect(formToSlug(form)).toBe("new-forms");
    }
  });

  /** Verifies completely unknown form names are simply lowercased. */
  it("lowercases unknown forms", () => {
    expect(formToSlug("SomeNewForm")).toBe("somenewform");
  });
});

// Tests formatTime: converts ISO 8601 timestamps to locale-formatted strings.
describe("formatTime", () => {
  /** Verifies the output is a non-empty string (format is locale-specific). */
  it("converts ISO string to locale string", () => {
    const result = formatTime("2026-01-15T12:00:00Z");
    // Just verify it returns a non-empty string (locale-specific format)
    expect(result).toBeTruthy();
    expect(typeof result).toBe("string");
  });
});

// Tests formatUptime: converts seconds to human-readable "Xh Ym" format.
// Edge cases: 0 seconds = "0m", sub-minute rounds down, 25+ hours OK.
describe("formatUptime", () => {
  /** Verifies sub-hour durations render as minutes only (no "0h" prefix). */
  it("formats minutes only", () => {
    expect(formatUptime(0)).toBe("0m");
    expect(formatUptime(30)).toBe("0m");
    expect(formatUptime(60)).toBe("1m");
    expect(formatUptime(300)).toBe("5m");
  });

  /** Verifies hour+minute format, including values exceeding 24 hours. */
  it("formats hours and minutes", () => {
    expect(formatUptime(3600)).toBe("1h 0m");
    expect(formatUptime(3661)).toBe("1h 1m");
    expect(formatUptime(7200)).toBe("2h 0m");
    expect(formatUptime(90060)).toBe("25h 1m");
  });
});
