const discoveries = [
  {
    form: "Factorial",
    expression: "147855! + 1",
    digits: "636,919",
    proof: "Pocklington",
    date: "2026-02-14",
  },
  {
    form: "Proth",
    expression: "87 · 2^1,290,473 + 1",
    digits: "388,342",
    proof: "Proth test",
    date: "2026-02-12",
  },
  {
    form: "Twin",
    expression: "3 · 2^850,121 ± 1",
    digits: "255,891",
    proof: "Proth + LLR",
    date: "2026-02-10",
  },
  {
    form: "Palindromic",
    expression: "1 [0]₃₇₅₁₂ 1",
    digits: "37,514",
    proof: "BPSW + MR₁₀",
    date: "2026-02-08",
  },
  {
    form: "Primorial",
    expression: "1648079# + 1",
    digits: "715,021",
    proof: "Morrison",
    date: "2026-02-05",
  },
  {
    form: "Gen. Fermat",
    expression: "142^65536 + 1",
    digits: "141,116",
    proof: "Pépin",
    date: "2026-01-29",
  },
  {
    form: "Sophie Germain",
    expression: "21 · 2^641,008 − 1",
    digits: "192,971",
    proof: "LLR",
    date: "2026-01-25",
  },
  {
    form: "Cullen",
    expression: "6,679,881 · 2^6,679,881 + 1",
    digits: "2,010,852",
    proof: "Proth test",
    date: "2026-01-18",
  },
  {
    form: "Repunit",
    expression: "R(10, 86,453)",
    digits: "86,453",
    proof: "PFGW PRP",
    date: "2026-01-11",
  },
  {
    form: "Wagstaff",
    expression: "(2^1,284,057 + 1) / 3",
    digits: "386,614",
    proof: "Vrba-Reix PRP",
    date: "2026-01-04",
  },
];

export function Discoveries() {
  return (
    <section id="discoveries" className="py-24 px-6">
      <div className="mx-auto max-w-6xl">
        <h2 className="text-3xl font-bold text-text mb-4 text-center">
          Recent Discoveries
        </h2>
        <p className="text-text-muted text-center max-w-2xl mx-auto mb-12">
          A sample of primes found by the Darkreach network.
        </p>

        <div className="overflow-x-auto rounded-lg border border-border">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-bg-secondary text-text-muted text-left">
                <th className="px-4 py-3 font-medium">Form</th>
                <th className="px-4 py-3 font-medium">Expression</th>
                <th className="px-4 py-3 font-medium text-right">Digits</th>
                <th className="px-4 py-3 font-medium">Proof</th>
                <th className="px-4 py-3 font-medium">Date</th>
              </tr>
            </thead>
            <tbody>
              {discoveries.map((d, i) => (
                <tr
                  key={i}
                  className="border-t border-border hover:bg-bg-secondary/50 transition-colors"
                >
                  <td className="px-4 py-3 text-text">{d.form}</td>
                  <td className="px-4 py-3 font-mono text-accent-purple">
                    {d.expression}
                  </td>
                  <td className="px-4 py-3 font-mono text-text text-right">
                    {d.digits}
                  </td>
                  <td className="px-4 py-3 text-text-muted">{d.proof}</td>
                  <td className="px-4 py-3 text-text-muted">{d.date}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </section>
  );
}
