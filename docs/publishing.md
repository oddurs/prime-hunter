# Publishing & Verification

How to go from discovery to recognition: t5k.org submission, OEIS contribution, verification standards, and academic publication.

---

## Discovery Pipeline

```
Day 0:      Discovery -- record expression, digit count, timestamp, hardware, version
            DO NOT ANNOUNCE PUBLICLY
Day 0-7:    Cross-verify with different tool (LLR/PFGW/PRST) on different hardware
Day 7-14:   Submit to t5k.org, seek community verification on Mersenne Forum
Month 1:    Update OEIS sequences, post arXiv preprint if novel
Month 3-6:  Submit journal paper (JIS for discovery, Math.Comp for methods)
```

---

## t5k.org Submission

1. **Create prover account** at [t5k.org/bios/submission.php](https://t5k.org/bios/submission.php)
2. **Establish proof code** -- short alphanumeric string documenting software/people/project
3. **Submit the prime** -- formula (under 255 chars) or full decimal expansion
4. **Verification queue** -- system performs trial division + PRP check (~51 primes in queue typically)
5. **Only PROVEN primes accepted.** PRPs are rejected.

51 archivable form categories including: Factorial (#9), Palindrome (#39), Primorial (#41), Wagstaff (#50), Twin (#48), Sophie Germain (#46).

---

## Verification Standards

Independent verification requires:
- **Different software** from discovery program
- **Different hardware architecture** (spanning Intel/AMD/ARM/GPU)
- **Different algorithm** where possible

### Cross-verification tools by form

| Form | Verification Software |
|---|---|
| k*b^n+1 (Proth) | LLR, PFGW, PRST |
| k*b^n-1 (Riesel) | LLR, PFGW, PRST |
| n!+/-1 | PFGW, Primo (small), PRST |
| Palindromic | PFGW, Primo (up to ~40K digits) |
| Wagstaff PRP | PFGW, custom Vrba-Reix |

---

## OEIS Contribution

1. Register at oeis.org (new accounts limited to 3 pending submissions)
2. Submit via [oeis.org/Submit.html](https://oeis.org/Submit.html) -- minimum 4 terms
3. Four-stage review: Proposal -> Review -> Approval -> Live
4. Software citation: `(Other) # Using darkreach (Rust/GMP)`

Key sequences:
- [A002981](https://oeis.org/A002981): n where n!+1 is prime
- [A002982](https://oeis.org/A002982): n where n!-1 is prime
- [A002385](https://oeis.org/A002385): Palindromic primes (base 10)

---

## Mersenne Forum Announcement

1. **Verify first, announce second.** Independent confirmation before public announcement.
2. Correct subforum: "And now for something completely different" (factorial/palindromic), "Conjectures 'R Us" (kbn), "Wagstaff PRP Search" (Wagstaff)
3. Include: expression, digit count, software, verification status, t5k.org link
4. Subject format: `[New record] 632760!-1 is prime (3,395,992 digits)`

---

## Academic Venues

| Journal | Best for |
|---|---|
| **Mathematics of Computation** | Algorithmic innovation + computation |
| **Journal of Integer Sequences** | Discovery announcements, new terms. Open access. |
| **INTEGERS** | Combinatorial number theory. **Bans AI-generated content.** |
| **Experimental Mathematics** | Computational experiments |
| **arXiv math.NT** | Preprints, discovery announcements |
