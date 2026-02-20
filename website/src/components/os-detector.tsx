"use client";

import { useEffect, useState } from "react";
import type { OS } from "@/lib/install-commands";

export function useDetectedOS(): OS {
  const [os, setOS] = useState<OS>("linux");

  useEffect(() => {
    const ua = navigator.userAgent.toLowerCase();
    if (ua.includes("mac")) {
      setOS("macos");
    } else if (ua.includes("win")) {
      setOS("windows");
    } else {
      setOS("linux");
    }
  }, []);

  return os;
}

const osLabels: Record<OS, string> = {
  macos: "macOS",
  linux: "Linux",
  windows: "Windows",
};

interface OSTabsProps {
  selected: OS;
  onChange: (os: OS) => void;
}

export function OSTabs({ selected, onChange }: OSTabsProps) {
  return (
    <div className="flex gap-1 p-1 bg-bg-secondary border border-border rounded-md w-fit">
      {(Object.keys(osLabels) as OS[]).map((os) => (
        <button
          key={os}
          onClick={() => onChange(os)}
          className={`px-4 py-1.5 text-sm rounded transition-colors ${
            selected === os
              ? "bg-accent-purple text-white"
              : "text-text-muted hover:text-text"
          }`}
        >
          {osLabels[os]}
        </button>
      ))}
    </div>
  );
}
