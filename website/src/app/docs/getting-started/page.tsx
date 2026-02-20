"use client";

import { CodeBlock } from "@/components/ui/code-block";

export default function GettingStartedPage() {
  return (
    <div className="prose-docs">
      <h1>Getting Started</h1>
      <p>
        This guide walks you through installing darkreach, running your first
        prime search, and viewing results.
      </p>

      <h2>Prerequisites</h2>
      <ul>
        <li>
          <strong>Rust</strong> 1.75 or later —{" "}
          <a href="https://rustup.rs">rustup.rs</a>
        </li>
        <li>
          <strong>GMP</strong> (GNU Multiple Precision Arithmetic Library)
        </li>
      </ul>

      <h3>macOS</h3>
      <CodeBlock language="bash">{"brew install gmp"}</CodeBlock>

      <h3>Linux (Debian/Ubuntu)</h3>
      <CodeBlock language="bash">
        {"sudo apt install build-essential libgmp-dev m4"}
      </CodeBlock>

      <h2>Build</h2>
      <CodeBlock language="bash">
        {`git clone https://github.com/darkreach/darkreach.git
cd darkreach
cargo build --release`}
      </CodeBlock>
      <p>
        The binary will be at <code>./target/release/darkreach</code>.
      </p>

      <h2>Run Your First Search</h2>
      <p>
        Try a quick factorial prime search to verify everything works:
      </p>
      <CodeBlock language="bash">
        {"./target/release/darkreach factorial --start 1 --end 100"}
      </CodeBlock>
      <p>
        You should see output on stderr as candidates are tested, with any
        primes found logged to the console.
      </p>

      <h3>More search examples</h3>
      <CodeBlock language="bash">
        {`# Proth primes k·2^n+1
./target/release/darkreach kbn --k 3 --base 2 --min-n 1 --max-n 1000

# Palindromic primes in base 10
./target/release/darkreach palindromic --base 10 --min-digits 1 --max-digits 9

# Twin primes
./target/release/darkreach twin --k 3 --base 2 --min-n 1 --max-n 10000`}
      </CodeBlock>

      <h2>View Results</h2>
      <p>
        By default, results are printed to stderr. To persist discoveries to a
        database, provide a PostgreSQL connection string:
      </p>
      <CodeBlock language="bash">
        {`export DATABASE_URL="postgres://user:pass@localhost/darkreach"
./target/release/darkreach factorial --start 1000 --end 5000`}
      </CodeBlock>
      <p>
        Results will be stored in the <code>primes</code> table and visible in
        the{" "}
        <a href="https://app.darkreach.ai">dashboard</a>.
      </p>

      <h2>Checkpointing</h2>
      <p>
        Searches automatically checkpoint progress every 60 seconds. If a search
        is interrupted, it resumes from the last checkpoint:
      </p>
      <CodeBlock language="bash">
        {`# Checkpoint is saved to darkreach.checkpoint by default
# Use --checkpoint to specify a custom path
./target/release/darkreach --checkpoint my-search.checkpoint \\
  kbn --k 3 --base 2 --min-n 100000 --max-n 500000`}
      </CodeBlock>

      <h2>Next Steps</h2>
      <ul>
        <li>
          Learn about the{" "}
          <a href="/docs/architecture">system architecture</a>
        </li>
        <li>
          Explore all{" "}
          <a href="/docs/prime-forms">12 prime forms</a>
        </li>
        <li>
          Deploy a{" "}
          <a href="/download/server">coordinator</a> or{" "}
          <a href="/download/worker">worker</a>
        </li>
        <li>
          <a href="/docs/contributing">Contribute</a> to the project
        </li>
      </ul>
    </div>
  );
}
