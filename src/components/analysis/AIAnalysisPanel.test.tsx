import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AIAnalysisPanel } from "./AIAnalysisPanel";

function renderPanel(overrides: Partial<React.ComponentProps<typeof AIAnalysisPanel>> = {}) {
  const props: React.ComponentProps<typeof AIAnalysisPanel> = {
    runLoading: false,
    onOpenSettings: vi.fn(),
    onRunAnalysis: vi.fn(),
    ...overrides,
  };

  render(<AIAnalysisPanel {...props} />);
  return props;
}

describe("AIAnalysisPanel", () => {
  it("keeps provider credentials out of object context and links to settings", () => {
    const props = renderPanel();

    expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Provider")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Models" }));
    expect(props.onOpenSettings).toHaveBeenCalledOnce();
  });

  it("runs analysis and renders only trace metadata", () => {
    const onRunAnalysis = vi.fn();
    renderPanel({
      onRunAnalysis,
      latestAnalysis: {
        id: "analysis-1",
        objectId: "object-1",
        analysisType: "general_summary",
        schemaVersion: 2,
        summary: "Useful summary.",
        tags: [],
        keyPoints: [],
        claims: [],
        actionItems: [],
        risks: [],
        trace: {
          provider: "anthropic",
          model: "claude-sonnet",
          capability: "chat",
        },
        createdAt: "2026-06-23T00:00:00Z",
      },
    });

    fireEvent.click(screen.getByRole("button", { name: "Run analysis" }));
    expect(onRunAnalysis).toHaveBeenCalledOnce();
    expect(screen.getByText("anthropic / claude-sonnet")).toBeInTheDocument();
    expect(screen.queryByText(/API key/i)).not.toBeInTheDocument();
  });
});
