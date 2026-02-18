/**
 * @module new-project-dialog
 *
 * Dialog for creating a new prime-hunting project. Supports two modes:
 * 1. Form-based creation with objective, form, and target fields
 * 2. TOML import for version-controlled project definitions
 */

import { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
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
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { API_BASE } from "@/lib/format";

const FORMS = [
  "factorial",
  "primorial",
  "kbn",
  "palindromic",
  "near_repdigit",
  "cullen_woodall",
  "wagstaff",
  "carol_kynea",
  "twin",
  "sophie_germain",
  "repunit",
  "gen_fermat",
];

const OBJECTIVES = ["record", "survey", "verification", "custom"];

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function NewProjectDialog({ open, onOpenChange }: Props) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [form, setForm] = useState("factorial");
  const [objective, setObjective] = useState("survey");
  const [rangeStart, setRangeStart] = useState("");
  const [rangeEnd, setRangeEnd] = useState("");
  const [toml, setToml] = useState("");
  const [submitting, setSubmitting] = useState(false);

  async function handleCreate() {
    if (!name.trim()) {
      toast.error("Project name is required");
      return;
    }
    setSubmitting(true);
    try {
      const body: Record<string, unknown> = {
        name: name.trim(),
        description,
        objective,
        form,
        target: {
          range_start: rangeStart ? Number(rangeStart) : undefined,
          range_end: rangeEnd ? Number(rangeEnd) : undefined,
        },
        strategy: { auto_strategy: true, phases: [] },
      };
      const res = await fetch(`${API_BASE}/api/projects`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      const data = await res.json();
      if (res.ok) {
        toast.success(`Project '${name}' created (slug: ${data.slug})`);
        onOpenChange(false);
        resetForm();
      } else {
        toast.error(data.error || "Failed to create project");
      }
    } catch {
      toast.error("Failed to create project");
    } finally {
      setSubmitting(false);
    }
  }

  async function handleImport() {
    if (!toml.trim()) {
      toast.error("TOML content is required");
      return;
    }
    setSubmitting(true);
    try {
      const res = await fetch(`${API_BASE}/api/projects/import`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ toml: toml.trim() }),
      });
      const data = await res.json();
      if (res.ok) {
        toast.success(`Project imported (slug: ${data.slug})`);
        onOpenChange(false);
        setToml("");
      } else {
        toast.error(data.error || "Failed to import project");
      }
    } catch {
      toast.error("Failed to import project");
    } finally {
      setSubmitting(false);
    }
  }

  function resetForm() {
    setName("");
    setDescription("");
    setForm("factorial");
    setObjective("survey");
    setRangeStart("");
    setRangeEnd("");
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>New Project</DialogTitle>
        </DialogHeader>

        <Tabs defaultValue="form">
          <TabsList className="mb-4">
            <TabsTrigger value="form">Create</TabsTrigger>
            <TabsTrigger value="toml">Import TOML</TabsTrigger>
          </TabsList>

          <TabsContent value="form" className="space-y-4">
            <div>
              <label className="text-sm font-medium">Name</label>
              <Input
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="wagstaff-record-2026"
              />
            </div>
            <div>
              <label className="text-sm font-medium">Description</label>
              <Input
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="Hunt for new Wagstaff primes"
              />
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="text-sm font-medium">Form</label>
                <Select value={form} onValueChange={setForm}>
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {FORMS.map((f) => (
                      <SelectItem key={f} value={f}>
                        {f}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div>
                <label className="text-sm font-medium">Objective</label>
                <Select value={objective} onValueChange={setObjective}>
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {OBJECTIVES.map((o) => (
                      <SelectItem key={o} value={o}>
                        {o}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="text-sm font-medium">Range Start</label>
                <Input
                  type="number"
                  value={rangeStart}
                  onChange={(e) => setRangeStart(e.target.value)}
                  placeholder="1"
                />
              </div>
              <div>
                <label className="text-sm font-medium">Range End</label>
                <Input
                  type="number"
                  value={rangeEnd}
                  onChange={(e) => setRangeEnd(e.target.value)}
                  placeholder="10000"
                />
              </div>
            </div>
            <Button
              onClick={handleCreate}
              disabled={submitting || !name.trim()}
              className="w-full"
            >
              {submitting ? "Creating..." : "Create Project"}
            </Button>
          </TabsContent>

          <TabsContent value="toml" className="space-y-4">
            <Textarea
              rows={12}
              value={toml}
              onChange={(e) => setToml(e.target.value)}
              placeholder={`[project]\nname = "my-project"\nobjective = "survey"\nform = "factorial"\n\n[target]\nrange_start = 1\nrange_end = 10000\n\n[strategy]\nauto_strategy = true`}
              className="font-mono text-xs"
            />
            <Button
              onClick={handleImport}
              disabled={submitting || !toml.trim()}
              className="w-full"
            >
              {submitting ? "Importing..." : "Import TOML"}
            </Button>
          </TabsContent>
        </Tabs>
      </DialogContent>
    </Dialog>
  );
}
