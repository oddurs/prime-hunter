"use client";

/**
 * @module account/page
 *
 * Operator-facing account and profile page. Displays editable profile
 * information (display name, email, role), API key management with
 * visibility toggle, copy, and rotation, and operator stats (credit,
 * primes found, trust level, rank).
 *
 * Data sources:
 * - Profile: `user_profiles` table via Supabase client
 * - Operator data: `operators` table via Supabase client (JWT auth)
 * - Key rotation: POST to `/api/v1/operators/rotate-key` with Bearer token
 */

import { useEffect, useState, useCallback } from "react";
import { toast } from "sonner";
import { Copy, Eye, EyeOff, Key, RefreshCw, Shield, User } from "lucide-react";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { ViewHeader } from "@/components/view-header";
import { useAuth } from "@/contexts/auth-context";
import { API_BASE, relativeTime, formatTime } from "@/lib/format";
import { supabase } from "@/lib/supabase";

interface OperatorProfile {
  id: string;
  username: string;
  email: string;
  api_key: string;
  team: string | null;
  credit: number;
  primes_found: number;
  joined_at: string;
}

/** Map numeric trust level to a human-readable label. */
function trustLabel(level: number): string {
  if (level >= 3) return "Trusted";
  if (level === 2) return "Reliable";
  return "New";
}

/** Badge color variant for trust levels. */
function trustBadgeClass(level: number): string {
  if (level >= 3) return "border-emerald-500/40 bg-emerald-500/10 text-emerald-400";
  if (level === 2) return "border-blue-500/40 bg-blue-500/10 text-blue-400";
  return "border-amber-500/40 bg-amber-500/10 text-amber-300";
}

/** Mask an API key, showing only the first 4 and last 4 characters. */
function maskApiKey(key: string): string {
  if (key.length <= 8) return key;
  return `${key.slice(0, 4)}...${key.slice(-4)}`;
}

export default function AccountPage() {
  const { user, session, role, operatorId } = useAuth();

  // Profile state
  const [displayName, setDisplayName] = useState("");
  const [savedDisplayName, setSavedDisplayName] = useState("");
  const [savingName, setSavingName] = useState(false);

  // Operator state
  const [operator, setOperator] = useState<OperatorProfile | null>(null);
  const [loadingOperator, setLoadingOperator] = useState(true);

  // API key visibility
  const [keyVisible, setKeyVisible] = useState(false);
  const [rotatingKey, setRotatingKey] = useState(false);
  const [confirmRotate, setConfirmRotate] = useState(false);

  // Trust / rank (derived from operator stats or defaults)
  const [trustLevel, setTrustLevel] = useState(1);
  const [rank, setRank] = useState<number | null>(null);

  /** Fetch user profile (display_name, role) from user_profiles. */
  const fetchProfile = useCallback(async () => {
    if (!user) return;
    const { data, error } = await supabase
      .from("user_profiles")
      .select("display_name, role")
      .eq("id", user.id)
      .single();
    if (!error && data) {
      setDisplayName(data.display_name ?? "");
      setSavedDisplayName(data.display_name ?? "");
    }
  }, [user]);

  /** Fetch operator data from operators table via Supabase. */
  const fetchOperator = useCallback(async () => {
    if (!operatorId) {
      setLoadingOperator(false);
      return;
    }
    const { data, error } = await supabase
      .from("operators")
      .select("*")
      .eq("id", operatorId)
      .single();
    if (!error && data) {
      setOperator(data as OperatorProfile);
      // Derive trust level from credit thresholds
      if (data.credit >= 1000) setTrustLevel(3);
      else if (data.credit >= 100) setTrustLevel(2);
      else setTrustLevel(1);
    }
    setLoadingOperator(false);
  }, [operatorId]);

  /** Fetch operator rank (position by primes_found). */
  const fetchRank = useCallback(async () => {
    if (!operatorId) return;
    const { count, error } = await supabase
      .from("operators")
      .select("id", { count: "exact", head: true })
      .gte("primes_found", operator?.primes_found ?? 0);
    if (!error && count !== null) {
      setRank(count);
    }
  }, [operatorId, operator?.primes_found]);

  useEffect(() => {
    fetchProfile();
    fetchOperator();
  }, [fetchProfile, fetchOperator]);

  useEffect(() => {
    if (operator) fetchRank();
  }, [operator, fetchRank]);

  /** Save display name to user_profiles. */
  async function saveDisplayName() {
    if (!user || displayName === savedDisplayName) return;
    setSavingName(true);
    const { error } = await supabase
      .from("user_profiles")
      .update({ display_name: displayName })
      .eq("id", user.id);
    if (error) {
      toast.error("Failed to save display name: " + error.message);
    } else {
      setSavedDisplayName(displayName);
      toast.success("Display name updated");
    }
    setSavingName(false);
  }

  /** Copy API key to clipboard. */
  async function copyApiKey() {
    if (!operator?.api_key) return;
    try {
      await navigator.clipboard.writeText(operator.api_key);
      toast.success("API key copied to clipboard");
    } catch {
      toast.error("Failed to copy API key");
    }
  }

  /** Rotate API key via backend endpoint. */
  async function rotateApiKey() {
    if (!session?.access_token) return;
    setRotatingKey(true);
    try {
      const res = await fetch(`${API_BASE}/api/v1/operators/rotate-key`, {
        method: "POST",
        headers: {
          Authorization: `Bearer ${session.access_token}`,
        },
      });
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        throw new Error(data.error || `HTTP ${res.status}`);
      }
      toast.success("API key rotated successfully");
      // Refresh operator data to get the new key
      await fetchOperator();
      setKeyVisible(false);
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Failed to rotate API key";
      toast.error(message);
    } finally {
      setRotatingKey(false);
      setConfirmRotate(false);
    }
  }

  const hasNameChanged = displayName !== savedDisplayName;

  return (
    <div className="container mx-auto max-w-4xl px-4 py-6">
      <ViewHeader
        title="Account"
        subtitle="Manage your profile, API key, and view operator stats."
        metadata={
          <div className="flex gap-3 text-sm text-muted-foreground">
            {role && (
              <Badge variant="outline" className="capitalize">
                {role}
              </Badge>
            )}
            {operator && (
              <span className="flex items-center gap-1">
                <User className="h-3.5 w-3.5" />
                {operator.username}
              </span>
            )}
          </div>
        }
      />

      <div className="space-y-6 mt-4">
        {/* ── Profile Section ──────────────────────────────── */}
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-base flex items-center gap-2">
              <User className="h-4 w-4" />
              Profile
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            {/* Display name */}
            <div className="space-y-1.5">
              <label className="text-xs font-medium text-muted-foreground">
                Display Name
              </label>
              <div className="flex gap-2">
                <Input
                  value={displayName}
                  onChange={(e) => setDisplayName(e.target.value)}
                  placeholder="Enter display name"
                  className="max-w-sm"
                />
                <Button
                  size="sm"
                  onClick={() => void saveDisplayName()}
                  disabled={!hasNameChanged || savingName}
                >
                  {savingName ? "Saving..." : "Save"}
                </Button>
              </div>
            </div>

            {/* Email (readonly) */}
            <div className="space-y-1.5">
              <label className="text-xs font-medium text-muted-foreground">
                Email
              </label>
              <Input
                value={user?.email ?? ""}
                readOnly
                className="max-w-sm bg-muted/50 cursor-not-allowed"
              />
            </div>

            {/* Role & Joined */}
            <div className="flex flex-wrap gap-6">
              <div className="space-y-1.5">
                <label className="text-xs font-medium text-muted-foreground">
                  Role
                </label>
                <div>
                  <Badge variant="outline" className="capitalize">
                    {role ?? "operator"}
                  </Badge>
                </div>
              </div>
              <div className="space-y-1.5">
                <label className="text-xs font-medium text-muted-foreground">
                  Joined
                </label>
                <div className="text-sm">
                  {operator?.joined_at
                    ? formatTime(operator.joined_at)
                    : user?.created_at
                      ? formatTime(user.created_at)
                      : "-"}
                </div>
              </div>
              {operator?.team && (
                <div className="space-y-1.5">
                  <label className="text-xs font-medium text-muted-foreground">
                    Team
                  </label>
                  <div className="text-sm">{operator.team}</div>
                </div>
              )}
            </div>
          </CardContent>
        </Card>

        {/* ── API Key Section ──────────────────────────────── */}
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-base flex items-center gap-2">
              <Key className="h-4 w-4" />
              API Key
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            {loadingOperator ? (
              <div className="text-sm text-muted-foreground">Loading...</div>
            ) : operator?.api_key ? (
              <>
                <div className="flex items-center gap-2">
                  <code className="flex-1 max-w-md rounded border bg-muted/50 px-3 py-2 font-mono text-sm">
                    {keyVisible ? operator.api_key : maskApiKey(operator.api_key)}
                  </code>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => setKeyVisible((v) => !v)}
                    title={keyVisible ? "Hide key" : "Show key"}
                  >
                    {keyVisible ? (
                      <EyeOff className="h-4 w-4" />
                    ) : (
                      <Eye className="h-4 w-4" />
                    )}
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => void copyApiKey()}
                    title="Copy key"
                  >
                    <Copy className="h-4 w-4" />
                  </Button>
                </div>

                {/* Rotate key */}
                <div className="flex items-center gap-2">
                  {confirmRotate ? (
                    <>
                      <span className="text-xs text-destructive">
                        This will invalidate your current key. Continue?
                      </span>
                      <Button
                        size="sm"
                        variant="destructive"
                        onClick={() => void rotateApiKey()}
                        disabled={rotatingKey}
                      >
                        {rotatingKey ? (
                          <>
                            <RefreshCw className="h-3.5 w-3.5 mr-1 animate-spin" />
                            Rotating...
                          </>
                        ) : (
                          "Confirm Rotate"
                        )}
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => setConfirmRotate(false)}
                        disabled={rotatingKey}
                      >
                        Cancel
                      </Button>
                    </>
                  ) : (
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => setConfirmRotate(true)}
                    >
                      <RefreshCw className="h-3.5 w-3.5 mr-1" />
                      Rotate Key
                    </Button>
                  )}
                </div>
              </>
            ) : (
              <div className="text-sm text-muted-foreground">
                No API key found. Contact an administrator to provision one.
              </div>
            )}
          </CardContent>
        </Card>

        {/* ── Stats Section ────────────────────────────────── */}
        <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
          <Card>
            <CardContent className="py-4 text-center">
              <p className="text-2xl font-bold font-mono">
                {operator ? operator.credit.toLocaleString() : "-"}
              </p>
              <p className="text-xs text-muted-foreground">Credit</p>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="py-4 text-center">
              <p className="text-2xl font-bold font-mono text-green-500">
                {operator ? operator.primes_found.toLocaleString() : "-"}
              </p>
              <p className="text-xs text-muted-foreground">Primes Found</p>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="py-4 text-center">
              <div className="flex items-center justify-center gap-2 mb-1">
                <Shield className="h-4 w-4 text-muted-foreground" />
                <Badge
                  variant="outline"
                  className={trustBadgeClass(trustLevel)}
                >
                  {trustLabel(trustLevel)}
                </Badge>
              </div>
              <p className="text-xs text-muted-foreground">Trust Level {trustLevel}</p>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="py-4 text-center">
              <p className="text-2xl font-bold font-mono">
                {rank !== null ? `#${rank}` : "-"}
              </p>
              <p className="text-xs text-muted-foreground">Rank</p>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}
