"use client";

import { usePathname } from "next/navigation";
import Link from "next/link";
import { docsNav } from "@/lib/docs-nav";
import { cn } from "@/lib/cn";
import { Menu, X } from "lucide-react";
import { useState } from "react";

function SidebarContent({ onNavigate }: { onNavigate?: () => void }) {
  const pathname = usePathname();

  return (
    <nav className="space-y-6">
      {docsNav.map((section) => (
        <div key={section.title}>
          <h3 className="text-xs font-medium text-text-muted uppercase tracking-wider mb-2">
            {section.title}
          </h3>
          <ul className="space-y-1">
            {section.items.map((item) => {
              const active = pathname === item.href;
              return (
                <li key={item.href}>
                  <Link
                    href={item.href}
                    onClick={onNavigate}
                    className={cn(
                      "block py-1.5 px-3 text-sm rounded-md transition-colors",
                      active
                        ? "bg-accent-purple/10 text-accent-purple border-l-2 border-accent-purple"
                        : "text-text-muted hover:text-text hover:bg-bg-secondary"
                    )}
                  >
                    {item.title}
                  </Link>
                </li>
              );
            })}
          </ul>
        </div>
      ))}
    </nav>
  );
}

export function DocSidebar() {
  const [mobileOpen, setMobileOpen] = useState(false);

  return (
    <>
      {/* Mobile toggle */}
      <div className="lg:hidden mb-4">
        <button
          onClick={() => setMobileOpen(!mobileOpen)}
          className="flex items-center gap-2 text-sm text-text-muted hover:text-text transition-colors"
        >
          {mobileOpen ? <X size={16} /> : <Menu size={16} />}
          {mobileOpen ? "Close menu" : "Documentation menu"}
        </button>
        {mobileOpen && (
          <div className="mt-4 p-4 bg-bg-secondary border border-border rounded-md">
            <SidebarContent onNavigate={() => setMobileOpen(false)} />
          </div>
        )}
      </div>

      {/* Desktop sidebar */}
      <aside className="hidden lg:block w-56 shrink-0 sticky top-20 self-start">
        <SidebarContent />
      </aside>
    </>
  );
}
