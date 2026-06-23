import type { AIAnalysis } from "../../types/api";

export function selectCurrentDisplayHints(
  documentId: string,
  analyses: Array<Pick<AIAnalysis, "parsedDocumentId" | "displayHints">>,
) {
  return analyses.find((analysis) => analysis.parsedDocumentId === documentId)?.displayHints;
}
