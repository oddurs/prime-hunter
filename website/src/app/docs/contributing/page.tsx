"use client";

import { CodeBlock } from "@/components/ui/code-block";

export default function ContributingPage() {
  return (
    <div className="prose-docs">
      <h1>Contributing</h1>
      <p>
        darkreach is open source under the MIT license. Contributions are
        welcome — whether it is a bug fix, new prime form, performance
        improvement, or documentation update.
      </p>

      <h2>Development Setup</h2>
      <CodeBlock language="bash">
        {`# Fork and clone
git clone https://github.com/YOUR_USERNAME/darkreach.git
cd darkreach

# Install dependencies
# macOS: brew install gmp
# Linux: sudo apt install build-essential libgmp-dev m4

# Build and test
cargo build
cargo test`}
      </CodeBlock>

      <h2>Workflow</h2>
      <ol>
        <li>Fork the repository on GitHub</li>
        <li>
          Create a feature branch: <code>git checkout -b my-feature</code>
        </li>
        <li>Make your changes with tests</li>
        <li>
          Run the full test suite: <code>cargo test</code>
        </li>
        <li>
          Run clippy: <code>cargo clippy -- -D warnings</code>
        </li>
        <li>
          Format: <code>cargo fmt</code>
        </li>
        <li>Open a pull request against <code>master</code></li>
      </ol>

      <h2>Code Style</h2>
      <ul>
        <li>
          <strong>Rust</strong>: Follow <code>rustfmt</code> defaults. No{" "}
          <code>unsafe</code> in the main crate (except the macOS QoS syscall).
        </li>
        <li>
          <strong>Comments</strong>: This codebase is a teaching tool for
          computational number theory. Document algorithms at an academic level
          — cite theorems, link OEIS sequences, reference papers.
        </li>
        <li>
          <strong>Engine files</strong>: ~30-40% comments. Server: ~20-30%.
          Frontend: ~15-25%.
        </li>
        <li>
          All output goes to stderr (<code>eprintln!</code>). Results are logged
          to PostgreSQL.
        </li>
      </ul>

      <h2>Testing</h2>
      <CodeBlock language="bash">
        {`# Run all tests
cargo test

# Run tests for a specific module
cargo test factorial
cargo test kbn
cargo test palindromic

# Run with small ranges to verify quickly
cargo run -- factorial --start 1 --end 100
cargo run -- kbn --k 3 --base 2 --min-n 1 --max-n 1000
cargo run -- palindromic --base 10 --min-digits 1 --max-digits 9`}
      </CodeBlock>

      <h2>Adding a New Prime Form</h2>
      <p>
        To add a new search form (e.g., <code>mega-primes</code>):
      </p>
      <ol>
        <li>
          Create <code>src/mega_primes.rs</code> with the search function
          following the sieve → test → prove → report pattern
        </li>
        <li>
          Add the module to <code>src/lib.rs</code>
        </li>
        <li>
          Add a CLI subcommand in <code>src/main.rs</code>
        </li>
        <li>
          Add a checkpoint variant in <code>src/checkpoint.rs</code>
        </li>
        <li>
          Add search manager support in <code>src/search_manager.rs</code>
        </li>
        <li>
          Add deploy support in <code>src/deploy.rs</code>
        </li>
        <li>
          Add the form to <code>website/src/lib/prime-forms.ts</code>
        </li>
        <li>Write tests covering known primes and edge cases</li>
      </ol>

      <h2>Project Structure</h2>
      <CodeBlock>
        {`src/
├── main.rs           # CLI routing (clap)
├── lib.rs            # Module re-exports, utilities
├── factorial.rs      # n! ± 1 search
├── palindromic.rs    # Palindromic prime search
├── kbn.rs            # k·b^n ± 1 search
├── ... (9 more form modules)
├── sieve.rs          # Sieve algorithms
├── proof.rs          # Deterministic proofs
├── certificate.rs    # Primality certificates
├── dashboard.rs      # Axum web server
├── db/               # PostgreSQL queries
├── checkpoint.rs     # Search checkpointing
├── search_manager.rs # Work distribution
└── worker_client.rs  # Worker-coordinator HTTP`}
      </CodeBlock>

      <h2>Questions?</h2>
      <p>
        Open an issue on{" "}
        <a
          href="https://github.com/darkreach/darkreach/issues"
          target="_blank"
          rel="noopener noreferrer"
        >
          GitHub
        </a>{" "}
        or start a discussion. We are happy to help with onboarding.
      </p>
    </div>
  );
}
