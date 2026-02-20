"use client";

import { useEffect } from "react";
import { X, Github } from "lucide-react";
import Link from "next/link";
import { DarkReachLogo } from "./darkreach-logo";

const navSections = [
  {
    title: "Product",
    links: [
      { label: "About", href: "/about" },
      { label: "Download", href: "/download" },
      { label: "Status", href: "/status" },
      { label: "Leaderboard", href: "/leaderboard" },
      { label: "Blog", href: "/blog" },
    ],
  },
  {
    title: "Documentation",
    links: [
      { label: "Getting Started", href: "/docs/getting-started" },
      { label: "Architecture", href: "/docs/architecture" },
      { label: "Prime Forms", href: "/docs/prime-forms" },
      { label: "API Reference", href: "/docs/api" },
      { label: "Contributing", href: "/docs/contributing" },
    ],
  },
  {
    title: "Deploy",
    links: [
      { label: "Coordinator Setup", href: "/download/server" },
      { label: "Worker Deployment", href: "/download/worker" },
    ],
  },
];

interface MobileNavProps {
  open: boolean;
  onClose: () => void;
}

export function MobileNav({ open, onClose }: MobileNavProps) {
  useEffect(() => {
    if (open) {
      document.body.style.overflow = "hidden";
    } else {
      document.body.style.overflow = "";
    }
    return () => {
      document.body.style.overflow = "";
    };
  }, [open]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 md:hidden">
      <div className="absolute inset-0 bg-black/60" onClick={onClose} />

      <div className="absolute inset-y-0 right-0 w-full max-w-sm bg-background border-l border-border overflow-y-auto">
        <div className="flex items-center justify-between px-6 h-16 border-b border-border">
          <div className="flex items-center gap-2">
            <DarkReachLogo size={24} />
            <span className="text-foreground font-semibold">darkreach</span>
          </div>
          <button
            onClick={onClose}
            className="text-muted-foreground hover:text-foreground transition-colors"
            aria-label="Close menu"
          >
            <X size={24} />
          </button>
        </div>

        <div className="px-6 py-6 space-y-8">
          {navSections.map((section) => (
            <div key={section.title}>
              <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wider mb-3">
                {section.title}
              </h3>
              <div className="space-y-1">
                {section.links.map((link) => (
                  <Link
                    key={link.href}
                    href={link.href}
                    onClick={onClose}
                    className="block py-2 text-sm text-foreground hover:text-accent-purple transition-colors"
                  >
                    {link.label}
                  </Link>
                ))}
              </div>
            </div>
          ))}

          <div className="pt-4 border-t border-border space-y-3">
            <a
              href="https://github.com/darkreach/darkreach"
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center gap-2 py-2 text-sm text-muted-foreground hover:text-foreground transition-colors"
            >
              <Github size={16} />
              GitHub
            </a>
            <a
              href="https://app.darkreach.ai"
              className="block w-full text-center px-4 py-2.5 rounded-md bg-accent-purple text-white text-sm font-medium hover:opacity-90 transition-opacity"
            >
              Open Dashboard
            </a>
          </div>
        </div>
      </div>
    </div>
  );
}
