import { Gauge, ShieldCheck } from "lucide-react";
import type { EvaluationRun } from "../../types/api";
import type { AppUiError } from "../../lib/errors";
import { Button } from "../ui/button";

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
                <p className="font-medium">{formatVerdict(latestEvaluation.verdict)}</p>
                <p className="mt-1 text-muted-foreground">
                  {latestEvaluation.evaluatorType} / {latestEvaluation.status}
                </p>
              </div>
              {latestEvaluation.score !== undefined ? (
                <div className="flex items-center gap-1 rounded-sm bg-muted px-2 py-1 text-[11px] text-muted-foreground">
                  <Gauge className="h-3.5 w-3.5" aria-hidden="true" />
                  {formatScore(latestEvaluation.score)}
                </div>
              ) : null}
            </div>
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
                  {latestEvaluation.evidence.slice(0, 3).map((item, index) => (
                    <li key={index}>{formatEvidenceItem(item)}</li>
                  ))}
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

function formatEvidenceItem(item: unknown) {
  if (!item || typeof item !== "object") {
    return String(item);
  }

  const text = "text" in item ? String(item.text) : "";
  const reference = "reference" in item && item.reference ? String(item.reference) : undefined;

  return reference ? `${text} (${reference})` : text;
}
