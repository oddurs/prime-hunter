export function GetStarted() {
  return (
    <section id="get-started" className="py-24 px-6">
      <div className="mx-auto max-w-3xl text-center">
        <h2 className="text-3xl font-bold text-text mb-4">Get Started</h2>
        <p className="text-text-muted mb-10">
          Install from source and start hunting primes in minutes.
        </p>

        <div className="text-left space-y-6">
          {/* Install */}
          <div>
            <h3 className="text-sm font-medium text-text-muted mb-2">
              Build from source
            </h3>
            <div className="code-block">
              <code className="text-text">
                <span className="text-text-muted"># Requires Rust and GMP</span>
                {"\n"}git clone https://github.com/darkreach/darkreach.git{"\n"}cd
                darkreach{"\n"}cargo build --release
              </code>
            </div>
          </div>

          {/* Quick example */}
          <div>
            <h3 className="text-sm font-medium text-text-muted mb-2">
              Run a search
            </h3>
            <div className="code-block">
              <code className="text-text">
                <span className="text-text-muted">
                  # Search for factorial primes from 1000! to 5000!
                </span>
                {"\n"}./target/release/darkreach factorial --start 1000 --end 5000
                {"\n"}
                {"\n"}
                <span className="text-text-muted">
                  # Search Proth primes k·2^n+1
                </span>
                {"\n"}./target/release/darkreach kbn --k 3 --base 2 --min-n 100000
                --max-n 200000
              </code>
            </div>
          </div>

          {/* Links */}
          <div className="flex flex-wrap items-center justify-center gap-4 pt-4">
            <a
              href="https://github.com/darkreach/darkreach"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center px-5 py-2.5 rounded-md bg-accent-purple text-white font-medium text-sm hover:opacity-90 transition-opacity"
            >
              GitHub Repository
            </a>
            <a
              href="https://github.com/darkreach/darkreach/wiki"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center px-5 py-2.5 rounded-md border border-border text-text-muted font-medium text-sm hover:text-text hover:border-text-muted transition-colors"
            >
              Documentation
            </a>
            <a
              href="#"
              className="inline-flex items-center px-5 py-2.5 rounded-md border border-border text-text-muted font-medium text-sm hover:text-text hover:border-text-muted transition-colors"
            >
              Live Dashboard
            </a>
          </div>
        </div>
      </div>
    </section>
  );
}
