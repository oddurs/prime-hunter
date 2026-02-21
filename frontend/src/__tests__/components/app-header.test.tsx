/**
 * @file Tests for the AppSidebar and TopBar navigation components
 * @module __tests__/components/app-header
 *
 * Validates the sidebar navigation and top bar that appear on every page of the
 * darkreach dashboard. Tests cover navigation link rendering, WebSocket
 * connection indicator (green/red dot), theme toggle button, running search
 * count badge, and role-based navigation filtering (admin vs operator).
 *
 * @see {@link ../../components/app-sidebar} Sidebar component
 * @see {@link ../../components/top-bar} Top bar component
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { AppSidebar } from "@/components/app-sidebar";
import { TopBar } from "@/components/top-bar";
import type { WsData } from "@/hooks/use-websocket";
import { defaultWsData } from "@/__mocks__/test-wrappers";

let mockWsData: WsData = { ...defaultWsData };

vi.mock("@/contexts/websocket-context", () => ({
  useWs: () => mockWsData,
}));

let mockRole: string | null = "admin";

vi.mock("@/contexts/auth-context", () => ({
  useAuth: () => ({
    user: { email: "tester@example.com" },
    session: null,
    loading: false,
    role: mockRole,
    operatorId: null,
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
  default: ({
    children,
    href,
    ...props
  }: {
    children: React.ReactNode;
    href: string;
    [key: string]: unknown;
  }) => (
    <a href={href} {...props}>
      {children}
    </a>
  ),
}));

// Mock sidebar components to simple divs so we can test content without SidebarProvider
vi.mock("@/components/ui/sidebar", () => ({
  Sidebar: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="sidebar">{children}</div>
  ),
  SidebarContent: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  SidebarFooter: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  SidebarGroup: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  SidebarGroupContent: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  SidebarGroupLabel: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  SidebarHeader: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  SidebarMenu: ({ children }: { children: React.ReactNode }) => (
    <ul>{children}</ul>
  ),
  SidebarMenuBadge: ({ children }: { children: React.ReactNode }) => (
    <span data-testid="sidebar-badge">{children}</span>
  ),
  SidebarMenuButton: ({
    children,
    ...props
  }: {
    children: React.ReactNode;
    asChild?: boolean;
    [key: string]: unknown;
  }) => <div {...props}>{children}</div>,
  SidebarMenuItem: ({ children }: { children: React.ReactNode }) => (
    <li>{children}</li>
  ),
  SidebarRail: () => <div />,
  SidebarSeparator: () => <hr />,
  SidebarTrigger: (props: Record<string, unknown>) => (
    <button {...props}>Toggle</button>
  ),
  SidebarInset: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  SidebarProvider: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  useSidebar: () => ({ state: "expanded", isMobile: false }),
}));

// Mock Sheet components to simple divs
vi.mock("@/components/ui/sheet", () => ({
  Sheet: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  SheetContent: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  SheetHeader: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  SheetTitle: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  SheetDescription: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  SheetClose: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
}));

describe("AppSidebar", () => {
  beforeEach(() => {
    mockPathname = "/";
    mockRole = "admin";
    mockWsData = { ...defaultWsData };
    vi.clearAllMocks();
  });

  /** Verifies that all primary navigation links render in the sidebar. */
  it("renders nav links", () => {
    render(<AppSidebar />);
    expect(screen.getAllByText("Dashboard").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("Searches").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("Agents").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("Browse").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("Docs").length).toBeGreaterThanOrEqual(1);
  });

  /** Verifies the numeric badge on the Searches item showing count of running searches. */
  it("shows running search count badge", () => {
    mockWsData = {
      ...defaultWsData,
      searches: [
        {
          id: 1,
          search_type: "kbn",
          params: { search_type: "kbn" },
          status: "running",
          started_at: "",
          stopped_at: null,
          pid: null,
          worker_id: "",
          tested: 0,
          found: 0,
        },
        {
          id: 2,
          search_type: "kbn",
          params: { search_type: "kbn" },
          status: "completed",
          started_at: "",
          stopped_at: null,
          pid: null,
          worker_id: "",
          tested: 0,
          found: 0,
        },
      ],
    };
    render(<AppSidebar />);
    const badges = screen.getAllByText("1");
    expect(badges.length).toBeGreaterThan(0);
  });

  /**
   * Verifies role-based navigation: operators see only Dashboard, Browse,
   * Leaderboard, and Docs. Admin-only links (Searches, Agents, Network,
   * Logs, Releases) are hidden.
   */
  it("shows limited nav for operator role", () => {
    mockRole = "operator";
    render(<AppSidebar />);
    expect(screen.getAllByText("Dashboard").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("Browse").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("Leaderboard").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("Docs").length).toBeGreaterThanOrEqual(1);
    // Admin-only items should not appear
    expect(screen.queryByText("Searches")).toBeNull();
    expect(screen.queryByText("Agents")).toBeNull();
    expect(screen.queryByText("Network")).toBeNull();
    expect(screen.queryByText("Logs")).toBeNull();
    expect(screen.queryByText("Releases")).toBeNull();
  });
});

describe("TopBar", () => {
  beforeEach(() => {
    mockPathname = "/";
    mockRole = "admin";
    mockWsData = { ...defaultWsData };
    vi.clearAllMocks();
  });

  /** Verifies the green dot indicator when the WebSocket is connected. */
  it("shows connection indicator", () => {
    mockWsData = { ...defaultWsData, connected: true };
    const { container } = render(<TopBar />);
    const dot = container.querySelector("[title='Connected']");
    expect(dot).toBeInTheDocument();
  });

  /** Verifies the red dot indicator when the WebSocket is disconnected. */
  it("shows disconnected indicator", () => {
    mockWsData = { ...defaultWsData, connected: false };
    const { container } = render(<TopBar />);
    const dot = container.querySelector("[title='Disconnected']");
    expect(dot).toBeInTheDocument();
  });

});
