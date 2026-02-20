# Distributed Compute Landscape

Summary of distributed/volunteer computing models and how the leading prime projects organize work. Reviewed 2026-02-19.

## Volunteer Computing Model (BOINC)

BOINC is the dominant volunteer-computing middleware. Participants install a BOINC client that periodically requests work from project servers, downloads inputs, executes jobs, and uploads results. Projects are autonomous and operate their own servers; volunteers can attach to multiple projects and allocate resource share between them. BOINC is designed for high-throughput computing on heterogeneous, untrusted, and unreliable volunteer machines, so result validation and throughput-focused scheduling are central design goals.

### Core server pipeline

BOINC projects use a multi-daemon server architecture. Key concepts:

- Jobs are issued as workunits with multiple instances (replicas) and validated using replication.
- BOINC supports adaptive replication: if a host/app version has a strong track record of correct results, replication can be reduced to save throughput.
- The scheduler is a web endpoint that dispatches job instances to clients.
- To avoid high database load, BOINC uses a shared-memory cache of dispatchable job instances. A feeder process populates this cache; the scheduler scans it to send work.
- Validator compares returned results for a job, determines if a quorum of equivalent results exists, and designates a canonical result.
- Assimilator processes completed, validated work (e.g., parse output, move files, write to DB).
- Transitioner handles state transitions for jobs/workunits based on flags set by schedulers/validators.
- File deleter removes input/output files after assimilation; database purger removes DB records.

This architecture is designed for high-throughput workloads, with many short or medium-length jobs and relatively low inter-node communication.

## GIMPS (Mersenne primes)

GIMPS runs a specialized distributed pipeline for Mersenne numbers (2^p - 1):

- Generate prime exponents p.
- Trial factoring and P-1 factoring are run first to eliminate candidates.
- If no factor is found, a PRP test is run.
- If PRP indicates probable prime, a Lucas-Lehmer test confirms primality.
- For some smaller composites, ECM is used to attempt to find larger factors.

GIMPS also supports PRP proof certification work types for verifying PRP proofs on reliable machines.

## PrimeGrid (multi-form primes)

PrimeGrid is a BOINC-based volunteer project. Participants install BOINC, attach to PrimeGrid, and choose which subprojects to run via project preferences.

PrimeGrid runs multiple subprojects covering several prime forms, including (at minimum):

- 321 Prime Search (3*2^n +/- 1)
- Cullen/Woodall Search
- Generalized Cullen/Woodall Search
- Extended Sierpinski Problem
- Generalized Fermat Prime Search
- Prime Sierpinski Project
- Proth Prime Search
- Seventeen or Bust
- Sierpinski/Riesel Base 5
- The Riesel Problem
- AP27 Search (arithmetic progressions)

## Implications for the prime-hunter ecosystem (inferred)

If we want parity with GIMPS/PrimeGrid-style distributed compute, the baseline feature set looks like:

- A single, low-friction client install with background execution.
- Work-type routing and user preferences (prime form, CPU vs GPU, memory limits).
- Multi-stage pipeline: sieve/trial factoring/P-1 before expensive PRP/LLR runs.
- Proof-first verification to reduce double-check load (PRP proofs where possible).
- High-throughput server architecture (work cache + scheduler + validator + assimilator).
- Public dashboards: project status, milestones, leaderboards, attribution.
- A clear onboarding funnel and “choose your subproject” UX.

## Sources

- https://boinc.berkeley.edu/boinc_a_platform_for_volunteer_computing.pdf
- https://www.mersenne.org/various/works.php
- https://www.mersenne.org/various/math.php
- https://www.mersenne.org/worktypes/
- https://www.mersenne.org/
- https://www.primegrid.com/
- https://www.primegrid.com/forum_index.php
