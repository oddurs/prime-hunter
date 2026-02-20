import { cn } from "@/lib/cn";

const variants = {
  default: "bg-bg border border-border text-text-muted",
  green: "bg-accent-green/10 border border-accent-green/30 text-accent-green",
  orange: "bg-accent-orange/10 border border-accent-orange/30 text-accent-orange",
  red: "bg-destructive/10 border border-destructive/30 text-destructive",
  purple: "bg-accent-purple/10 border border-accent-purple/30 text-accent-purple",
} as const;

interface BadgeProps {
  variant?: keyof typeof variants;
  className?: string;
  children: React.ReactNode;
}

export function Badge({ variant = "default", className, children }: BadgeProps) {
  return (
    <span
      className={cn(
        "inline-flex items-center text-xs font-mono px-2 py-0.5 rounded-full",
        variants[variant],
        className
      )}
    >
      {children}
    </span>
  );
}
