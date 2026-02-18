import { describe, it, expect } from "vitest";
import { numberWithCommas, formToSlug, formatTime, formatUptime } from "@/lib/format";

describe("numberWithCommas", () => {
  it("formats small numbers unchanged", () => {
    expect(numberWithCommas(42)).toBe("42");
    expect(numberWithCommas(0)).toBe("0");
    expect(numberWithCommas(999)).toBe("999");
  });

  it("adds commas for thousands", () => {
    expect(numberWithCommas(1000)).toBe("1,000");
    expect(numberWithCommas(1234567)).toBe("1,234,567");
    expect(numberWithCommas(1000000000)).toBe("1,000,000,000");
  });
});

describe("formToSlug", () => {
  it("maps known forms", () => {
    expect(formToSlug("Factorial")).toBe("factorial");
    expect(formToSlug("Palindromic")).toBe("palindromic");
    expect(formToSlug("Kbn")).toBe("kbn");
    expect(formToSlug("KBN")).toBe("kbn");
  });

  it("maps lowercase variants", () => {
    expect(formToSlug("factorial")).toBe("factorial");
    expect(formToSlug("palindromic")).toBe("palindromic");
    expect(formToSlug("kbn")).toBe("kbn");
  });

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

  it("lowercases unknown forms", () => {
    expect(formToSlug("SomeNewForm")).toBe("somenewform");
  });
});

describe("formatTime", () => {
  it("converts ISO string to locale string", () => {
    const result = formatTime("2026-01-15T12:00:00Z");
    // Just verify it returns a non-empty string (locale-specific format)
    expect(result).toBeTruthy();
    expect(typeof result).toBe("string");
  });
});

describe("formatUptime", () => {
  it("formats minutes only", () => {
    expect(formatUptime(0)).toBe("0m");
    expect(formatUptime(30)).toBe("0m");
    expect(formatUptime(60)).toBe("1m");
    expect(formatUptime(300)).toBe("5m");
  });

  it("formats hours and minutes", () => {
    expect(formatUptime(3600)).toBe("1h 0m");
    expect(formatUptime(3661)).toBe("1h 1m");
    expect(formatUptime(7200)).toBe("2h 0m");
    expect(formatUptime(90060)).toBe("25h 1m");
  });
});
