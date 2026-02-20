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
        "py-24 px-6 sm:px-8 lg:px-12",
        secondary && "bg-card",
        className
      )}
    >
      <div className="mx-auto max-w-7xl">{children}</div>
    </section>
  );
}
