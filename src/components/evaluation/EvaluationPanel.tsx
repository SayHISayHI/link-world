import { Gauge, ShieldCheck } from "lucide-react";
import type { EvaluationRun, EvidenceItem } from "../../types/api";
import type { AppUiError } from "../../lib/errors";
import { Button } from "../ui/button";

const EVIDENCE_SOURCE_LABELS: Record<string, string> = {
  original_content: "Saved content",
  internal_library: "Local library",
  external_check: "External check",
  sandbox_run: "Sandbox result",
  user_feedback: "User feedback",
};
interface EvaluationPanelProps {
  latestEvaluation?: EvaluationRun;
  loading: boolean;
  error?: AppUiError;
  onRunEvaluation: () => void;
}

export function EvaluationPanel({
  latestEvaluation,
  loading,
  error,
  onRunEvaluation,
}: EvaluationPanelProps) {
  const dimensions = toDimensionEntries(latestEvaluation?.dimensions);
  const trace = latestEvaluation?.trace;

  return (
    <section>
      <div className="mt-4 flex items-center justify-between gap-3">
        <h3 className="text-sm font-semibold">Evaluation</h3>
        <Button onClick={onRunEvaluation} disabled={loading} className="h-8 text-xs" title="Run evaluation">
          <ShieldCheck className="h-4 w-4" aria-hidden="true" />
          Evaluate
        </Button>
      </div>
      <div className="mt-3 space-y-3 rounded-md border border-border bg-surface p-3 text-xs leading-5">
        {error ? (
          <div className="rounded-md border border-red-200 bg-red-50 p-2 text-red-800">
            <p className="font-medium">{error.title}</p>
            <p>{error.message}</p>
          </div>
        ) : null}
        {latestEvaluation ? (
          <>
            <div className="flex items-start justify-between gap-3">
              <div>
                <div className="flex flex-wrap items-center gap-2">
                  <p className="font-medium">{formatVerdict(latestEvaluation.verdict)}</p>
                  <span className="rounded-full border border-border bg-background px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
                    Evaluator inference
                  </span>
                </div>
                <p className="mt-1 text-muted-foreground">
                  {latestEvaluation.evaluatorType} / {latestEvaluation.status} / contract v
                  {latestEvaluation.outputSchemaVersion}
                </p>
              </div>
              {latestEvaluation.score !== undefined ? (
                <div className="flex items-center gap-1 rounded-sm bg-muted px-2 py-1 text-[11px] text-muted-foreground">
                  <Gauge className="h-3.5 w-3.5" aria-hidden="true" />
                  {formatScore(latestEvaluation.score)}
                </div>
              ) : null}
            </div>
            {latestEvaluation.failureReason ? (
              <div role="status" className="rounded-sm border border-amber-200 bg-amber-50 px-2 py-1 text-amber-900">
                Evaluation stopped with stable code: {latestEvaluation.failureReason}
              </div>
            ) : null}
            {trace ? (
              <details className="rounded-sm border border-border bg-background px-2 py-1.5">
                <summary className="cursor-pointer font-medium">Execution trace</summary>
                <dl className="mt-2 grid grid-cols-2 gap-x-3 gap-y-1 text-muted-foreground">
                  <div>
                    <dt>Status</dt>
                    <dd className="font-medium text-foreground">{trace.status}</dd>
                  </div>
                  <div>
                    <dt>Runtime / limit</dt>
                    <dd className="font-medium text-foreground">
                      {formatDuration(trace.latencyMs)} / {formatDuration(trace.timeoutMs)}
                    </dd>
                  </div>
                  <div>
                    <dt>Executor</dt>
                    <dd className="font-medium text-foreground">{trace.executionKind}</dd>
                  </div>
                  <div>
                    <dt>Trace contract</dt>
                    <dd className="font-medium text-foreground">v{trace.schemaVersion}</dd>
                  </div>
                  <div className="col-span-2">
                    <dt>Correlation</dt>
                    <dd className="break-all font-mono text-[10px] text-foreground">
                      {trace.correlationId}
                    </dd>
                  </div>
                  <div>
                    <dt>Input fingerprint</dt>
                    <dd className="font-mono text-[10px] text-foreground" title={trace.inputHash}>
                      {shortHash(trace.inputHash)}
                    </dd>
                  </div>
                  <div>
                    <dt>Output fingerprint</dt>
                    <dd className="font-mono text-[10px] text-foreground" title={trace.outputHash}>
                      {shortHash(trace.outputHash)}
                    </dd>
                  </div>
                </dl>
              </details>
            ) : null}
            {dimensions.length > 0 ? (
              <div className="grid grid-cols-2 gap-2">
                {dimensions.map(([name, value]) => (
                  <div key={name} className="rounded-sm bg-background px-2 py-1">
                    <p className="truncate text-muted-foreground">{formatDimensionName(name)}</p>
                    <p className="font-medium">{formatScore(value)}</p>
                  </div>
                ))}
              </div>
            ) : null}
            {latestEvaluation.evidence.length > 0 ? (
              <div className="border-t border-border pt-3">
                <p className="font-medium">Evidence</p>
                <ul className="mt-1 space-y-1 text-muted-foreground">
                  {latestEvaluation.evidence.slice(0, 3).map((item, index) => {
                    const evidence = formatEvidenceItem(item);
                    return (
                      <li key={`${evidence.label}-${item.reference ?? index}`} className="flex gap-2">
                        <span className="shrink-0 rounded-sm bg-muted px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide text-foreground">
                          {evidence.label}
                        </span>
                        <span>{evidence.text}</span>
                      </li>
                    );
                  })}
                </ul>
              </div>
            ) : null}
            {latestEvaluation.limitations.length > 0 ? (
              <div className="border-t border-border pt-3 text-muted-foreground">
                <p className="font-medium text-foreground">Limitations</p>
                <p className="mt-1">{latestEvaluation.limitations[0]}</p>
              </div>
            ) : null}
          </>
        ) : (
          <p className="text-muted-foreground">
            {loading ? "Evaluation running..." : "No evaluation run yet."}
          </p>
        )}
      </div>
    </section>
  );
}

function toDimensionEntries(value: unknown): Array<[string, number]> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return [];
  }

  return Object.entries(value)
    .filter((entry): entry is [string, number] => typeof entry[1] === "number")
    .slice(0, 6);
}

function formatVerdict(verdict: string) {
  return verdict
    .split("_")
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
}

function formatDimensionName(name: string) {
  return name.replace(/([A-Z])/g, " $1").replace(/^./, (character) => character.toUpperCase());
}

function formatScore(score: number) {
  return `${Math.round(score * 100)}%`;
}

function formatEvidenceItem(item: EvidenceItem) {
  const label = EVIDENCE_SOURCE_LABELS[item.source] ?? "Unclassified";
  const text = item.reference ? `${item.text} (${item.reference})` : item.text;
  return { label, text };
}
function formatDuration(milliseconds: number | undefined) {
  return milliseconds === undefined ? "Pending" : `${milliseconds} ms`;
}

function shortHash(hash: string | undefined) {
  if (!hash) {
    return "Pending";
  }

  return hash.length > 16 ? `${hash.slice(0, 16)}…` : hash;
}
