-- Migration 013: Specialized Agent Roles
--
-- Adds named roles (Engine, Frontend, Ops, Research) that bundle domain context,
-- permissions, default model, and associated templates. Roles provide domain-specific
-- defaults when creating agent tasks, so an "engine" task automatically gets the right
-- CLAUDE.md files, permission level, and cost budget.

-- 1. agent_roles table: defines named roles with domain context and defaults
CREATE TABLE agent_roles (
  id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  description TEXT NOT NULL DEFAULT '',
  domains JSONB NOT NULL DEFAULT '["engine"]',
  default_permission_level INTEGER NOT NULL DEFAULT 1
    CHECK (default_permission_level BETWEEN 0 AND 3),
  default_model TEXT NOT NULL DEFAULT 'sonnet',
  system_prompt TEXT,
  default_max_cost_usd NUMERIC(10,2),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 2. Junction table: associates roles with their relevant templates
CREATE TABLE agent_role_templates (
  role_name TEXT NOT NULL REFERENCES agent_roles(name) ON DELETE CASCADE,
  template_name TEXT NOT NULL REFERENCES agent_templates(name) ON DELETE CASCADE,
  PRIMARY KEY (role_name, template_name)
);

-- 3. Add role_name columns to existing tables
ALTER TABLE agent_tasks ADD COLUMN role_name TEXT REFERENCES agent_roles(name);
ALTER TABLE agent_templates ADD COLUMN role_name TEXT REFERENCES agent_roles(name);

-- 4. Seed the four core roles
INSERT INTO agent_roles (name, description, domains, default_permission_level, default_model, system_prompt, default_max_cost_usd) VALUES
  ('engine',
   'Engine optimization and prime-hunting algorithm specialist',
   '["engine"]',
   2,
   'sonnet',
   'You are an engine optimization specialist for primehunt. You work with rug/GMP arbitrary-precision arithmetic, modular sieves, and primality testing algorithms. Always run `cargo test` after changes. Follow the patterns in src/CLAUDE.md.',
   5.00),
  ('frontend',
   'Frontend specialist for the Next.js dashboard',
   '["frontend"]',
   2,
   'sonnet',
   'You are a frontend specialist for the primehunt dashboard. You work with Next.js 16, React 19, Tailwind 4, shadcn/ui, and Recharts. Always run `npm run build` in the frontend/ directory after changes. Follow the patterns in frontend/CLAUDE.md.',
   3.00),
  ('ops',
   'Operations and infrastructure specialist',
   '["deploy","server"]',
   3,
   'sonnet',
   'You are an ops/infrastructure specialist for primehunt. You manage SSH deployments, systemd services, PGO builds, and server configuration. Be cautious with destructive operations — always confirm before modifying production systems.',
   10.00),
  ('research',
   'Research analyst for prime number theory and competitive analysis',
   '["docs"]',
   0,
   'haiku',
   'You are a research analyst for primehunt. You analyze OEIS sequences, academic papers, and competitive prime-hunting strategies. You NEVER write or modify code — your output is always analysis, recommendations, and documentation.',
   1.00);

-- 5. Seed 8 role-specific templates

-- Engine templates
INSERT INTO agent_templates (name, description, steps, role_name) VALUES
  ('implement-prime-form',
   'Implement a new prime form search module (5-step workflow)',
   '[
     {"title":"Research form","description":"Research the mathematical properties, OEIS sequences, known results, and optimal algorithms for this prime form. Document sieve strategy and primality test approach.","permission_level":0},
     {"title":"Implement sieve","description":"Implement the modular sieve for candidate filtering. Follow patterns from existing forms in src/ (e.g., factorial.rs, kbn.rs).","permission_level":1,"depends_on_step":0},
     {"title":"Implement primality test","description":"Implement the primality test and proof generation. Use rug::Integer for arithmetic, follow test_prime patterns from kbn.rs.","permission_level":1,"depends_on_step":1},
     {"title":"Wire into CLI","description":"Add CLI subcommand in main.rs, checkpoint variant, search_manager entry, deploy.rs, and lib.rs re-exports.","permission_level":1,"depends_on_step":2},
     {"title":"Test and verify","description":"Run cargo test, test with small ranges, verify results against OEIS. Ensure all existing tests still pass.","permission_level":1,"depends_on_step":3}
   ]'::jsonb,
   'engine'),
  ('optimize-sieve',
   'Optimize sieve performance for a search form (4-step workflow)',
   '[
     {"title":"Profile current performance","description":"Benchmark current sieve throughput using small ranges. Identify bottlenecks with timing measurements.","permission_level":1},
     {"title":"Design optimization","description":"Analyze the sieve algorithm and propose optimizations (wheel factorization, Montgomery multiplication, batch processing).","permission_level":0,"depends_on_step":0},
     {"title":"Implement optimization","description":"Implement the optimized sieve. Ensure correctness by comparing results with the original implementation.","permission_level":1,"depends_on_step":1},
     {"title":"Benchmark and verify","description":"Run benchmarks comparing old vs new performance. Run cargo test to verify correctness.","permission_level":1,"depends_on_step":2}
   ]'::jsonb,
   'engine');

-- Frontend templates
INSERT INTO agent_templates (name, description, steps, role_name) VALUES
  ('add-page',
   'Add a new page to the frontend dashboard (4-step workflow)',
   '[
     {"title":"Plan page layout","description":"Design the page structure, identify data sources (Supabase queries or WebSocket), and list required components.","permission_level":0},
     {"title":"Create page and hooks","description":"Create the page.tsx file in app/ and any required custom hooks in hooks/. Follow existing patterns.","permission_level":1,"depends_on_step":0},
     {"title":"Add navigation","description":"Add the page to app-header.tsx navigation and update any relevant routing.","permission_level":1,"depends_on_step":1},
     {"title":"Build and verify","description":"Run npm run build in frontend/ to verify no TypeScript errors. Check responsive layout.","permission_level":1,"depends_on_step":2}
   ]'::jsonb,
   'frontend'),
  ('add-component',
   'Create a new reusable component (3-step workflow)',
   '[
     {"title":"Design component API","description":"Define the component props, state, and behavior. Identify which shadcn/ui primitives to use.","permission_level":0},
     {"title":"Implement component","description":"Create the component file in components/. Use Tailwind for styling, Lucide for icons.","permission_level":1,"depends_on_step":0},
     {"title":"Build and verify","description":"Run npm run build in frontend/ to verify no TypeScript errors.","permission_level":1,"depends_on_step":1}
   ]'::jsonb,
   'frontend');

-- Ops templates
INSERT INTO agent_templates (name, description, steps, role_name) VALUES
  ('deploy-update',
   'Deploy an update to production servers (3-step workflow)',
   '[
     {"title":"Build release binary","description":"Run cargo build --release (or PGO build if available). Verify binary size and dependencies.","permission_level":2},
     {"title":"Deploy to servers","description":"Use deploy.rs or deploy.sh to push the binary to production servers. Monitor rollout.","permission_level":3,"depends_on_step":0},
     {"title":"Verify deployment","description":"Check that services are running, workers are heartbeating, and searches resume correctly.","permission_level":1,"depends_on_step":1}
   ]'::jsonb,
   'ops'),
  ('scale-fleet',
   'Scale the worker fleet up or down (3-step workflow)',
   '[
     {"title":"Assess current capacity","description":"Check fleet status, worker counts, CPU utilization, and search throughput.","permission_level":0},
     {"title":"Adjust worker count","description":"Start or stop worker instances via systemd. Configure search assignments.","permission_level":3,"depends_on_step":0},
     {"title":"Monitor scaling","description":"Verify new workers register, throughput adjusts, and no work blocks are orphaned.","permission_level":1,"depends_on_step":1}
   ]'::jsonb,
   'ops');

-- Research templates
INSERT INTO agent_templates (name, description, steps, role_name) VALUES
  ('research-form',
   'Research a prime form for potential implementation (4-step workflow)',
   '[
     {"title":"Survey OEIS sequences","description":"Look up relevant OEIS sequences, known primes, and open conjectures for this form.","permission_level":0},
     {"title":"Analyze algorithms","description":"Research optimal sieve strategies, primality tests, and proof methods. Compare with existing implementations.","permission_level":0,"depends_on_step":0},
     {"title":"Competitive analysis","description":"Check PrimePages, GIMPS, and other projects for current records and active searches.","permission_level":0,"depends_on_step":0},
     {"title":"Write recommendation","description":"Summarize findings with a recommendation on whether to implement, expected difficulty, and potential for record-setting discoveries.","permission_level":0,"depends_on_step":1}
   ]'::jsonb,
   'research'),
  ('analyze-results',
   'Analyze search results and recommend next steps (3-step workflow)',
   '[
     {"title":"Gather statistics","description":"Query prime discovery data: counts by form, digit distributions, throughput rates, verification status.","permission_level":0},
     {"title":"Identify patterns","description":"Analyze discovery patterns, density estimates, and compare with theoretical predictions.","permission_level":0,"depends_on_step":0},
     {"title":"Recommend next searches","description":"Based on analysis, recommend which forms/ranges to search next for maximum discovery potential.","permission_level":0,"depends_on_step":1}
   ]'::jsonb,
   'research');

-- 6. Wire role-template associations (role-specific + relevant generic templates)

-- Engine role: its own templates + generic implement/fix-bug
INSERT INTO agent_role_templates (role_name, template_name) VALUES
  ('engine', 'implement-prime-form'),
  ('engine', 'optimize-sieve'),
  ('engine', 'implement-feature'),
  ('engine', 'fix-bug'),
  ('engine', 'run-search');

-- Frontend role: its own templates + generic implement/fix-bug/code-review
INSERT INTO agent_role_templates (role_name, template_name) VALUES
  ('frontend', 'add-page'),
  ('frontend', 'add-component'),
  ('frontend', 'implement-feature'),
  ('frontend', 'fix-bug'),
  ('frontend', 'code-review');

-- Ops role: its own templates + generic run-search
INSERT INTO agent_role_templates (role_name, template_name) VALUES
  ('ops', 'deploy-update'),
  ('ops', 'scale-fleet'),
  ('ops', 'run-search');

-- Research role: its own templates + generic update-docs
INSERT INTO agent_role_templates (role_name, template_name) VALUES
  ('research', 'research-form'),
  ('research', 'analyze-results'),
  ('research', 'update-docs');

-- 7. Enable realtime for the new tables
ALTER PUBLICATION supabase_realtime ADD TABLE agent_roles;
ALTER PUBLICATION supabase_realtime ADD TABLE agent_role_templates;
