/**
 * @file Tests for the JsonBlock component
 * @module __tests__/components/json-block
 *
 * Validates the JsonBlock component used to display JSON data in a formatted
 * <pre> block throughout the dashboard (search parameters, prime details,
 * agent task configs). Tests cover label rendering, string/object/array/nested/
 * null/numeric data formatting, and configurable maxHeight overflow control.
 *
 * @see {@link ../../components/json-block} Source component
 */
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { JsonBlock } from "@/components/json-block";

// Tests the JsonBlock component: label display, various data types
// (string, object, array, nested, null, number), and height overflow config.
describe("JsonBlock", () => {
  /** Verifies the label text renders above the JSON content. */
  it("renders label text", () => {
    render(<JsonBlock label="Parameters" data={{}} />);
    expect(screen.getByText("Parameters")).toBeInTheDocument();
  });

  /** Verifies string data renders without JSON.stringify quotes. */
  it("renders string data as-is", () => {
    render(<JsonBlock label="Raw" data="hello world" />);
    expect(screen.getByText("hello world")).toBeInTheDocument();
  });

  /** Verifies object data renders as pretty-printed JSON with 2-space indent. */
  it("renders object data as formatted JSON", () => {
    const data = { key: "value", count: 42 };
    render(<JsonBlock label="Config" data={data} />);
    const pre = screen.getByText(/key/);
    expect(pre).toBeInTheDocument();
    expect(pre.textContent).toContain('"key": "value"');
    expect(pre.textContent).toContain('"count": 42');
  });

  /** Verifies array data renders as a JSON array. */
  it("renders array data as formatted JSON", () => {
    const data = [1, 2, 3];
    render(<JsonBlock label="Items" data={data} />);
    const pre = screen.getByText(/1/);
    expect(pre.textContent).toContain("1");
    expect(pre.textContent).toContain("2");
    expect(pre.textContent).toContain("3");
  });

  /** Verifies deeply nested objects render their inner keys and values. */
  it("renders nested objects", () => {
    const data = { outer: { inner: "deep" } };
    render(<JsonBlock label="Nested" data={data} />);
    expect(screen.getByText(/inner/)).toBeInTheDocument();
    expect(screen.getByText(/deep/)).toBeInTheDocument();
  });

  /** Verifies null renders as the literal string "null". */
  it("renders null data", () => {
    render(<JsonBlock label="Empty" data={null} />);
    expect(screen.getByText("null")).toBeInTheDocument();
  });

  /** Verifies numeric data renders as its string representation. */
  it("renders numeric data", () => {
    render(<JsonBlock label="Number" data={42} />);
    expect(screen.getByText("42")).toBeInTheDocument();
  });

  /** Verifies the default max-h-40 Tailwind class for overflow control. */
  it("applies default maxHeight class", () => {
    const { container } = render(<JsonBlock label="Test" data="data" />);
    const pre = container.querySelector("pre");
    expect(pre?.className).toContain("max-h-40");
  });

  /** Verifies custom maxHeight prop overrides the default overflow class. */
  it("applies custom maxHeight class", () => {
    const { container } = render(
      <JsonBlock label="Test" data="data" maxHeight="max-h-96" />
    );
    const pre = container.querySelector("pre");
    expect(pre?.className).toContain("max-h-96");
  });
});
