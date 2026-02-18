/**
 * @module utils
 *
 * Tailwind CSS class merging utility. The `cn()` function combines
 * `clsx` (conditional class joining) with `tailwind-merge` (deduplication
 * of conflicting Tailwind classes like `px-2 px-4` → `px-4`).
 *
 * Standard shadcn/ui utility — used by every component in `components/ui/`.
 */

import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}
