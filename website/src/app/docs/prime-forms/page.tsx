"use client";

import { CodeBlock } from "@/components/ui/code-block";
import { Badge } from "@/components/ui/badge";

interface FormEntry {
  name: string;
  formula: string;
  oeis: string;
  oeisUrl: string;
  algorithm: string;
  description: string;
  command: string;
  records?: string;
}

const forms: FormEntry[] = [
  {
    name: "Factorial",
    formula: "n! ± 1",
    oeis: "A002981 / A002982",
    oeisUrl: "https://oeis.org/A002981",
    algorithm: "Pocklington / Morrison proof",
    description:
      "Primes adjacent to factorial numbers. n! grows super-exponentially, making these primes extremely rare at large n. Pocklington proofs use the known factorization of n! for N-1 certificates.",
    command:
      "darkreach factorial --start 1000 --end 5000",
    records: "Largest known: 208003! - 1 (1,015,843 digits)",
  },
  {
    name: "Primorial",
    formula: "p# ± 1",
    oeis: "A014545 / A005234",
    oeisUrl: "https://oeis.org/A014545",
    algorithm: "Pocklington / Morrison proof",
    description:
      "Primes adjacent to the product of all primes up to p. Similar structure to factorials but the complete factorization is trivially known, enabling efficient proofs.",
    command:
      "darkreach primorial --start 1000 --end 50000",
    records: "Largest known: 1648079# + 1 (715,021 digits)",
  },
  {
    name: "Proth / Riesel (k*b^n ± 1)",
    formula: "k·b^n ± 1",
    oeis: "A080076",
    oeisUrl: "https://oeis.org/A080076",
    algorithm: "Proth test / LLR test + BSGS sieve",
    description:
      "The workhorse form for large prime searches. Proth's theorem provides a simple deterministic test for k·2^n+1 when k < 2^n. The BSGS (baby-step giant-step) sieve efficiently eliminates composites.",
    command:
      "darkreach kbn --k 3 --base 2 --min-n 100000 --max-n 500000",
  },
  {
    name: "Cullen / Woodall",
    formula: "n·2^n ± 1",
    oeis: "A005849 / A002234",
    oeisUrl: "https://oeis.org/A005849",
    algorithm: "Proth test / LLR test",
    description:
      "Cullen numbers (n·2^n+1) and Woodall numbers (n·2^n-1). A special case of k·b^n±1 where k=n. Cullen primes are very rare — only 16 known.",
    command:
      "darkreach cullen-woodall --min-n 1000 --max-n 100000",
  },
  {
    name: "Generalized Fermat",
    formula: "b^(2^n) + 1",
    oeis: "A019434",
    oeisUrl: "https://oeis.org/A019434",
    algorithm: "Pepin test / Proth test",
    description:
      "Generalization of Fermat numbers F_n = 2^(2^n)+1 to arbitrary bases. Only 5 Fermat primes are known (F_0 through F_4). Generalized Fermat primes with large bases are more common.",
    command:
      "darkreach gen-fermat --min-base 2 --max-base 1000 --exponent 16",
  },
  {
    name: "Wagstaff",
    formula: "(2^p + 1) / 3",
    oeis: "A000978",
    oeisUrl: "https://oeis.org/A000978",
    algorithm: "Vrba-Reix PRP test",
    description:
      "Wagstaff numbers for prime p. No deterministic primality proof is known — all results are probable primes (PRP). The multiplicative-order sieve eliminates many composites efficiently.",
    command: "darkreach wagstaff --min-p 1000 --max-p 100000",
  },
  {
    name: "Carol / Kynea",
    formula: "(2^n ± 1)² − 2",
    oeis: "A091515 / A091516",
    oeisUrl: "https://oeis.org/A091515",
    algorithm: "LLR test",
    description:
      "Carol primes (2^n-1)²-2 and Kynea primes (2^n+1)²-2. These expand to 4^n - 2^(n+1) - 1 and 4^n + 2^(n+1) - 1 respectively, which are k·b^n-1 forms testable by LLR.",
    command:
      "darkreach carol-kynea --min-n 10 --max-n 100000",
  },
  {
    name: "Twin Primes",
    formula: "p, p + 2",
    oeis: "A001359",
    oeisUrl: "https://oeis.org/A001359",
    algorithm: "Quad sieve + Proth/LLR intersection",
    description:
      "Pairs of primes separated by exactly 2. The twin prime conjecture (infinitely many exist) remains unproven. darkreach searches for twin primes of the form k·2^n ± 1, requiring both to pass primality tests.",
    command:
      "darkreach twin --k 3 --base 2 --min-n 1000 --max-n 100000",
  },
  {
    name: "Sophie Germain",
    formula: "p, 2p + 1",
    oeis: "A005384",
    oeisUrl: "https://oeis.org/A005384",
    algorithm: "Proth + LLR intersection sieve",
    description:
      "A prime p is a Sophie Germain prime if 2p+1 is also prime (a safe prime). Safe primes are used in cryptography (Diffie-Hellman groups). Both p and 2p+1 must pass independent primality tests.",
    command:
      "darkreach sophie-germain --k 3 --base 2 --min-n 1000 --max-n 100000",
  },
  {
    name: "Palindromic",
    formula: "d₁d₂...d₂d₁",
    oeis: "A002385",
    oeisUrl: "https://oeis.org/A002385",
    algorithm: "Deep sieve + Miller-Rabin",
    description:
      "Primes that read the same forwards and backwards in a given base. Even-digit palindromes are always divisible by (base+1), so only odd-digit counts are searched. Batch generation with deep sieve filtering.",
    command:
      "darkreach palindromic --base 10 --min-digits 1 --max-digits 11",
  },
  {
    name: "Near-Repdigit",
    formula: "aaa...baa...a",
    oeis: "—",
    oeisUrl: "#",
    algorithm: "BLS N+1 proof",
    description:
      "Palindromic primes where all digits are the same except one. The structured form enables BLS (Brillhart-Lehmer-Selfridge) N+1 proofs, providing deterministic verification.",
    command:
      "darkreach near-repdigit --base 10 --min-digits 5 --max-digits 15",
  },
  {
    name: "Repunit",
    formula: "R(b,n) = (b^n − 1) / (b − 1)",
    oeis: "A004023",
    oeisUrl: "https://oeis.org/A004023",
    algorithm: "PFGW PRP",
    description:
      "Numbers consisting entirely of 1s in base b. Extremely rare — only 11 known decimal repunit primes. n must be prime for R(10,n) to possibly be prime. PFGW acceleration is essential for large candidates.",
    command: "darkreach repunit --base 10 --min-n 100 --max-n 100000",
  },
];

export default function PrimeFormsPage() {
  return (
    <div className="prose-docs">
      <h1>Prime Forms</h1>
      <p>
        darkreach searches for 12 special forms of prime numbers. Each form has
        a dedicated sieve, primality test, and (where possible) deterministic
        proof algorithm.
      </p>

      <div className="space-y-10 mt-8">
        {forms.map((form) => (
          <div
            key={form.name}
            className="border border-border rounded-lg p-6 bg-card"
          >
            <div className="flex flex-wrap items-center gap-3 mb-3">
              <h2 className="text-xl font-bold text-foreground m-0 p-0 border-0">
                {form.name}
              </h2>
              <Badge variant="purple">{form.algorithm}</Badge>
            </div>
            <div className="font-mono text-accent-purple text-lg mb-3">
              {form.formula}
            </div>
            <p className="mb-3">{form.description}</p>
            {form.records && (
              <p className="text-sm text-muted-foreground mb-3">{form.records}</p>
            )}
            <p className="text-sm mb-2">
              <strong>OEIS:</strong>{" "}
              {form.oeisUrl !== "#" ? (
                <a
                  href={form.oeisUrl}
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  {form.oeis}
                </a>
              ) : (
                <span className="text-muted-foreground">{form.oeis}</span>
              )}
            </p>
            <CodeBlock language="bash">{form.command}</CodeBlock>
          </div>
        ))}
      </div>
    </div>
  );
}
