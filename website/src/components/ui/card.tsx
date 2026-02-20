import { cn } from "@/lib/cn";
import type { HTMLAttributes } from "react";

interface CardProps extends HTMLAttributes<HTMLDivElement> {
  hover?: boolean;
}

export function Card({ hover = false, className, children, ...props }: CardProps) {
  return (
    <div
      className={cn(
        "rounded-md border border-border bg-card p-5",
        hover && "card-glow",
        className
      )}
      {...props}
    >
      {children}
    </div>
  );
}
