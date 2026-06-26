import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { LocalMetricsSnapshot } from "../../types/api";
import { DiagnosticsSettings } from "./DiagnosticsSettings";

const mocks = vi.hoisted(() => ({
  loadSnapshot: vi.fn(),
  retryBackgroundJob: vi.fn(),
}));

vi.mock("../../hooks/commands/useLocalMetricsSnapshot", () => ({
  useLocalMetricsSnapshot: () => ({
    data: snapshot,
    error: undefined,
    loading: false,
    loadSnapshot: mocks.loadSnapshot,
  }),
}));

vi.mock("../../hooks/commands/useRetryBackgroundJob", () => ({
  useRetryBackgroundJob: () => ({
    error: undefined,
    loading: false,
    retryBackgroundJob: mocks.retryBackgroundJob,
  }),
}));

const snapshot: LocalMetricsSnapshot = {
  appVersion: "0.1.0",
  dataDir: "C:\\Users\\tester\\AppData\\LinkWorld",
  databasePath: "C:\\Users\\tester\\AppData\\LinkWorld\\link-world.sqlite3",
  objectStorePath: "C:\\Users\\tester\\AppData\\LinkWorld\\objects",
  databaseHealth: {
    healthy: true,
    quickCheck: "ok",
    foreignKeyViolations: 0,
    appliedMigrationVersion: 3,
    sizeBytes: 2048,
  },
  objectStoreHealth: {
    healthy: true,
    sizeBytes: 4096,
    fileCount: 2,
  },
  jobs: {
    queued: 1,
    running: 0,
    failed: 1,
    blocked: 0,
    cancelled: 0,
    recentFailures: [
      {
        jobId: "job-failed",
        jobType: "capture.fetch_url",
        status: "failed",
        objectId: "obj-failed",
        lastError: "capture failed for https://example.com/a[redacted] using [credential-reference]",
        updatedAt: "2026-06-26T00:00:00Z",
      },
    ],
  },
  models: {
    configuredCount: 0,
    enabledCount: 0,
    defaultChatConfigured: false,
    status: "not_configured_normal_degradation",
  },
  privacy: {
    supportBundleAvailable: false,
    redaction: [
      "No source snapshots or parsed document content are included.",
      "Model credential references and API keys are not returned.",
    ],
  },
};

describe("DiagnosticsSettings", () => {
  beforeEach(() => {
    mocks.loadSnapshot.mockReset();
    mocks.retryBackgroundJob.mockReset();
    mocks.retryBackgroundJob.mockResolvedValue(true);
  });

  it("renders local health and normal AI degradation without leaking redacted details", () => {
    render(<DiagnosticsSettings />);

    expect(screen.getByText("Diagnostics")).toBeInTheDocument();
    expect(screen.getByText("Healthy")).toBeInTheDocument();
    expect(screen.getByText("Not configured - normal degradation")).toBeInTheDocument();
    expect(screen.getByText(/AI features are degraded by design/)).toBeInTheDocument();
    expect(screen.getByText(/https:\/\/example.com\/a\[redacted\]/)).toBeInTheDocument();
    expect(screen.queryByText(/secret=1/)).not.toBeInTheDocument();
    expect(screen.queryByText(/keyring:model-provider/)).not.toBeInTheDocument();
  });

  it("opens object and retries capture jobs from failed job summaries", async () => {
    const onOpenObject = vi.fn();
    render(<DiagnosticsSettings onOpenObject={onOpenObject} />);

    fireEvent.click(screen.getByRole("button", { name: "Open object" }));
    expect(onOpenObject).toHaveBeenCalledWith("obj-failed");

    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(mocks.retryBackgroundJob).toHaveBeenCalledWith({ jobId: "job-failed" });
  });
});
