import { DarkReachLogo } from "./darkreach-logo";
import Link from "next/link";

const columns = [
  {
    title: "Product",
    links: [
      { label: "Dashboard", href: "https://app.darkreach.ai", external: true },
      { label: "Download", href: "/download" },
      { label: "Status", href: "/status" },
      { label: "Leaderboard", href: "/leaderboard" },
    ],
  },
  {
    title: "Documentation",
    links: [
      { label: "Getting Started", href: "/docs/getting-started" },
      { label: "Architecture", href: "/docs/architecture" },
      { label: "Prime Forms", href: "/docs/prime-forms" },
      { label: "API Reference", href: "/docs/api" },
    ],
  },
  {
    title: "Community",
    links: [
      {
        label: "GitHub",
        href: "https://github.com/darkreach/darkreach",
        external: true,
      },
      { label: "Discord", href: "#" },
      { label: "Leaderboard", href: "/leaderboard" },
      { label: "Contributing", href: "/docs/contributing" },
    ],
  },
  {
    title: "Legal",
    links: [
      {
        label: "MIT License",
        href: "https://github.com/darkreach/darkreach/blob/master/LICENSE",
        external: true,
      },
    ],
  },
];

export function Footer() {
  return (
    <footer className="border-t border-border">
      <div className="mx-auto max-w-7xl px-6 sm:px-8 lg:px-12 py-12">
        <div className="grid grid-cols-2 md:grid-cols-5 gap-8">
          <div className="col-span-2 md:col-span-1">
            <div className="flex items-center gap-2 mb-4">
              <DarkReachLogo size={20} />
              <span className="text-foreground font-semibold">darkreach</span>
            </div>
            <p className="text-sm text-muted-foreground leading-relaxed">
              AI-driven distributed computing for scientific discovery.
            </p>
          </div>

          {columns.map((col) => (
            <div key={col.title}>
              <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wider mb-3">
                {col.title}
              </h3>
              <ul className="space-y-2">
                {col.links.map((link) => (
                  <li key={link.label}>
                    {"external" in link && link.external ? (
                      <a
                        href={link.href}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="text-sm text-muted-foreground hover:text-foreground transition-colors"
                      >
                        {link.label}
                      </a>
                    ) : (
                      <Link
                        href={link.href}
                        className="text-sm text-muted-foreground hover:text-foreground transition-colors"
                      >
                        {link.label}
                      </Link>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>

        <div className="mt-12 pt-8 border-t border-border flex flex-col sm:flex-row items-center justify-between gap-4">
          <span className="text-muted-foreground text-sm">
            &copy; {new Date().getFullYear()} darkreach. Open source under MIT.
          </span>
          <a
            href="https://github.com/darkreach/darkreach"
            target="_blank"
            rel="noopener noreferrer"
            className="text-sm text-muted-foreground hover:text-foreground transition-colors"
          >
            github.com/darkreach/darkreach
          </a>
        </div>
      </div>
    </footer>
  );
}
