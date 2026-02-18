import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { MetricsBar } from "@/components/metrics-bar";

describe("MetricsBar", () => {
  it("renders label and percentage", () => {
    render(<MetricsBar label="CPU" percent={45} />);
    expect(screen.getByText("CPU")).toBeInTheDocument();
    expect(screen.getByText("45.0%")).toBeInTheDocument();
  });

  it("renders custom detail text", () => {
    render(<MetricsBar label="Memory" percent={60} detail="4.2 / 8.0 GB" />);
    expect(screen.getByText("Memory")).toBeInTheDocument();
    expect(screen.getByText("4.2 / 8.0 GB")).toBeInTheDocument();
  });

  it("uses green color below 70%", () => {
    const { container } = render(<MetricsBar label="CPU" percent={50} />);
    const bar = container.querySelector("[style]");
    expect(bar?.className).toContain("bg-green-500");
  });

  it("uses yellow color at 70-89%", () => {
    const { container } = render(<MetricsBar label="CPU" percent={75} />);
    const bar = container.querySelector("[style]");
    expect(bar?.className).toContain("bg-yellow-500");
  });

  it("uses red color at 90%+", () => {
    const { container } = render(<MetricsBar label="CPU" percent={95} />);
    const bar = container.querySelector("[style]");
    expect(bar?.className).toContain("bg-red-500");
  });

  it("clamps percentage to 0-100", () => {
    const { container } = render(<MetricsBar label="Disk" percent={150} />);
    const bar = container.querySelector("[style]") as HTMLElement;
    expect(bar?.style.width).toBe("100%");
  });

  it("clamps negative percentage to 0", () => {
    render(<MetricsBar label="Disk" percent={-10} />);
    expect(screen.getByText("0.0%")).toBeInTheDocument();
  });
});
