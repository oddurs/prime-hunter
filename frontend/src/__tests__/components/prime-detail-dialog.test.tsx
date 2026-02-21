/**
 * @file Tests for the PrimeDetailDialog modal component
 * @module __tests__/components/prime-detail-dialog
 *
 * Validates the full prime details modal that opens when a user clicks on a
 * prime record. Tests cover open/close state, expression title, form badge,
 * digit count formatting, proof method display, verification status (verified
 * with tier vs pending), loading state, search parameters JSON block,
 * re-verify and permalink buttons, and null prime fallback.
 *
 * @see {@link ../../components/prime-detail-dialog} Source component
 * @see {@link ../../hooks/use-primes} PrimeDetail type
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";

// Mock next/link to render a plain <a> for testing.
vi.mock("next/link", () => ({
  default: ({
    children,
    href,
  }: {
    children: React.ReactNode;
    href: string;
  }) => <a href={href}>{children}</a>,
}));

// Mock sonner
vi.mock("sonner", () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
  },
}));

// Mock lucide-react icons
vi.mock("lucide-react", () => ({
  CheckCircle2: () => <span data-testid="check-icon" />,
  Clock: () => <span data-testid="clock-icon" />,
  ExternalLink: () => <span data-testid="external-link-icon" />,
  RefreshCw: () => <span data-testid="refresh-icon" />,
  Loader2: () => <span data-testid="loader-icon" />,
}));

// Mock format module
vi.mock("@/lib/format", () => ({
  API_BASE: "http://localhost:3000",
  numberWithCommas: (x: number) =>
    x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ","),
  formatTime: (iso: string) => new Date(iso).toLocaleString(),
}));

// Mock JsonBlock
vi.mock("@/components/json-block", () => ({
  JsonBlock: ({ label, data }: { label: string; data: unknown }) => (
    <div data-testid="json-block">
      <span>{label}</span>
      <pre>{typeof data === "string" ? data : JSON.stringify(data)}</pre>
    </div>
  ),
}));

// Mock the Dialog components from shadcn/ui
vi.mock("@/components/ui/dialog", () => ({
  Dialog: ({
    children,
    open,
  }: {
    children: React.ReactNode;
    open: boolean;
  }) => (open ? <div data-testid="dialog">{children}</div> : null),
  DialogContent: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="dialog-content">{children}</div>
  ),
  DialogHeader: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  DialogTitle: ({
    children,
    className,
  }: {
    children: React.ReactNode;
    className?: string;
  }) => <h2 className={className}>{children}</h2>,
}));

import { PrimeDetailDialog } from "@/components/prime-detail-dialog";

const samplePrime = {
  id: 42,
  form: "factorial",
  expression: "100!+1",
  digits: 158,
  found_at: "2026-01-15T10:30:00Z",
  proof_method: "Pocklington",
  verified: true,
  verified_at: "2026-01-15T11:00:00Z",
  verification_method: "BPSW+MR",
  verification_tier: 2,
  search_params: null,
};

// Tests the PrimeDetailDialog modal: open/close states, prime data display
// (expression, form, digits, proof, verification), loading state, JSON params,
// action buttons (re-verify, permalink), and null prime fallback.
describe("PrimeDetailDialog", () => {
  const mockOnOpenChange = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders nothing when closed", () => {
    const { container } = render(
      <PrimeDetailDialog
        prime={samplePrime}
        open={false}
        onOpenChange={mockOnOpenChange}
      />
    );
    expect(container.querySelector("[data-testid='dialog']")).toBeNull();
  });

  it("renders dialog when open", () => {
    render(
      <PrimeDetailDialog
        prime={samplePrime}
        open={true}
        onOpenChange={mockOnOpenChange}
      />
    );
    expect(screen.getByTestId("dialog")).toBeInTheDocument();
  });

  it("displays prime expression as title", () => {
    render(
      <PrimeDetailDialog
        prime={samplePrime}
        open={true}
        onOpenChange={mockOnOpenChange}
      />
    );
    expect(screen.getByText("100!+1")).toBeInTheDocument();
  });

  it("displays form badge", () => {
    render(
      <PrimeDetailDialog
        prime={samplePrime}
        open={true}
        onOpenChange={mockOnOpenChange}
      />
    );
    expect(screen.getByText("factorial")).toBeInTheDocument();
  });

  it("displays digit count with commas", () => {
    render(
      <PrimeDetailDialog
        prime={{ ...samplePrime, digits: 45678 }}
        open={true}
        onOpenChange={mockOnOpenChange}
      />
    );
    expect(screen.getByText("45,678")).toBeInTheDocument();
  });

  it("displays proof method when present", () => {
    render(
      <PrimeDetailDialog
        prime={samplePrime}
        open={true}
        onOpenChange={mockOnOpenChange}
      />
    );
    expect(screen.getByText("Pocklington")).toBeInTheDocument();
  });

  it("shows verified status with tier", () => {
    render(
      <PrimeDetailDialog
        prime={samplePrime}
        open={true}
        onOpenChange={mockOnOpenChange}
      />
    );
    expect(screen.getByText("Tier 2")).toBeInTheDocument();
    expect(screen.getByTestId("check-icon")).toBeInTheDocument();
  });

  it("shows Pending when not verified", () => {
    render(
      <PrimeDetailDialog
        prime={{ ...samplePrime, verified: false }}
        open={true}
        onOpenChange={mockOnOpenChange}
      />
    );
    expect(screen.getByText("Pending")).toBeInTheDocument();
    expect(screen.getByTestId("clock-icon")).toBeInTheDocument();
  });

  it("displays loading state", () => {
    render(
      <PrimeDetailDialog
        prime={null}
        open={true}
        onOpenChange={mockOnOpenChange}
        loading={true}
      />
    );
    expect(screen.getByText("Loading...")).toBeInTheDocument();
    expect(
      screen.getByText("Loading prime details...")
    ).toBeInTheDocument();
  });

  it("shows verification details when verified", () => {
    render(
      <PrimeDetailDialog
        prime={samplePrime}
        open={true}
        onOpenChange={mockOnOpenChange}
      />
    );
    expect(screen.getByText("BPSW+MR")).toBeInTheDocument();
  });

  it("shows search parameters as JSON block when parseable", () => {
    render(
      <PrimeDetailDialog
        prime={{
          ...samplePrime,
          search_params: '{"start": 1, "end": 100}',
        }}
        open={true}
        onOpenChange={mockOnOpenChange}
      />
    );
    expect(screen.getByTestId("json-block")).toBeInTheDocument();
  });

  it("shows Re-verify button when showVerifyButton is true", () => {
    render(
      <PrimeDetailDialog
        prime={samplePrime}
        open={true}
        onOpenChange={mockOnOpenChange}
        showVerifyButton={true}
      />
    );
    expect(screen.getByText("Re-verify")).toBeInTheDocument();
  });

  it("hides Re-verify button by default", () => {
    render(
      <PrimeDetailDialog
        prime={samplePrime}
        open={true}
        onOpenChange={mockOnOpenChange}
      />
    );
    expect(screen.queryByText("Re-verify")).not.toBeInTheDocument();
  });

  it("shows Permalink button when showVerifyButton is true", () => {
    render(
      <PrimeDetailDialog
        prime={samplePrime}
        open={true}
        onOpenChange={mockOnOpenChange}
        showVerifyButton={true}
      />
    );
    expect(screen.getByText("Permalink")).toBeInTheDocument();
  });

  it("displays 'Prime detail' when prime is null and not loading", () => {
    render(
      <PrimeDetailDialog
        prime={null}
        open={true}
        onOpenChange={mockOnOpenChange}
        loading={false}
      />
    );
    expect(screen.getByText("Prime detail")).toBeInTheDocument();
  });
});
