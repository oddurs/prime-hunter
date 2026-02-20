export interface BlogPost {
  slug: string;
  title: string;
  excerpt: string;
  date: string;
  author: string;
  tags: string[];
}

export const blogPosts: BlogPost[] = [
  {
    slug: "announcing-darkreach",
    title: "Announcing darkreach: AI-Driven Distributed Computing",
    excerpt:
      "We are launching darkreach — an open-source platform that combines autonomous AI agents with high-performance number theory algorithms to push the boundaries of mathematical discovery.",
    date: "2026-02-20",
    author: "darkreach team",
    tags: ["announcement", "launch"],
  },
  {
    slug: "ai-agents-prime-hunting",
    title: "How AI Agents Optimize Prime Hunting Strategies",
    excerpt:
      "A deep dive into how darkreach's autonomous agents research strategies, tune sieve parameters, and select optimal algorithms for each prime form — without human intervention.",
    date: "2026-02-15",
    author: "darkreach team",
    tags: ["ai", "agents", "engineering"],
  },
  {
    slug: "12-prime-forms",
    title: "A Tour of 12 Special Prime Forms",
    excerpt:
      "From n!±1 to generalized Fermats, each prime form has unique mathematical properties that require specialized sieves and tests. Here is how we approach each one.",
    date: "2026-02-08",
    author: "darkreach team",
    tags: ["mathematics", "primes"],
  },
  {
    slug: "fleet-architecture",
    title: "Building a Distributed Prime Search Fleet",
    excerpt:
      "How we built a PostgreSQL-based work distribution system with row-level locking, fault-tolerant checkpointing, and real-time coordination for dozens of workers.",
    date: "2026-01-28",
    author: "darkreach team",
    tags: ["infrastructure", "distributed-systems"],
  },
  {
    slug: "primality-certificates",
    title: "Deterministic Primality Certificates: Trust, but Verify",
    excerpt:
      "Why probabilistic primality tests are not enough for mathematical discoveries, and how Pocklington, Morrison, and BLS certificates provide independently verifiable proofs.",
    date: "2026-01-15",
    author: "darkreach team",
    tags: ["mathematics", "cryptography"],
  },
];
