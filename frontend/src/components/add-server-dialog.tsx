"use client";

/**
 * @module add-server-dialog
 *
 * Modal dialog for adding a new remote server to the fleet. Collects
 * hostname, SSH user, SSH key path, and search parameters, then POSTs
 * a deployment request to the Rust backend's `/api/deploy` endpoint.
 */

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

interface AddServerDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onDeployed: () => void;
}

type SearchType = "kbn" | "factorial" | "palindromic";

const defaults: Record<SearchType, Record<string, number>> = {
  kbn: { k: 3, base: 2, min_n: 1, max_n: 100000 },
  factorial: { start: 1, end: 10000 },
  palindromic: { base: 10, min_digits: 1, max_digits: 11 },
};

export function AddServerDialog({ open, onOpenChange, onDeployed }: AddServerDialogProps) {
  const [hostname, setHostname] = useState("");
  const [sshUser, setSshUser] = useState("root");
  const [sshKey, setSshKey] = useState("");
  const [searchType, setSearchType] = useState<SearchType>("kbn");
  const [values, setValues] = useState<Record<string, number>>(defaults.kbn);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  function handleTypeChange(type: SearchType) {
    setSearchType(type);
    setValues(defaults[type]);
    setError(null);
  }

  function handleValueChange(key: string, raw: string) {
    const num = parseInt(raw, 10);
    if (!isNaN(num)) {
      setValues((prev) => ({ ...prev, [key]: num }));
    }
  }

  async function handleSubmit() {
    if (!hostname.trim()) {
      setError("Hostname is required");
      return;
    }
    setError(null);
    setSubmitting(true);
    try {
      const body: Record<string, unknown> = {
        hostname: hostname.trim(),
        ssh_user: sshUser.trim() || "root",
        coordinator_url: window.location.origin,
        search_type: searchType,
        ...values,
      };
      if (sshKey.trim()) {
        body.ssh_key = sshKey.trim();
      }
      const res = await fetch("/api/fleet/deploy", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        throw new Error(data.error || `HTTP ${res.status}`);
      }
      onOpenChange(false);
      onDeployed();
      // Reset form
      setHostname("");
      setSshUser("root");
      setSshKey("");
      setSearchType("kbn");
      setValues(defaults.kbn);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to deploy");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>Add Server</DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
          <div className="text-xs font-medium text-muted-foreground">
            SSH Connection
          </div>

          <div>
            <label className="text-xs font-medium text-muted-foreground mb-1 block">
              Hostname / IP
            </label>
            <Input
              placeholder="192.168.1.100"
              value={hostname}
              onChange={(e) => setHostname(e.target.value)}
            />
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="text-xs font-medium text-muted-foreground mb-1 block">
                SSH User
              </label>
              <Input
                placeholder="root"
                value={sshUser}
                onChange={(e) => setSshUser(e.target.value)}
              />
            </div>
            <div>
              <label className="text-xs font-medium text-muted-foreground mb-1 block">
                SSH Key (optional)
              </label>
              <Input
                placeholder="~/.ssh/id_rsa"
                value={sshKey}
                onChange={(e) => setSshKey(e.target.value)}
              />
            </div>
          </div>

          <div className="border-t pt-4">
            <div className="text-xs font-medium text-muted-foreground mb-3">
              Search Configuration
            </div>

            <div>
              <label className="text-xs font-medium text-muted-foreground mb-1 block">
                Search type
              </label>
              <Select value={searchType} onValueChange={(v) => handleTypeChange(v as SearchType)}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="kbn">KBN (k*b^n +/- 1)</SelectItem>
                  <SelectItem value="factorial">Factorial (n! +/- 1)</SelectItem>
                  <SelectItem value="palindromic">Palindromic</SelectItem>
                </SelectContent>
              </Select>
            </div>

            {searchType === "kbn" && (
              <>
                <div className="grid grid-cols-2 gap-3 mt-3">
                  <div>
                    <label className="text-xs font-medium text-muted-foreground mb-1 block">k</label>
                    <Input type="number" value={values.k} onChange={(e) => handleValueChange("k", e.target.value)} />
                  </div>
                  <div>
                    <label className="text-xs font-medium text-muted-foreground mb-1 block">Base</label>
                    <Input type="number" value={values.base} onChange={(e) => handleValueChange("base", e.target.value)} />
                  </div>
                </div>
                <div className="grid grid-cols-2 gap-3 mt-3">
                  <div>
                    <label className="text-xs font-medium text-muted-foreground mb-1 block">Min n</label>
                    <Input type="number" value={values.min_n} onChange={(e) => handleValueChange("min_n", e.target.value)} />
                  </div>
                  <div>
                    <label className="text-xs font-medium text-muted-foreground mb-1 block">Max n</label>
                    <Input type="number" value={values.max_n} onChange={(e) => handleValueChange("max_n", e.target.value)} />
                  </div>
                </div>
              </>
            )}

            {searchType === "factorial" && (
              <div className="grid grid-cols-2 gap-3 mt-3">
                <div>
                  <label className="text-xs font-medium text-muted-foreground mb-1 block">Start</label>
                  <Input type="number" value={values.start} onChange={(e) => handleValueChange("start", e.target.value)} />
                </div>
                <div>
                  <label className="text-xs font-medium text-muted-foreground mb-1 block">End</label>
                  <Input type="number" value={values.end} onChange={(e) => handleValueChange("end", e.target.value)} />
                </div>
              </div>
            )}

            {searchType === "palindromic" && (
              <>
                <div className="mt-3">
                  <label className="text-xs font-medium text-muted-foreground mb-1 block">Base</label>
                  <Input type="number" value={values.base} onChange={(e) => handleValueChange("base", e.target.value)} />
                </div>
                <div className="grid grid-cols-2 gap-3 mt-3">
                  <div>
                    <label className="text-xs font-medium text-muted-foreground mb-1 block">Min digits</label>
                    <Input type="number" value={values.min_digits} onChange={(e) => handleValueChange("min_digits", e.target.value)} />
                  </div>
                  <div>
                    <label className="text-xs font-medium text-muted-foreground mb-1 block">Max digits</label>
                    <Input type="number" value={values.max_digits} onChange={(e) => handleValueChange("max_digits", e.target.value)} />
                  </div>
                </div>
              </>
            )}
          </div>

          {error && (
            <div className="text-sm text-red-500 bg-red-500/10 rounded-md px-3 py-2">
              {error}
            </div>
          )}

          <Button onClick={handleSubmit} disabled={submitting} className="w-full">
            {submitting ? "Deploying..." : "Deploy Worker"}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
