"use client";

import { useState } from "react";
import { CodeBlock } from "./ui/code-block";
import { OSTabs, useDetectedOS } from "./os-detector";
import { installMethods, type OS } from "@/lib/install-commands";

export function InstallCommand() {
  const detectedOS = useDetectedOS();
  const [os, setOS] = useState<OS>(detectedOS);
  const [methodIndex, setMethodIndex] = useState(0);

  const methods = installMethods[os];
  const activeMethod = methods[methodIndex] ?? methods[0];

  return (
    <div className="space-y-4">
      <OSTabs selected={os} onChange={(o) => { setOS(o); setMethodIndex(0); }} />

      <div className="flex gap-2">
        {methods.map((method, i) => (
          <button
            key={method.label}
            onClick={() => setMethodIndex(i)}
            className={`px-3 py-1 text-sm rounded-md border transition-colors ${
              i === methodIndex
                ? "border-accent-purple text-accent-purple"
                : "border-border text-muted-foreground hover:text-foreground"
            }`}
          >
            {method.label}
          </button>
        ))}
      </div>

      <CodeBlock language={os === "windows" ? "powershell" : "bash"}>
        {activeMethod.commands}
      </CodeBlock>
    </div>
  );
}
