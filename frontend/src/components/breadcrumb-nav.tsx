"use client";

import { usePathname } from "next/navigation";
import Link from "next/link";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";

const ROUTE_MAP: Record<string, { group?: string; label: string }> = {
  "/": { label: "Dashboard" },
  "/browse": { group: "Discovery", label: "Browse" },
  "/searches": { group: "Discovery", label: "Searches" },
  "/projects": { group: "Discovery", label: "Projects" },
  "/leaderboard": { group: "Discovery", label: "Leaderboard" },
  "/network": { group: "Operations", label: "Network" },
  "/performance": { group: "Operations", label: "Observability" },
  "/logs": { group: "Operations", label: "Logs" },
  "/releases": { group: "Operations", label: "Releases" },
  "/strategy": { group: "Operations", label: "Strategy" },
  "/agents": { group: "Intelligence", label: "Agents" },
  "/docs": { label: "Docs" },
  "/my-nodes": { label: "My Nodes" },
  "/account": { label: "Account" },
  "/prime": { label: "Prime Detail" },
};

export function BreadcrumbNav() {
  const pathname = usePathname();

  const match = ROUTE_MAP[pathname] ?? ROUTE_MAP["/" + pathname.split("/")[1]];
  if (!match) return null;

  return (
    <Breadcrumb>
      <BreadcrumbList>
        {match.group && (
          <>
            <BreadcrumbItem>
              <BreadcrumbLink asChild>
                <Link href="/" className="text-muted-foreground">
                  {match.group}
                </Link>
              </BreadcrumbLink>
            </BreadcrumbItem>
            <BreadcrumbSeparator />
          </>
        )}
        <BreadcrumbItem>
          <BreadcrumbPage>{match.label}</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>
  );
}
