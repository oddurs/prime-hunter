"use client";

import { useEffect, useState } from "react";
import { usePathname } from "next/navigation";
import { DarkReachLogo } from "./darkreach-logo";
import { MobileNav } from "./mobile-nav";
import { ChevronDown, Github, Menu } from "lucide-react";
import Link from "next/link";

const docsLinks = [
  { label: "Getting Started", href: "/docs/getting-started" },
  { label: "Architecture", href: "/docs/architecture" },
  { label: "Prime Forms", href: "/docs/prime-forms" },
  { label: "API Reference", href: "/docs/api" },
  { label: "Contributing", href: "/docs/contributing" },
];

const downloadLinks = [
  { label: "Download", href: "/download" },
  { label: "Coordinator Setup", href: "/download/server" },
  { label: "Worker Deployment", href: "/download/worker" },
];

function NavDropdown({
  label,
  links,
  active,
}: {
  label: string;
  links: { label: string; href: string }[];
  active: boolean;
}) {
  const [open, setOpen] = useState(false);

  return (
    <div
      className="relative"
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
    >
      <button
        className={`flex items-center gap-1 text-sm transition-colors ${
          active ? "text-text" : "text-text-muted hover:text-text"
        }`}
      >
        {label}
        <ChevronDown size={14} />
      </button>
      {active && (
        <span className="absolute bottom-0 left-0 right-0 h-0.5 bg-accent-orange" />
      )}
      {open && (
        <div className="absolute top-full left-0 pt-2 z-50">
          <div className="bg-bg-secondary border border-border rounded-md py-1 min-w-[180px] shadow-lg">
            {links.map((link) => (
              <Link
                key={link.href}
                href={link.href}
                className="block px-4 py-2 text-sm text-text-muted hover:text-text hover:bg-bg transition-colors"
              >
                {link.label}
              </Link>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

export function Navbar() {
  const [scrolled, setScrolled] = useState(false);
  const [mobileOpen, setMobileOpen] = useState(false);
  const pathname = usePathname();

  useEffect(() => {
    const handleScroll = () => setScrolled(window.scrollY > 20);
    window.addEventListener("scroll", handleScroll, { passive: true });
    return () => window.removeEventListener("scroll", handleScroll);
  }, []);

  const isActive = (path: string) => pathname === path;
  const isActivePrefix = (prefix: string) => pathname.startsWith(prefix);

  return (
    <>
      <nav
        className={`fixed top-0 left-0 right-0 z-50 transition-colors duration-200 ${
          scrolled
            ? "bg-bg/95 backdrop-blur-sm border-b border-border"
            : "bg-transparent"
        }`}
      >
        <div className="mx-auto max-w-7xl flex items-center justify-between px-6 sm:px-8 lg:px-12 h-16">
          <Link href="/" className="flex items-center gap-2">
            <DarkReachLogo size={28} />
            <span className="text-text font-semibold text-lg">darkreach</span>
          </Link>

          <div className="hidden md:flex items-center gap-8">
            <Link
              href="/about"
              className={`relative text-sm pb-0.5 transition-colors ${
                isActive("/about")
                  ? "text-text"
                  : "text-text-muted hover:text-text"
              }`}
            >
              About
              {isActive("/about") && (
                <span className="absolute bottom-0 left-0 right-0 h-0.5 bg-accent-orange" />
              )}
            </Link>

            <NavDropdown
              label="Docs"
              links={docsLinks}
              active={isActivePrefix("/docs")}
            />

            <NavDropdown
              label="Download"
              links={downloadLinks}
              active={isActivePrefix("/download")}
            />

            <Link
              href="/blog"
              className={`relative text-sm pb-0.5 transition-colors ${
                isActive("/blog")
                  ? "text-text"
                  : "text-text-muted hover:text-text"
              }`}
            >
              Blog
              {isActive("/blog") && (
                <span className="absolute bottom-0 left-0 right-0 h-0.5 bg-accent-orange" />
              )}
            </Link>

            <Link
              href="/status"
              className={`relative text-sm pb-0.5 transition-colors ${
                isActive("/status")
                  ? "text-text"
                  : "text-text-muted hover:text-text"
              }`}
            >
              Status
              {isActive("/status") && (
                <span className="absolute bottom-0 left-0 right-0 h-0.5 bg-accent-orange" />
              )}
            </Link>
          </div>

          <div className="flex items-center gap-4">
            <a
              href="https://github.com/darkreach/darkreach"
              target="_blank"
              rel="noopener noreferrer"
              className="text-text-muted hover:text-text transition-colors"
              aria-label="GitHub"
            >
              <Github size={20} />
            </a>
            <a
              href="https://app.darkreach.ai"
              className="hidden sm:inline-flex items-center px-4 py-1.5 rounded-md bg-accent-purple text-white text-sm font-medium hover:opacity-90 transition-opacity"
            >
              Open Dashboard
            </a>
            <button
              className="md:hidden text-text-muted hover:text-text transition-colors"
              onClick={() => setMobileOpen(true)}
              aria-label="Open menu"
            >
              <Menu size={24} />
            </button>
          </div>
        </div>
      </nav>

      <MobileNav open={mobileOpen} onClose={() => setMobileOpen(false)} />
    </>
  );
}
