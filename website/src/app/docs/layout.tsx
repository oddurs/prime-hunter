import { DocSidebar } from "@/components/doc-sidebar";
import type { Metadata } from "next";

export const metadata: Metadata = {
  title: {
    default: "Documentation",
    template: "%s — darkreach docs",
  },
};

export default function DocsLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <div className="mx-auto max-w-7xl px-6 sm:px-8 lg:px-12 py-12">
      <div className="lg:flex lg:gap-12">
        <DocSidebar />
        <div className="flex-1 min-w-0">{children}</div>
      </div>
    </div>
  );
}
