"use client";

import { RoleGuard } from "@/contexts/auth-context";

export default function StrategyLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return <RoleGuard>{children}</RoleGuard>;
}
