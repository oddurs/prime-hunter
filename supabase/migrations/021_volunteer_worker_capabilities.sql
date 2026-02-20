-- Volunteer worker capability metadata for assignment gating.
--
-- Supports host-aware scheduling via /api/v1/work query and
-- persisted worker capability profiles.

ALTER TABLE volunteer_workers
  ADD COLUMN IF NOT EXISTS os TEXT,
  ADD COLUMN IF NOT EXISTS arch TEXT,
  ADD COLUMN IF NOT EXISTS ram_gb INTEGER,
  ADD COLUMN IF NOT EXISTS has_gpu BOOLEAN DEFAULT FALSE,
  ADD COLUMN IF NOT EXISTS gpu_model TEXT,
  ADD COLUMN IF NOT EXISTS gpu_vram_gb INTEGER;

CREATE INDEX IF NOT EXISTS idx_volunteer_workers_os_arch ON volunteer_workers(os, arch);
CREATE INDEX IF NOT EXISTS idx_volunteer_workers_has_gpu ON volunteer_workers(has_gpu);
