/**
 * @file Tests for the MetricsBar component
 * @module __tests__/components/metrics-bar
 *
 * Validates the horizontal progress bar used to display hardware utilization
 * metrics (CPU, memory, disk) on host node cards. Tests cover label/percentage
 * rendering, detail text, color thresholds (green < 70%, yellow 70-89%,
 * red >= 90%), and percentage clamping to the 0-100% range.
 *
 * @see {@link ../../components/metrics-bar} Source component
 */
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { MetricsBar } from "@/components/metrics-bar";

// Tests the MetricsBar component: label/percentage display, detail text,
// color thresholds (green/yellow/red), and value clamping.
describe("MetricsBar", () => {
  /** Verifies the bar label and percentage value render correctly. */
  it("renders label and percentage", () => {
    render(<MetricsBar label="CPU" percent={45} />);
    expect(screen.getByText("CPU")).toBeInTheDocument();
    expect(screen.getByText("45.0%")).toBeInTheDocument();
  });

  /** Verifies the optional detail text (e.g. "4.2 / 8.0 GB") renders alongside. */
  it("renders custom detail text", () => {
    render(<MetricsBar label="Memory" percent={60} detail="4.2 / 8.0 GB" />);
    expect(screen.getByText("Memory")).toBeInTheDocument();
    expect(screen.getByText("4.2 / 8.0 GB")).toBeInTheDocument();
  });

  /** Verifies green bar color for utilization below 70%. */
  it("uses green color below 70%", () => {
    const { container } = render(<MetricsBar label="CPU" percent={50} />);
    const bar = container.querySelector("[style]");
    expect(bar?.className).toContain("bg-green-500");
  });

  /** Verifies yellow bar color for utilization between 70% and 89%. */
  it("uses yellow color at 70-89%", () => {
    const { container } = render(<MetricsBar label="CPU" percent={75} />);
    const bar = container.querySelector("[style]");
    expect(bar?.className).toContain("bg-yellow-500");
  });

  /** Verifies red bar color for critical utilization at 90% or above. */
  it("uses red color at 90%+", () => {
    const { container } = render(<MetricsBar label="CPU" percent={95} />);
    const bar = container.querySelector("[style]");
    expect(bar?.className).toContain("bg-red-500");
  });

  /** Verifies percentages above 100 are clamped to 100% bar width. */
  it("clamps percentage to 0-100", () => {
    const { container } = render(<MetricsBar label="Disk" percent={150} />);
    const bar = container.querySelector("[style]") as HTMLElement;
    expect(bar?.style.width).toBe("100%");
  });

  /** Verifies negative percentages are clamped to 0.0%. */
  it("clamps negative percentage to 0", () => {
    render(<MetricsBar label="Disk" percent={-10} />);
    expect(screen.getByText("0.0%")).toBeInTheDocument();
  });
});
