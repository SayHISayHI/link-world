import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { EvaluationRun } from "../../types/api";
import { EvaluationPanel } from "./EvaluationPanel";

const evaluation: EvaluationRun = {
  id: "run-1",
  requestId: "request-1",
  correlationId: "correlation-1",
  retryOfRunId: "parent-run-1234567890",
  objectId: "object-1",
  evaluatorType: "prompt_evaluator",
  evaluatorVersion: "0.1.0",
  planSchemaVersion: 1,
  inputSchemaVersion: 1,
  outputSchemaVersion: 1,
  status: "failed",
  verdict: "unknown",
  dimensions: {},
  evidence: [],
  artifacts: [],
  limitations: ["Execution did not complete."],
  nextActions: [],
  failureReason: "evaluation.timeout",
  createdAt: "2026-07-01T00:00:00Z",
  completedAt: "2026-07-01T00:00:00.005Z",
  trace: {
    id: "trace-1",
    schemaVersion: 1,
    requestId: "request-1",
    correlationId: "correlation-1",
    evaluatorType: "prompt_evaluator",
    evaluatorVersion: "0.1.0",
    executionKind: "local_deterministic",
    inputHash: "1234567890abcdef-input",
    timeoutMs: 5,
    latencyMs: 6,
    status: "failed",
    errorCode: "evaluation.timeout",
  },
};

describe("EvaluationPanel", () => {
  it("labels inference and exposes privacy-bounded execution trace details", () => {
    const onRunEvaluation = vi.fn();
    const { getByText, queryByText } = render(
      <EvaluationPanel
        latestEvaluation={evaluation}
        loading={false}
        onRunEvaluation={onRunEvaluation}
      />,
    );

    expect(getByText("Evaluator inference")).toBeInTheDocument();
    expect(getByText("Retry")).toBeInTheDocument();
    expect(getByText(/Retry of/)).toBeInTheDocument();
    getByText("Retry").click();
    expect(onRunEvaluation).toHaveBeenCalledTimes(1);
    expect(getByText(/evaluation\.timeout/)).toBeInTheDocument();

    getByText("Execution trace").click();

    expect(getByText("6 ms / 5 ms")).toBeInTheDocument();
    expect(getByText("local_deterministic")).toBeInTheDocument();
    expect(getByText("correlation-1")).toBeInTheDocument();
    expect(getByText("1234567890abcdef…")).toBeInTheDocument();
    expect(queryByText(/saved content/i)).toBeNull();
  });
});
