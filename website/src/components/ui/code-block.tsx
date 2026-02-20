"use client";

import { cn } from "@/lib/cn";
import { Copy, Check } from "lucide-react";
import { useState } from "react";

interface CodeBlockProps {
  children: string;
  language?: string;
  className?: string;
}

export function CodeBlock({ children, language, className }: CodeBlockProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    navigator.clipboard.writeText(children);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className={cn("relative group", className)}>
      {language && (
        <div className="absolute top-0 left-0 px-3 py-1 text-xs text-text-muted font-mono bg-bg-secondary border-b border-r border-border rounded-tl-md rounded-br-md">
          {language}
        </div>
      )}
      <button
        onClick={handleCopy}
        className="absolute top-2 right-2 p-1.5 rounded-md bg-bg border border-border text-text-muted hover:text-text opacity-0 group-hover:opacity-100 transition-opacity"
        aria-label="Copy code"
      >
        {copied ? <Check size={14} /> : <Copy size={14} />}
      </button>
      <pre className="code-block">
        <code>{children}</code>
      </pre>
    </div>
  );
}
