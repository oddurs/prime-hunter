import { cn } from "@/lib/cn";
import { type ButtonHTMLAttributes, type AnchorHTMLAttributes } from "react";

const variants = {
  primary:
    "bg-accent-purple text-white hover:opacity-90",
  outline:
    "border border-border text-text-muted hover:text-text hover:border-text-muted",
  ghost:
    "text-text-muted hover:text-text hover:bg-bg-secondary",
} as const;

const sizes = {
  sm: "px-3 py-1.5 text-sm",
  md: "px-5 py-2.5 text-sm",
  lg: "px-6 py-3 text-base",
} as const;

type ButtonProps = {
  variant?: keyof typeof variants;
  size?: keyof typeof sizes;
} & (
  | ({ href: string } & AnchorHTMLAttributes<HTMLAnchorElement>)
  | ({ href?: never } & ButtonHTMLAttributes<HTMLButtonElement>)
);

export function Button({
  variant = "primary",
  size = "md",
  className,
  ...props
}: ButtonProps) {
  const classes = cn(
    "inline-flex items-center justify-center rounded-md font-medium transition-colors",
    variants[variant],
    sizes[size],
    className
  );

  if ("href" in props && props.href) {
    return <a className={classes} {...(props as AnchorHTMLAttributes<HTMLAnchorElement>)} />;
  }

  return <button className={classes} {...(props as ButtonHTMLAttributes<HTMLButtonElement>)} />;
}
