import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AppHeader } from "@/components/app-header";
import type { WsData } from "@/hooks/use-websocket";
import { defaultWsData } from "@/__mocks__/test-wrappers";

// Track theme toggle calls
const mockToggleTheme = vi.fn();
let mockTheme = "dark";

vi.mock("@/hooks/use-theme", () => ({
  useTheme: () => ({
    theme: mockTheme,
    toggleTheme: mockToggleTheme,
    setTheme: vi.fn(),
  }),
}));

let mockWsData: WsData = { ...defaultWsData };

vi.mock("@/contexts/websocket-context", () => ({
  useWs: () => mockWsData,
}));

vi.mock("@/contexts/auth-context", () => ({
  useAuth: () => ({
    user: { email: "tester@example.com" },
    session: null,
    loading: false,
    signIn: vi.fn(),
    signOut: vi.fn(),
  }),
}));

let mockPathname = "/";

vi.mock("next/navigation", () => ({
  usePathname: () => mockPathname,
}));

// Mock next/link to render a regular <a>
vi.mock("next/link", () => ({
  default: ({ children, href, ...props }: { children: React.ReactNode; href: string; [key: string]: unknown }) => (
    <a href={href} {...props}>{children}</a>
  ),
}));

// Mock Sheet components to simple divs
vi.mock("@/components/ui/sheet", () => ({
  Sheet: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  SheetContent: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  SheetHeader: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  SheetTitle: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  SheetClose: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}));

describe("AppHeader", () => {
  beforeEach(() => {
    mockPathname = "/";
    mockTheme = "dark";
    mockWsData = { ...defaultWsData };
    vi.clearAllMocks();
  });

  it("renders nav links", () => {
    render(<AppHeader />);
    // Both desktop and mobile navs render, so use getAllByText
    expect(screen.getAllByText("Dashboard").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("Searches").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("Agents").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("Browse").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("Docs").length).toBeGreaterThanOrEqual(1);
  });

  it("shows connection indicator", () => {
    mockWsData = { ...defaultWsData, connected: true };
    const { container } = render(<AppHeader />);
    const dot = container.querySelector("[title='Connected']");
    expect(dot).toBeInTheDocument();
  });

  it("shows disconnected indicator", () => {
    mockWsData = { ...defaultWsData, connected: false };
    const { container } = render(<AppHeader />);
    const dot = container.querySelector("[title='Disconnected']");
    expect(dot).toBeInTheDocument();
  });

  it("calls toggleTheme on button click", async () => {
    const user = userEvent.setup();
    render(<AppHeader />);

    const themeBtn = screen.getByLabelText("Switch to light mode");
    await user.click(themeBtn);
    expect(mockToggleTheme).toHaveBeenCalled();
  });

  it("shows running search count badge", () => {
    mockWsData = {
      ...defaultWsData,
      searches: [
        { id: 1, search_type: "kbn", params: { search_type: "kbn" }, status: "running", started_at: "", stopped_at: null, pid: null, worker_id: "", tested: 0, found: 0 },
        { id: 2, search_type: "kbn", params: { search_type: "kbn" }, status: "completed", started_at: "", stopped_at: null, pid: null, worker_id: "", tested: 0, found: 0 },
      ],
    };
    render(<AppHeader />);
    // Should show "1" badge on Searches tab
    const badges = screen.getAllByText("1");
    expect(badges.length).toBeGreaterThan(0);
  });
});
