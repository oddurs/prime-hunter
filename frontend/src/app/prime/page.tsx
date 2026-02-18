"use client";

/**
 * @module prime/page
 *
 * Individual prime detail page. Accessed via `/prime?id=<N>`. Shows
 * the full expression, digit count, form, proof method, discovery
 * timestamp, search parameters, and verification status. Provides a
 * "Verify" button that triggers independent re-verification via the
 * Rust backend's `/api/verify` endpoint.
 */

import { useEffect, useState, useCallback } from "react";
import { useSearchParams } from "next/navigation";
import Link from "next/link";
import { supabase } from "@/lib/supabase";
import { API_BASE, numberWithCommas, formatTime } from "@/lib/format";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { toast } from "sonner";
import {
  CheckCircle2,
  Clock,
  ArrowLeft,
  Copy,
  RefreshCw,
  Loader2,
} from "lucide-react";

interface PrimeData {
  id: number;
  form: string;
  expression: string;
  digits: number;
  found_at: string;
  search_params: string;
  proof_method: string;
  verified: boolean;
  verified_at: string | null;
  verification_method: string | null;
  verification_tier: number | null;
}

export default function PrimePage() {
  const searchParams = useSearchParams();
  const idParam = searchParams.get("id");
  const [prime, setPrime] = useState<PrimeData | null>(null);
  const [loading, setLoading] = useState(true);
  const [notFound, setNotFound] = useState(false);
  const [verifying, setVerifying] = useState(false);

  useEffect(() => {
    if (!idParam) {
      setLoading(false);
      setNotFound(true);
      return;
    }
    const id = Number(idParam);
    if (!Number.isInteger(id) || id <= 0) {
      setLoading(false);
      setNotFound(true);
      return;
    }

    async function fetchPrime() {
      const { data, error } = await supabase
        .from("primes")
        .select(
          "id, form, expression, digits, found_at, search_params, proof_method, verified, verified_at, verification_method, verification_tier"
        )
        .eq("id", id)
        .single();

      if (error || !data) {
        setNotFound(true);
      } else {
        setPrime(data as PrimeData);
      }
      setLoading(false);
    }

    fetchPrime();
  }, [idParam]);

  const handleVerify = useCallback(async () => {
    if (!prime) return;
    setVerifying(true);
    try {
      const res = await fetch(
        `${API_BASE}/api/primes/${prime.id}/verify`,
        { method: "POST" }
      );
      const data = await res.json();
      if (!res.ok) {
        throw new Error(data.error || `HTTP ${res.status}`);
      }
      if (data.result === "verified") {
        toast.success(`Verified: ${data.method} (Tier ${data.tier})`);
        setPrime((prev) =>
          prev
            ? {
                ...prev,
                verified: true,
                verification_method: data.method,
                verification_tier: data.tier,
                verified_at: new Date().toISOString(),
              }
            : prev
        );
      } else if (data.result === "failed") {
        toast.error(`Verification failed: ${data.reason}`);
      } else if (data.result === "skipped") {
        toast.info(`Verification skipped: ${data.reason}`);
      }
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Verification request failed";
      toast.error(message);
    } finally {
      setVerifying(false);
    }
  }, [prime]);

  function copyPermalink() {
    if (!prime) return;
    const url = `${window.location.origin}/prime/?id=${prime.id}`;
    navigator.clipboard.writeText(url);
    toast.success("Link copied to clipboard");
  }

  let parsedSearchParams: Record<string, unknown> | null = null;
  if (prime?.search_params) {
    try {
      parsedSearchParams = JSON.parse(prime.search_params);
    } catch {
      // leave as null
    }
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <Loader2 className="size-5 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (notFound || !prime) {
    return (
      <div className="py-20 text-center">
        <h1 className="text-xl font-semibold text-foreground mb-2">
          Prime not found
        </h1>
        <p className="text-sm text-muted-foreground mb-4">
          {idParam
            ? `No prime with ID ${idParam} exists.`
            : "No prime ID specified."}
        </p>
        <Button variant="outline" asChild>
          <Link href="/browse">Browse primes</Link>
        </Button>
      </div>
    );
  }

  return (
    <>
      <div className="flex items-center gap-3 mb-6">
        <Button variant="ghost" size="sm" asChild>
          <Link href="/browse">
            <ArrowLeft className="size-4 mr-1" />
            Browse
          </Link>
        </Button>
      </div>

      <Card className="mb-6">
        <CardHeader className="pb-3">
          <div className="flex items-start justify-between gap-4">
            <div className="min-w-0">
              <CardTitle className="font-mono text-primary text-lg break-all">
                {prime.expression}
              </CardTitle>
              <p className="text-sm text-muted-foreground mt-1">
                Prime #{prime.id}
              </p>
            </div>
            <div className="flex items-center gap-2 flex-shrink-0">
              <Button variant="outline" size="sm" onClick={copyPermalink}>
                <Copy className="size-3.5 mr-1" />
                Copy link
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={handleVerify}
                disabled={verifying}
              >
                {verifying ? (
                  <Loader2 className="size-3.5 mr-1 animate-spin" />
                ) : (
                  <RefreshCw className="size-3.5 mr-1" />
                )}
                {verifying ? "Verifying..." : "Re-verify"}
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-2 md:grid-cols-4 gap-6 text-sm">
            <div>
              <div className="text-xs font-medium text-muted-foreground mb-1">
                Form
              </div>
              <Badge variant="outline">{prime.form}</Badge>
            </div>
            <div>
              <div className="text-xs font-medium text-muted-foreground mb-1">
                Digits
              </div>
              <span className="font-semibold">
                {numberWithCommas(prime.digits)}
              </span>
            </div>
            <div>
              <div className="text-xs font-medium text-muted-foreground mb-1">
                Proof method
              </div>
              <Badge variant="outline">{prime.proof_method}</Badge>
            </div>
            <div>
              <div className="text-xs font-medium text-muted-foreground mb-1">
                Verification
              </div>
              {prime.verified ? (
                <span className="inline-flex items-center gap-1 text-green-600 dark:text-green-400 font-medium">
                  <CheckCircle2 className="size-3.5" />
                  Tier {prime.verification_tier}
                </span>
              ) : (
                <span className="inline-flex items-center gap-1 text-muted-foreground">
                  <Clock className="size-3.5" />
                  Pending
                </span>
              )}
            </div>
            <div className="col-span-2">
              <div className="text-xs font-medium text-muted-foreground mb-1">
                Discovered
              </div>
              <span>{formatTime(prime.found_at)}</span>
            </div>
            {prime.verified && prime.verification_method && (
              <div className="col-span-2">
                <div className="text-xs font-medium text-muted-foreground mb-1">
                  Verification details
                </div>
                <div className="bg-muted rounded-md p-3 text-xs space-y-1">
                  <div>
                    <span className="text-muted-foreground">Method:</span>{" "}
                    {prime.verification_method}
                  </div>
                  {prime.verified_at && (
                    <div>
                      <span className="text-muted-foreground">
                        Verified at:
                      </span>{" "}
                      {formatTime(prime.verified_at)}
                    </div>
                  )}
                </div>
              </div>
            )}
          </div>

          {parsedSearchParams && (
            <div className="mt-6">
              <div className="text-xs font-medium text-muted-foreground mb-1">
                Search parameters
              </div>
              <pre className="bg-muted rounded-md p-3 text-xs overflow-auto max-h-48">
                {JSON.stringify(parsedSearchParams, null, 2)}
              </pre>
            </div>
          )}
          {prime.search_params && !parsedSearchParams && (
            <div className="mt-6">
              <div className="text-xs font-medium text-muted-foreground mb-1">
                Search parameters
              </div>
              <pre className="bg-muted rounded-md p-3 text-xs overflow-auto max-h-48">
                {prime.search_params}
              </pre>
            </div>
          )}
        </CardContent>
      </Card>
    </>
  );
}
