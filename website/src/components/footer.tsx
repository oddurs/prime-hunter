import { DarkReachLogo } from "./darkreach-logo";

export function Footer() {
  return (
    <footer className="border-t border-border py-12 px-6">
      <div className="mx-auto max-w-6xl flex flex-col sm:flex-row items-center justify-between gap-4">
        <div className="flex items-center gap-2">
          <DarkReachLogo size={20} />
          <span className="text-text-muted text-sm">
            &copy; {new Date().getFullYear()} Darkreach. Open source under MIT.
          </span>
        </div>

        <div className="flex items-center gap-6 text-sm text-text-muted">
          <a
            href="https://github.com/darkreach/darkreach"
            target="_blank"
            rel="noopener noreferrer"
            className="hover:text-text transition-colors"
          >
            GitHub
          </a>
          <a
            href="https://github.com/darkreach/darkreach/wiki"
            target="_blank"
            rel="noopener noreferrer"
            className="hover:text-text transition-colors"
          >
            Docs
          </a>
          <a href="#" className="hover:text-text transition-colors">
            Dashboard
          </a>
        </div>
      </div>
    </footer>
  );
}
