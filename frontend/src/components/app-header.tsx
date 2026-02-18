"use client";

import { useState } from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { Menu, Moon, Sun } from "lucide-react";
import { useWs } from "@/contexts/websocket-context";
import { useTheme } from "@/hooks/use-theme";
import { cn } from "@/lib/utils";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetClose,
} from "@/components/ui/sheet";

export function AppHeader() {
  const pathname = usePathname();
  const { connected, searches } = useWs();
  const { theme, toggleTheme } = useTheme();
  const [mobileOpen, setMobileOpen] = useState(false);

  const runningCount = searches.filter((s) => s.status === "running").length;

  const navItems = [
    { title: "Dashboard", href: "/", count: undefined },
    { title: "Searches", href: "/searches", count: runningCount || undefined },
    { title: "Browse", href: "/browse", count: undefined },
    { title: "Docs", href: "/docs", count: undefined },
  ];

  function isActive(href: string) {
    return href === "/" ? pathname === "/" : pathname.startsWith(href);
  }

  return (
    <header className="sticky top-0 z-50 flex h-14 items-center border-b px-6">
      <div className="mx-auto flex w-full max-w-6xl items-center gap-6">
        {/* Logo */}
        <Link href="/" className="flex items-center gap-2 flex-shrink-0">
          <div className="flex size-7 items-center justify-center rounded-md bg-[#f78166] text-white text-sm font-bold">
            &Sigma;
          </div>
          <span className="font-semibold text-[var(--header-foreground)] tracking-tight hidden sm:inline">
            sigma
          </span>
        </Link>

        {/* Desktop nav */}
        <nav className="hidden md:flex items-center gap-1 overflow-x-auto">
          {navItems.map((item) => {
            const active = isActive(item.href);
            return (
              <Link
                key={item.href}
                href={item.href}
                className={cn(
                  "relative flex items-center gap-1.5 px-3 py-1 text-sm font-medium transition-colors rounded-md",
                  active
                    ? "text-white"
                    : "text-[var(--header-foreground)]/70 hover:text-[var(--header-foreground)] hover:bg-white/[0.12]"
                )}
              >
                {item.title}
                {item.count != null && (
                  <span className="inline-flex items-center justify-center min-w-[18px] h-[18px] px-1 text-[11px] font-semibold leading-none rounded-full bg-white/[0.15] text-[var(--header-foreground)]">
                    {item.count}
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
              <div className="flex size-6 items-center justify-center rounded-md bg-[#f78166] text-white text-xs font-bold">
                &Sigma;
              </div>
              sigma
            </SheetTitle>
          </SheetHeader>
          <nav className="flex flex-col py-2">
            {navItems.map((item) => {
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
          </nav>
        </SheetContent>
      </Sheet>
    </header>
  );
}
