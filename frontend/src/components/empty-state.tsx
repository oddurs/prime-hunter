import { Card, CardContent } from "@/components/ui/card";
import { cn } from "@/lib/utils";

interface EmptyStateProps {
  message: string;
  className?: string;
}

export function EmptyState({ message, className }: EmptyStateProps) {
  return (
    <Card className={cn("py-8 border-dashed", className)}>
      <CardContent className="p-0 px-4 text-center text-muted-foreground text-sm">
        {message}
      </CardContent>
    </Card>
  );
}
