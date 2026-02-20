import { Card } from "@/components/ui/card";
import { BookOpen, Cpu, Layers, Code2, GitPullRequest, Workflow } from "lucide-react";
import Link from "next/link";

const quickLinks = [
  {
    icon: BookOpen,
    title: "Getting Started",
    description: "Install, build, and run your first prime search in minutes.",
    href: "/docs/getting-started",
  },
  {
    icon: Layers,
    title: "Architecture",
    description: "System diagram, engine/server/frontend breakdown, and data flow.",
    href: "/docs/architecture",
  },
  {
    icon: Cpu,
    title: "Prime Forms",
    description: "All 12 forms with formulas, OEIS references, and CLI commands.",
    href: "/docs/prime-forms",
  },
  {
    icon: Code2,
    title: "API Reference",
    description: "REST endpoints and WebSocket protocol for the coordinator.",
    href: "/docs/api",
  },
  {
    icon: GitPullRequest,
    title: "Contributing",
    description: "Fork/PR workflow, code style, testing, and adding new forms.",
    href: "/docs/contributing",
  },
  {
    icon: Workflow,
    title: "Download",
    description: "Install methods, coordinator setup, and worker deployment.",
    href: "/download",
  },
];

export default function DocsPage() {
  return (
    <div className="prose-docs">
      <h1>darkreach Documentation</h1>
      <p>
        darkreach is an AI-driven distributed computing platform for hunting
        special-form prime numbers. It combines high-performance number theory
        algorithms with autonomous AI agents to push the boundaries of
        mathematical discovery.
      </p>

      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 mt-8 not-prose">
        {quickLinks.map((link) => (
          <Link key={link.href} href={link.href}>
            <Card hover className="h-full group cursor-pointer">
              <div className="flex items-center gap-3 mb-2">
                <link.icon size={18} className="text-accent-purple" />
                <h3 className="text-text font-semibold group-hover:text-accent-purple transition-colors">
                  {link.title}
                </h3>
              </div>
              <p className="text-sm text-text-muted">{link.description}</p>
            </Card>
          </Link>
        ))}
      </div>
    </div>
  );
}
