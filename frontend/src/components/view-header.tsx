/**
 * @module view-header
 *
 * Reusable page header component with title, optional subtitle,
 * metadata slot, and action buttons. Provides consistent layout
 * across all dashboard pages (Browse, Fleet, Searches, Agents, etc.).
 */

import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

interface ViewHeaderProps {
  title: string;
  subtitle?: string;
  metadata?: ReactNode;
  actions?: ReactNode;
  tabs?: ReactNode;
  className?: string;
}

export function ViewHeader({
  title,
  subtitle,
  metadata,
  actions,
  tabs,
  className,
}: ViewHeaderProps) {
  return (
    <div className={cn("mb-4", className)}>
      <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between pb-4">
        <div>
          <h1 className="text-2xl font-semibold text-foreground">{title}</h1>
          {subtitle && (
            <p className="text-sm text-muted-foreground mt-1">{subtitle}</p>
          )}
          {metadata && <div className="mt-3">{metadata}</div>}
        </div>
        {actions && (
          <div className="flex flex-wrap items-center gap-2">{actions}</div>
        )}
      </div>
      {tabs ? (
        <div className="border-b">{tabs}</div>
      ) : (
        <div className="border-b" />
      )}
    </div>
  );
}
