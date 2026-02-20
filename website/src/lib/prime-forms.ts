export interface PrimeForm {
  name: string;
  formula: string;
  description: string;
  algorithm: string;
}

export const primeForms: PrimeForm[] = [
  {
    name: "Factorial",
    formula: "n! ± 1",
    description:
      "Primes adjacent to factorial numbers. GMP factorial computation with modular sieve elimination.",
    algorithm: "Pocklington / Morrison",
  },
  {
    name: "Primorial",
    formula: "p# ± 1",
    description:
      "Primes adjacent to the product of all primes up to p. Similar structure to factorials but denser.",
    algorithm: "Pocklington / Morrison",
  },
  {
    name: "Proth / Riesel",
    formula: "k·bⁿ ± 1",
    description:
      "The workhorse form. Covers Proth numbers (k·2ⁿ+1) and Riesel numbers (k·2ⁿ−1) with BSGS sieve.",
    algorithm: "Proth test / LLR",
  },
  {
    name: "Cullen / Woodall",
    formula: "n·2ⁿ ± 1",
    description:
      "Cullen numbers (n·2ⁿ+1) and Woodall numbers (n·2ⁿ−1). Special case of k·bⁿ±1 with k=n.",
    algorithm: "Proth test / LLR",
  },
  {
    name: "Generalized Fermat",
    formula: "b^(2ⁿ) + 1",
    description:
      "Generalization of Fermat numbers F_n = 2^(2ⁿ)+1 to arbitrary bases. Pépin-style testing.",
    algorithm: "Pépin / Proth",
  },
  {
    name: "Wagstaff",
    formula: "(2ᵖ + 1) / 3",
    description:
      "Wagstaff numbers for prime p. No deterministic proof exists — results are probable primes (PRP).",
    algorithm: "Vrba-Reix PRP",
  },
  {
    name: "Carol / Kynea",
    formula: "(2ⁿ ± 1)² − 2",
    description:
      "Carol primes (2ⁿ−1)²−2 and Kynea primes (2ⁿ+1)²−2. Sparse but fast to test.",
    algorithm: "LLR test",
  },
  {
    name: "Twin Primes",
    formula: "p, p + 2",
    description:
      "Pairs of primes separated by 2. Quad sieve eliminates candidates, then Proth+LLR intersection.",
    algorithm: "Proth + LLR",
  },
  {
    name: "Sophie Germain",
    formula: "p, 2p + 1",
    description:
      "Prime p where 2p+1 is also prime. Foundation for safe primes used in cryptography.",
    algorithm: "Proth + LLR",
  },
  {
    name: "Palindromic",
    formula: "d₁d₂...d₂d₁",
    description:
      "Primes that read the same forwards and backwards in a given base. Deep sieve with batch generation.",
    algorithm: "Miller-Rabin",
  },
  {
    name: "Near-Repdigit",
    formula: "aaa...baa...a",
    description:
      "Palindromic primes where all digits are the same except one. BLS N+1 proofs available.",
    algorithm: "BLS proof",
  },
  {
    name: "Repunit",
    formula: "(bⁿ − 1) / (b − 1)",
    description:
      "Numbers consisting entirely of 1s in base b. Extremely rare primes — only 11 known decimal repunit primes.",
    algorithm: "PFGW PRP",
  },
];
