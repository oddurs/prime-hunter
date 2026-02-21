"use client";

import { useState, useRef, useCallback, useEffect } from "react";
import { Bell, BellOff } from "lucide-react";
import { useWs } from "@/contexts/websocket-context";
import { useBrowserNotifications } from "@/hooks/use-notifications";
import { cn } from "@/lib/utils";
import { API_BASE, relativeTime } from "@/lib/format";
import { BreadcrumbNav } from "@/components/breadcrumb-nav";
import { Separator } from "@/components/ui/separator";
import { SidebarTrigger } from "@/components/ui/sidebar";
import { Switch } from "@/components/ui/switch";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

export function TopBar() {
  const { connected, notifications } = useWs();
  const { supported, permission, enabled, setEnabled } =
    useBrowserNotifications();

  // Notification unread tracking
  const lastSeenRef = useRef<number>(0);
  const [unreadCount, setUnreadCount] = useState(0);

  useEffect(() => {
    const count = notifications.filter(
      (n) => n.timestamp_ms > lastSeenRef.current
    ).length;
    setUnreadCount(count);
  }, [notifications]);

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

  // Error budget status
  const errorBudgetErrorsPerHour =
    Number(process.env.NEXT_PUBLIC_ERROR_BUDGET_ERRORS_PER_HOUR) || 10;
  const errorBudgetWarningsPerHour =
    Number(process.env.NEXT_PUBLIC_ERROR_BUDGET_WARNINGS_PER_HOUR) || 50;
  const [budgetStatus, setBudgetStatus] = useState<
    "Healthy" | "Risk" | "Breached"
  >("Healthy");

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
        const res = await fetch(
          `${API_BASE}/api/observability/report?${params}`
        );
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
          byLevel.find(
            ([level]) => level === "warning" || level === "warn"
          )?.[1] ?? 0;
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

  return (
    <header
      data-slot="top-bar"
      className="sticky top-0 z-40 flex h-12 items-center gap-3 border-b px-4"
    >
      <SidebarTrigger className="-ml-1" />
      <Separator orientation="vertical" className="mr-1 h-4" />
      <BreadcrumbNav />

      <div className="flex-1" />

      {/* Connection status */}
      <div
        className={cn(
          "size-2 rounded-full shrink-0",
          connected ? "bg-green-500" : "bg-red-500"
        )}
        title={connected ? "Connected" : "Disconnected"}
      />

      {/* Error budget badge */}
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

      {/* Notification bell */}
      <DropdownMenu onOpenChange={handleNotifOpenChange}>
        <DropdownMenuTrigger asChild>
          <button
            className="relative flex size-8 items-center justify-center rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
            aria-label="Notifications"
          >
            {enabled ? <Bell className="size-4" /> : <BellOff className="size-4" />}
            {unreadCount > 0 && (
              <span className="absolute -top-0.5 -right-0.5 flex size-4 items-center justify-center rounded-full bg-[#6366f1] text-[10px] font-bold text-white leading-none">
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

    </header>
  );
}
