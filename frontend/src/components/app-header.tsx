"use client";

/**
 * @module app-header
 *
 * Top navigation bar for the dashboard. Contains the darkreach logo,
 * page links (Dashboard, Browse, Searches, Network, Docs, Agents),
 * a connection status indicator, notification bell with in-app notifications,
 * theme toggle (dark/light), user avatar dropdown, and mobile hamburger menu.
 * Highlights the current route.
 */

import { useState, useRef, useCallback, useEffect } from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { Bell, BellOff, ChevronDown, LogOut, Menu, Moon, Sun, User } from "lucide-react";
import { useWs } from "@/contexts/websocket-context";
import { useAuth } from "@/contexts/auth-context";
import { useTheme } from "@/hooks/use-theme";
import { useBrowserNotifications } from "@/hooks/use-notifications";
import { cn } from "@/lib/utils";
import { API_BASE, relativeTime } from "@/lib/format";
import { DarkReachLogo } from "@/components/darkreach-logo";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Switch } from "@/components/ui/switch";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetClose,
} from "@/components/ui/sheet";

export function AppHeader() {
  const pathname = usePathname();
  const { connected, searches, agentTasks, notifications } = useWs();
  const { user, role, signOut } = useAuth();
  const { theme, toggleTheme } = useTheme();
  const { supported, permission, enabled, setEnabled } = useBrowserNotifications();
  const [mobileOpen, setMobileOpen] = useState(false);

  // Track when the user last opened the notification dropdown to compute unread count
  const lastSeenRef = useRef<number>(0);
  const [unreadCount, setUnreadCount] = useState(0);

  // Recompute unread count whenever notifications change
  useEffect(() => {
    const count = notifications.filter(
      (n) => n.timestamp_ms > lastSeenRef.current
    ).length;
    setUnreadCount(count);
  }, [notifications]);

  // When the notification dropdown closes, mark all as seen
  const handleNotifOpenChange = useCallback(
    (open: boolean) => {
      if (!open) {
        if (notifications.length > 0) {
          lastSeenRef.current = Math.max(
            ...notifications.map((n) => n.timestamp_ms)
          );
        }
        setUnreadCount(0);
      }
    },
    [notifications]
  );

  // User initials for avatar
  const initials = user?.email
    ? user.email
        .split("@")[0]
        .split(/[._-]/)
        .slice(0, 2)
        .map((s) => s[0]?.toUpperCase() ?? "")
        .join("")
    : "?";

  const runningCount = searches.filter((s) => s.status === "running").length;
  const activeAgentCount = agentTasks.filter(
    (t) => t.status === "in_progress"
  ).length;
  const errorBudgetErrorsPerHour =
    Number(process.env.NEXT_PUBLIC_ERROR_BUDGET_ERRORS_PER_HOUR) || 10;
  const errorBudgetWarningsPerHour =
    Number(process.env.NEXT_PUBLIC_ERROR_BUDGET_WARNINGS_PER_HOUR) || 50;
  const [budgetStatus, setBudgetStatus] = useState<"Healthy" | "Risk" | "Breached">(
    "Healthy"
  );

  useEffect(() => {
    let active = true;
    async function fetchBudget() {
      try {
        const to = new Date();
        const from = new Date(Date.now() - 60 * 60 * 1000);
        const params = new URLSearchParams({
          from: from.toISOString(),
          to: to.toISOString(),
        });
        const res = await fetch(`${API_BASE}/api/observability/report?${params}`);
        if (!res.ok) return;
        const data = (await res.json()) as {
          budget?: { status?: string };
          logs?: { by_level?: Array<[string, number]> };
        };
        const reportStatus = data.budget?.status;
        if (reportStatus === "breached") {
          if (active) setBudgetStatus("Breached");
          return;
        }
        if (reportStatus === "risk") {
          if (active) setBudgetStatus("Risk");
          return;
        }
        if (reportStatus === "healthy") {
          if (active) setBudgetStatus("Healthy");
          return;
        }

        const byLevel = data.logs?.by_level ?? [];
        const errorCount =
          byLevel.find(([level]) => level === "error")?.[1] ?? 0;
        const warnCount =
          byLevel.find(([level]) => level === "warning" || level === "warn")?.[1] ??
          0;
        const errorsPerHour = errorCount;
        const warningsPerHour = warnCount;
        const nextStatus =
          errorsPerHour > errorBudgetErrorsPerHour
            ? "Breached"
            : warningsPerHour > errorBudgetWarningsPerHour
              ? "Risk"
              : "Healthy";
        if (active) setBudgetStatus(nextStatus);
      } catch {
        // ignore
      }
    }
    fetchBudget();
    const timer = setInterval(fetchBudget, 60000);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, [errorBudgetErrorsPerHour, errorBudgetWarningsPerHour]);

  type NavLink = { title: string; href: string; count?: number };
  type NavGroup = { title: string; items: NavLink[] };
  type NavEntry = NavLink | NavGroup;

  function isGroup(entry: NavEntry): entry is NavGroup {
    return "items" in entry;
  }

  const isAdmin = role === "admin";

  const navEntries: NavEntry[] = isAdmin
    ? [
        { title: "Dashboard", href: "/" },
        {
          title: "Discovery",
          items: [
            { title: "Browse", href: "/browse" },
            { title: "Searches", href: "/searches", count: runningCount || undefined },
            { title: "Projects", href: "/projects" },
            { title: "Leaderboard", href: "/leaderboard" },
          ],
        },
        {
          title: "Operations",
          items: [
            { title: "Network", href: "/network" },
            { title: "Observability", href: "/performance" },
            { title: "Logs", href: "/logs" },
            { title: "Releases", href: "/releases" },
            { title: "Strategy", href: "/strategy" },
          ],
        },
        { title: "Agents", href: "/agents", count: activeAgentCount || undefined },
        { title: "Docs", href: "/docs" },
      ]
    : [
        { title: "Dashboard", href: "/" },
        { title: "Browse", href: "/browse" },
        { title: "My Nodes", href: "/my-nodes" },
        { title: "Leaderboard", href: "/leaderboard" },
        { title: "Account", href: "/account" },
        { title: "Docs", href: "/docs" },
      ];

  // Flat list for mobile menu
  const navItems = navEntries.flatMap((entry) =>
    isGroup(entry) ? entry.items : [entry]
  );

  function isActive(href: string) {
    return href === "/" ? pathname === "/" : pathname.startsWith(href);
  }

  function isGroupActive(group: NavGroup) {
    return group.items.some((item) => isActive(item.href));
  }

  /** Aggregate badge count for a dropdown group */
  function groupCount(group: NavGroup) {
    const total = group.items.reduce((sum, item) => sum + (item.count ?? 0), 0);
    return total || undefined;
  }

  return (
    <header className="sticky top-0 z-50 flex h-14 items-center border-b px-6">
      <div className="mx-auto flex w-full max-w-6xl items-center gap-6">
        {/* Logo */}
        <Link href="/" className="flex items-center gap-2 flex-shrink-0">
          <DarkReachLogo size={22} className="text-[#6366f1]" />
          <span className="font-semibold text-[var(--header-foreground)] tracking-tight text-sm">
            darkreach
          </span>
        </Link>

        {/* Desktop nav */}
        <nav className="hidden md:flex items-center gap-1">
          {navEntries.map((entry) => {
            if (isGroup(entry)) {
              const active = isGroupActive(entry);
              const count = groupCount(entry);
              return (
                <DropdownMenu key={entry.title}>
                  <DropdownMenuTrigger asChild>
                    <button
                      className={cn(
                        "relative flex items-center gap-1 px-3 py-1 text-sm font-medium transition-colors rounded-md",
                        active
                          ? "text-white"
                          : "text-[var(--header-foreground)]/70 hover:text-[var(--header-foreground)] hover:bg-white/[0.12]"
                      )}
                    >
                      {entry.title}
                      {count != null && (
                        <span className="inline-flex items-center justify-center min-w-[18px] h-[18px] px-1 text-[11px] font-semibold leading-none rounded-full bg-white/[0.15] text-[var(--header-foreground)]">
                          {count}
                        </span>
                      )}
                      <ChevronDown className="size-3 opacity-60" />
                      {active && (
                        <span className="absolute bottom-[-13px] left-2 right-2 h-[2px] bg-[#f78166] rounded-full" />
                      )}
                    </button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="start" className="min-w-[160px]">
                    {entry.items.map((item) => (
                      <DropdownMenuItem key={item.href} asChild>
                        <Link
                          href={item.href}
                          className={cn(
                            "flex items-center justify-between gap-4 cursor-pointer",
                            isActive(item.href) && "font-semibold"
                          )}
                        >
                          {item.title}
                          {item.count != null && (
                            <span className="inline-flex items-center justify-center min-w-[18px] h-[18px] px-1 text-[11px] font-semibold leading-none rounded-full bg-muted text-muted-foreground">
                              {item.count}
                            </span>
                          )}
                        </Link>
                      </DropdownMenuItem>
                    ))}
                  </DropdownMenuContent>
                </DropdownMenu>
              );
            }

            const active = isActive(entry.href);
            return (
              <Link
                key={entry.href}
                href={entry.href}
                className={cn(
                  "relative flex items-center gap-1.5 px-3 py-1 text-sm font-medium transition-colors rounded-md",
                  active
                    ? "text-white"
                    : "text-[var(--header-foreground)]/70 hover:text-[var(--header-foreground)] hover:bg-white/[0.12]"
                )}
              >
                {entry.title}
                {entry.count != null && (
                  <span className="inline-flex items-center justify-center min-w-[18px] h-[18px] px-1 text-[11px] font-semibold leading-none rounded-full bg-white/[0.15] text-[var(--header-foreground)]">
                    {entry.count}
                  </span>
                )}
                {active && (
                  <span className="absolute bottom-[-13px] left-2 right-2 h-[2px] bg-[#f78166] rounded-full" />
                )}
              </Link>
            );
          })}
        </nav>

        {/* Right side */}
        <div className="ml-auto flex items-center gap-3">
          <div
            className={cn(
              "size-2 rounded-full flex-shrink-0",
              connected ? "bg-green-500" : "bg-red-500"
            )}
            title={connected ? "Connected" : "Disconnected"}
          />
          {budgetStatus !== "Healthy" && (
            <span
              className={cn(
                "px-2 py-0.5 rounded-full text-[11px] font-semibold border",
                budgetStatus === "Breached"
                  ? "bg-red-500/20 text-red-400 border-red-500/30"
                  : "bg-amber-500/20 text-amber-300 border-amber-500/30"
              )}
            >
              Budget {budgetStatus}
            </span>
          )}
          {/* Notification bell with in-app notifications */}
          <DropdownMenu onOpenChange={handleNotifOpenChange}>
            <DropdownMenuTrigger asChild>
              <button
                className="relative flex size-8 items-center justify-center rounded-md text-[var(--header-foreground)]/60 hover:text-[var(--header-foreground)] hover:bg-white/[0.12] transition-colors"
                aria-label="Notifications"
              >
                {enabled ? (
                  <Bell className="size-4" />
                ) : (
                  <BellOff className="size-4" />
                )}
                {unreadCount > 0 && (
                  <span className="absolute -top-0.5 -right-0.5 flex size-4 items-center justify-center rounded-full bg-[#f78166] text-[10px] font-bold text-white leading-none">
                    {unreadCount > 9 ? "9+" : unreadCount}
                  </span>
                )}
              </button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-80">
              <DropdownMenuLabel className="flex items-center justify-between">
                Notifications
                {supported && (
                  <div className="flex items-center gap-2">
                    <span className="text-xs font-normal text-muted-foreground">
                      Browser
                    </span>
                    <Switch
                      id="notif-toggle"
                      checked={enabled}
                      onCheckedChange={setEnabled}
                      disabled={permission === "denied"}
                      className="scale-75"
                    />
                  </div>
                )}
              </DropdownMenuLabel>
              {permission === "denied" && (
                <p className="px-2 pb-1 text-xs text-muted-foreground">
                  Browser notifications blocked. Allow in site settings.
                </p>
              )}
              <DropdownMenuSeparator />
              {notifications.length === 0 ? (
                <div className="px-2 py-6 text-center text-sm text-muted-foreground">
                  No notifications yet
                </div>
              ) : (
                <div className="max-h-64 overflow-y-auto">
                  {notifications.slice(0, 20).map((n) => (
                    <div
                      key={n.id}
                      className="flex flex-col gap-0.5 px-2 py-2 text-sm border-b last:border-0 border-border/50"
                    >
                      <div className="flex items-start justify-between gap-2">
                        <span className="font-medium leading-tight">
                          {n.title}
                        </span>
                        <span className="text-[11px] text-muted-foreground whitespace-nowrap tabular-nums">
                          {relativeTime(
                            new Date(n.timestamp_ms).toISOString()
                          )}
                        </span>
                      </div>
                      {n.details.length > 0 && (
                        <span className="text-xs text-muted-foreground leading-snug truncate">
                          {n.details[0]}
                        </span>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </DropdownMenuContent>
          </DropdownMenu>

          {/* Theme toggle */}
          <button
            onClick={toggleTheme}
            className="flex size-8 items-center justify-center rounded-md text-[var(--header-foreground)]/60 hover:text-[var(--header-foreground)] hover:bg-white/[0.12] transition-colors"
            aria-label={
              theme === "dark"
                ? "Switch to light mode"
                : "Switch to dark mode"
            }
          >
            {theme === "dark" ? (
              <Sun className="size-4" />
            ) : (
              <Moon className="size-4" />
            )}
          </button>

          {/* User dropdown */}
          {user && (
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <button
                  className="flex size-7 items-center justify-center rounded-full bg-white/[0.15] text-[11px] font-semibold text-[var(--header-foreground)] hover:bg-white/[0.25] transition-colors"
                  aria-label="User menu"
                >
                  {initials}
                </button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-56">
                <DropdownMenuLabel className="font-normal">
                  <div className="flex flex-col gap-1">
                    <div className="flex items-center gap-2">
                      <p className="text-sm font-medium leading-none">
                        {user.email?.split("@")[0] ?? "User"}
                      </p>
                      {role && (
                        <span className={cn(
                          "px-1.5 py-0.5 rounded text-[10px] font-semibold uppercase leading-none",
                          role === "admin"
                            ? "bg-indigo-500/20 text-indigo-400"
                            : "bg-emerald-500/20 text-emerald-400"
                        )}>
                          {role}
                        </span>
                      )}
                    </div>
                    <p className="text-xs text-muted-foreground leading-none">
                      {user.email}
                    </p>
                  </div>
                </DropdownMenuLabel>
                <DropdownMenuSeparator />
                <DropdownMenuItem asChild className="cursor-pointer">
                  <Link href="/account">
                    <User className="size-4" />
                    Account
                  </Link>
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={() => signOut()}
                  className="cursor-pointer"
                >
                  <LogOut className="size-4" />
                  Sign out
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          )}

          {/* Mobile hamburger */}
          <button
            onClick={() => setMobileOpen(true)}
            className="flex md:hidden size-8 items-center justify-center rounded-md text-[var(--header-foreground)]/60 hover:text-[var(--header-foreground)] hover:bg-white/[0.12] transition-colors"
            aria-label="Open menu"
          >
            <Menu className="size-5" />
          </button>
        </div>
      </div>

      {/* Mobile sheet */}
      <Sheet open={mobileOpen} onOpenChange={setMobileOpen}>
        <SheetContent side="left" className="w-64 p-0">
          <SheetHeader className="border-b px-4 py-3">
            <SheetTitle className="flex items-center gap-2">
              <DarkReachLogo size={20} className="text-[#6366f1]" />
              darkreach
            </SheetTitle>
          </SheetHeader>
          <nav className="flex flex-col py-2">
            {navEntries.map((entry) => {
              if (isGroup(entry)) {
                return (
                  <div key={entry.title}>
                    <div className="px-4 pt-4 pb-1 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground/60">
                      {entry.title}
                    </div>
                    {entry.items.map((item) => {
                      const active = isActive(item.href);
                      return (
                        <SheetClose key={item.href} asChild>
                          <Link
                            href={item.href}
                            className={cn(
                              "flex items-center justify-between px-4 py-2.5 text-sm font-medium transition-colors",
                              active
                                ? "text-foreground bg-accent border-l-2 border-[#f78166]"
                                : "text-muted-foreground hover:text-foreground hover:bg-accent/50"
                            )}
                          >
                            {item.title}
                            {item.count != null && (
                              <span className="inline-flex items-center justify-center min-w-[20px] h-5 px-1.5 text-xs font-semibold rounded-full bg-muted text-muted-foreground">
                                {item.count}
                              </span>
                            )}
                          </Link>
                        </SheetClose>
                      );
                    })}
                  </div>
                );
              }

              const active = isActive(entry.href);
              return (
                <SheetClose key={entry.href} asChild>
                  <Link
                    href={entry.href}
                    className={cn(
                      "flex items-center justify-between px-4 py-2.5 text-sm font-medium transition-colors",
                      active
                        ? "text-foreground bg-accent border-l-2 border-[#f78166]"
                        : "text-muted-foreground hover:text-foreground hover:bg-accent/50"
                    )}
                  >
                    {entry.title}
                    {entry.count != null && (
                      <span className="inline-flex items-center justify-center min-w-[20px] h-5 px-1.5 text-xs font-semibold rounded-full bg-muted text-muted-foreground">
                        {entry.count}
                      </span>
                    )}
                  </Link>
                </SheetClose>
              );
            })}
          </nav>
          {user && (
            <div className="mt-auto border-t px-4 py-3">
              <div className="flex items-center gap-3">
                <div className="flex size-8 items-center justify-center rounded-full bg-muted text-xs font-semibold">
                  {initials}
                </div>
                <div className="flex flex-col min-w-0">
                  <span className="text-sm font-medium truncate">
                    {user.email?.split("@")[0] ?? "User"}
                  </span>
                  <span className="text-xs text-muted-foreground truncate">
                    {user.email}
                  </span>
                </div>
              </div>
              <button
                onClick={() => signOut()}
                className="mt-3 flex w-full items-center gap-2 rounded-md px-2 py-2 text-sm text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors"
              >
                <LogOut className="size-4" />
                Sign out
              </button>
            </div>
          )}
        </SheetContent>
      </Sheet>
    </header>
  );
}
