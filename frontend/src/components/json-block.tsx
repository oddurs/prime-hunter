interface JsonBlockProps {
  label: string;
  data: unknown;
  maxHeight?: string;
}

export function JsonBlock({ label, data, maxHeight = "max-h-40" }: JsonBlockProps) {
  return (
    <div>
      <div className="text-xs font-medium text-muted-foreground mb-1">{label}</div>
      <pre className={`bg-muted rounded-md p-3 text-xs overflow-auto ${maxHeight}`}>
        {typeof data === "string" ? data : JSON.stringify(data, null, 2)}
      </pre>
    </div>
  );
}
