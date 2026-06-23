import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AIAnalysisPanel } from "./AIAnalysisPanel";

function renderPanel(overrides: Partial<React.ComponentProps<typeof AIAnalysisPanel>> = {}) {
  const props: React.ComponentProps<typeof AIAnalysisPanel> = {
    provider: "openai",
    apiFamily: "openai_chat_completions",
    chatBaseUrl: "https://api.openai.com/v1",
    chatModel: "gpt-4.1-mini",
    apiKey: "",
    hasApiKey: true,
    configLoading: false,
    testLoading: false,
    runLoading: false,
    onProviderChange: vi.fn(),
    onApiFamilyChange: vi.fn(),
    onChatBaseUrlChange: vi.fn(),
    onChatModelChange: vi.fn(),
    onApiKeyChange: vi.fn(),
    onSaveConfig: vi.fn(),
    onTestConfig: vi.fn(),
    onRunAnalysis: vi.fn(),
    ...overrides,
  };

  render(<AIAnalysisPanel {...props} />);
  return props;
}

describe("AIAnalysisPanel", () => {
  it("exposes provider and protocol without revealing the stored API key", () => {
    const props = renderPanel();

    expect(screen.getByLabelText("Provider")).toHaveValue("openai");
    expect(screen.getByLabelText("API Protocol")).toHaveValue("openai_chat_completions");
    expect(screen.getByLabelText("API Key")).toHaveValue("");
    expect(screen.getByLabelText("API Key")).toHaveAttribute(
      "placeholder",
      "Configured — leave blank to keep it",
    );

    fireEvent.change(screen.getByLabelText("API Protocol"), {
      target: { value: "anthropic_messages" },
    });
    expect(props.onApiFamilyChange).toHaveBeenCalledWith("anthropic_messages");
  });

  it("runs a connection test and renders only redacted result metadata", () => {
    const onTestConfig = vi.fn();
    renderPanel({
      onTestConfig,
      testResult: {
        provider: "anthropic",
        apiFamily: "anthropic_messages",
        model: "test-model",
        latencyMs: 42,
      },
    });

    fireEvent.click(screen.getByRole("button", { name: "Test" }));
    expect(onTestConfig).toHaveBeenCalledOnce();
    expect(screen.getByText("Connected to anthropic / test-model in 42 ms.")).toBeInTheDocument();
  });
});
