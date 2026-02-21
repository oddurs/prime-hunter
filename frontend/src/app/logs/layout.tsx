"use client";

import { RoleGuard } from "@/contexts/auth-context";

export default function AdminLayout({ children }: { children: React.ReactNode }) {
  return <RoleGuard>{children}</RoleGuard>;
}
