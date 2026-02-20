import { useCallback, useState } from "react";
import Link from "next/link";
import { toast } from "sonner";
import { CheckCircle2, Clock, ExternalLink, RefreshCw, Loader2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { JsonBlock } from "@/components/json-block";
import { API_BASE, numberWithCommas, formatTime } from "@/lib/format";

interface PrimeData {
  id: number;
  form: string;
  expression: string;
  digits: number;
  found_at: string;
  search_params?: string | null;
  proof_method?: string;
  verified?: boolean;
  verified_at?: string | null;
  verification_method?: string | null;
  verification_tier?: number | null;
}

interface PrimeDetailDialogProps {
  prime: PrimeData | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  showVerifyButton?: boolean;
  loading?: boolean;
}

export function PrimeDetailDialog({
  prime,
  open,
  onOpenChange,
  showVerifyButton = false,
  loading = false,
}: PrimeDetailDialogProps) {
  const [verifying, setVerifying] = useState(false);

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

  let parsedSearchParams: Record<string, unknown> | null = null;
  if (prime?.search_params) {
    try {
      parsedSearchParams = JSON.parse(prime.search_params);
    } catch {
      // leave as null
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="font-mono text-primary break-all">
            {loading ? "Loading..." : (prime?.expression ?? "Prime detail")}
          </DialogTitle>
        </DialogHeader>
        {loading && (
          <p className="text-sm text-muted-foreground">Loading prime details...</p>
        )}
        {!loading && prime && (
          <div className="space-y-4">
            <div className="grid grid-cols-2 gap-4 text-sm">
              <div>
                <div className="text-xs font-medium text-muted-foreground mb-1">Form</div>
                <Badge variant="outline">{prime.form}</Badge>
              </div>
              <div>
                <div className="text-xs font-medium text-muted-foreground mb-1">Digits</div>
                <span className="font-semibold">{numberWithCommas(prime.digits)}</span>
              </div>
              {prime.proof_method && (
                <div>
                  <div className="text-xs font-medium text-muted-foreground mb-1">Proof</div>
                  <Badge variant="outline">{prime.proof_method}</Badge>
                </div>
              )}
              {prime.verified !== undefined && (
                <div>
                  <div className="text-xs font-medium text-muted-foreground mb-1">Verification</div>
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
              )}
              <div className="col-span-2">
                <div className="text-xs font-medium text-muted-foreground mb-1">Found at</div>
                <span>{formatTime(prime.found_at)}</span>
              </div>
            </div>
            {prime.verified && prime.verification_method && (
              <div>
                <div className="text-xs font-medium text-muted-foreground mb-1">Verification details</div>
                <div className="bg-muted rounded-md p-3 text-xs space-y-1">
                  <div><span className="text-muted-foreground">Method:</span> {prime.verification_method}</div>
                  {prime.verified_at && (
                    <div><span className="text-muted-foreground">Verified at:</span> {formatTime(prime.verified_at)}</div>
                  )}
                </div>
              </div>
            )}
            {parsedSearchParams && (
              <JsonBlock label="Search parameters" data={parsedSearchParams} maxHeight="max-h-48" />
            )}
            {prime.search_params && !parsedSearchParams && (
              <JsonBlock label="Search parameters" data={prime.search_params} maxHeight="max-h-48" />
            )}
            {showVerifyButton && (
              <div className="flex items-center gap-2 pt-2 border-t">
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
                <Button variant="outline" size="sm" asChild>
                  <Link href={`/prime/?id=${prime.id}`}>
                    <ExternalLink className="size-3.5 mr-1" />
                    Permalink
                  </Link>
                </Button>
              </div>
            )}
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
