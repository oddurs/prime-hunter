import { cn } from "@/lib/cn";

interface SectionProps {
  id?: string;
  className?: string;
  children: React.ReactNode;
  secondary?: boolean;
}

export function Section({ id, className, children, secondary = false }: SectionProps) {
  return (
    <section
      id={id}
      className={cn(
        "py-24 px-6",
        secondary && "bg-bg-secondary",
        className
      )}
    >
      <div className="mx-auto max-w-6xl">{children}</div>
    </section>
  );
}
