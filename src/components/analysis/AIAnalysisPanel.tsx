import { AlertTriangle, CheckCircle2, Settings, Sparkles } from "lucide-react";
import type { AIAnalysis } from "../../types/api";
import type { AppUiError } from "../../lib/errors";
import { Button } from "../ui/button";
import { useI18n } from "../../i18n";

interface AIAnalysisPanelProps {
  latestAnalysis?: AIAnalysis;
  runLoading: boolean;
  error?: AppUiError;
  onOpenSettings: () => void;
  onRunAnalysis: () => void;
}

export function AIAnalysisPanel({
  latestAnalysis,
  runLoading,
  error,
  onOpenSettings,
  onRunAnalysis,
}: AIAnalysisPanelProps) {
  const { t } = useI18n();
  return (
    <section className="mt-4 border-t border-border pt-4">
      <div className="flex items-center justify-between gap-3">
        <h3 className="text-sm font-semibold">{t("AI Analysis")}</h3>
        <Button variant="ghost" className="h-8 px-2 text-xs" onClick={onOpenSettings}>
          <Settings className="h-4 w-4" aria-hidden="true" />
          {t("Models")}
        </Button>
      </div>
      <div className="mt-3 space-y-3 text-xs leading-5">
        <Button onClick={onRunAnalysis} disabled={runLoading} className="h-8 w-full text-xs">
          <Sparkles className="h-4 w-4" aria-hidden="true" />
          {runLoading ? t("Running...") : t("Run analysis")}
        </Button>
        {error ? (
          <div className="border-l-2 border-red-400 pl-2 text-red-800 dark:text-red-200">
            <p className="font-medium">{t(error.title)}</p>
            <p>{t(error.message)}</p>
          </div>
        ) : null}
        {latestAnalysis ? (
          <div className="space-y-4 border-t border-border pt-3">
            <div>
              <div className="flex items-center justify-between gap-2">
                <p className="font-medium text-foreground">
                  {latestAnalysis.category ?? t("General analysis")}
                </p>
                <span className="text-[11px] text-muted-foreground">{t("Unverified interpretation")}</span>
              </div>
              <p className="mt-1 text-muted-foreground">
                {latestAnalysis.summary ?? t("Analysis exists without summary.")}
              </p>
            </div>

            <div className="grid grid-cols-2 gap-3 border-y border-border py-3">
              <Metric
                label={t("AI quality estimate")}
                value={
                  latestAnalysis.qualityScore !== undefined
                    ? String(Math.round(latestAnalysis.qualityScore * 10)) + " / 10"
                    : t("Not scored")
                }
              />
              <Metric
                label={t("Confidence")}
                value={
                  latestAnalysis.confidence !== undefined
                    ? String(Math.round(latestAnalysis.confidence * 100)) + "%"
                    : t("Not reported")
                }
              />
            </div>

            <AnalysisList title={t("Key points")} values={latestAnalysis.keyPoints} />
            <AnalysisList
              title={t("Suggested actions")}
              values={latestAnalysis.actionItems}
              icon={CheckCircle2}
            />
            <AnalysisList
              title={t("Risks and limitations")}
              values={latestAnalysis.risks}
              icon={AlertTriangle}
              tone="risk"
            />
            <AnalysisList title={t("Source claims to verify")} values={latestAnalysis.claims} />

            {latestAnalysis.trace ? (
              <p className="border-t border-border pt-3 text-[11px] text-muted-foreground">
                {latestAnalysis.trace.provider} / {latestAnalysis.trace.model}
                {latestAnalysis.trace.promptTemplateVersion
                  ? " / prompt " + latestAnalysis.trace.promptTemplateVersion
                  : ""}
              </p>
            ) : null}
          </div>
        ) : (
          <p className="text-muted-foreground">
            {t("No AI analysis yet. Model credentials are managed globally in Settings.")}
          </p>
        )}
      </div>
    </section>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <p className="text-[11px] text-muted-foreground">{label}</p>
      <p className="mt-0.5 font-medium text-foreground">{value}</p>
    </div>
  );
}

function AnalysisList({
  title,
  values,
  icon: Icon,
  tone = "default",
}: {
  title: string;
  values: unknown[];
  icon?: typeof CheckCircle2;
  tone?: "default" | "risk";
}) {
  const items = values.map(formatAnalysisValue).filter(Boolean);
  if (!items.length) return null;

  return (
    <div>
      <p className="font-medium text-foreground">{title}</p>
      <ul className="mt-1.5 space-y-1.5">
        {items.map((item, index) => (
          <li
            key={title + ":" + String(index)}
            className={
              "flex items-start gap-2 " +
              (tone === "risk" ? "text-red-800 dark:text-red-200" : "text-muted-foreground")
            }
          >
            {Icon ? (
              <Icon className="mt-0.5 h-3.5 w-3.5 shrink-0" aria-hidden="true" />
            ) : (
              <span className="mt-2 h-1 w-1 shrink-0 rounded-full bg-current" aria-hidden="true" />
            )}
            <span>{item}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

function formatAnalysisValue(value: unknown): string {
  if (typeof value === "string") return value.trim();
  if (!value || typeof value !== "object") return "";
  const record = value as Record<string, unknown>;
  for (const key of ["text", "title", "summary", "action", "claim", "risk"]) {
    if (typeof record[key] === "string") return record[key].trim();
  }
  return "";
}