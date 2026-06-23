import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { MarkdownDocumentView } from "./MarkdownDocumentView";

describe("MarkdownDocumentView", () => {
  const writeText = vi.fn<(_: string) => Promise<void>>().mockResolvedValue(undefined);

  beforeEach(() => {
    writeText.mockClear();
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
  });

  it("renders the fixed AST pipeline, table of contents and AI layout", () => {
    render(
      <MarkdownDocumentView
        documentId="doc-1"
        sourceUrl="https://example.com/guide/start"
        text="fallback"
        displayHints={{
          schemaVersion: 1,
          mode: "reference",
          confidence: 0.9,
          reason: "Lookup-oriented content",
        }}
        markdown={[
          "## Setup",
          "",
          "> [!WARNING]",
          "> Keep a backup.",
          "",
          "## Usage",
          "",
          "Read the [next page](../next).",
          "",
          "### Results",
          "",
          "| Item | Value |",
          "| --- | --- |",
          "| Mode | Safe |",
        ].join("\n")}
      />,
    );

    expect(screen.getByText("AI layout · Reference")).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Table of contents" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /Setup/ })).toHaveAttribute("id", "setup");
    expect(screen.getByText("Warning").closest("aside")).toHaveAttribute("data-callout-kind", "warning");
    expect(screen.queryByText("[!WARNING]")).not.toBeInTheDocument();
    expect(screen.getByRole("table")).toHaveAttribute("data-density", "compact");
    expect(screen.getByRole("link", { name: "next page" })).toHaveAttribute(
      "href",
      "https://example.com/next",
    );
  });

  it("copies code and expands long code blocks", async () => {
    const code = Array.from({ length: 41 }, (_, index) => `line ${index + 1}`).join("\n");
    render(
      <MarkdownDocumentView
        documentId="doc-code"
        text="fallback"
        displayHints={{ schemaVersion: 1, mode: "article", confidence: 0.9 }}
        markdown={`## Code\n\n\`\`\`text\n${code}\n\`\`\``}
      />,
    );

    const expand = screen.getByRole("button", { name: "Expand 41 lines" });
    expect(expand).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(expand);
    expect(screen.getByRole("button", { name: "Collapse" })).toHaveAttribute("aria-expanded", "true");

    fireEvent.click(screen.getByRole("button", { name: "Copy" }));
    expect(writeText).toHaveBeenCalledWith(code);
    expect(await screen.findByRole("button", { name: "Copied" })).toBeInTheDocument();
  });

  it("falls back to the compatibility copy path", async () => {
    writeText.mockRejectedValueOnce(new Error("permission denied"));
    const execCommand = vi.fn().mockReturnValue(true);
    Object.defineProperty(document, "execCommand", { configurable: true, value: execCommand });
    render(
      <MarkdownDocumentView
        documentId="doc-copy-fallback"
        text="fallback"
        markdown={"```text\ncopy me\n```"}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Copy" }));
    expect(await screen.findByRole("button", { name: "Copied" })).toBeInTheDocument();
    expect(execCommand).toHaveBeenCalledWith("copy");
  });

  it("blocks raw HTML and unsafe URL protocols", () => {
    const { container } = render(
      <MarkdownDocumentView
        documentId="doc-safe"
        text="fallback"
        markdown={'<script>alert(1)</script>\n\n[bad](javascript:alert(1))\n\n![pixel](data:image/png;base64,AAAA)'}
      />,
    );

    expect(container.querySelector("script")).not.toBeInTheDocument();
    expect(screen.getByText("bad").closest("a")?.getAttribute("href") ?? "").not.toContain("javascript:");
    expect(container.querySelector('img[src^="data:"]')).not.toBeInTheDocument();
  });

  it("renders plain text fallback without interpreting Markdown", () => {
    render(
      <MarkdownDocumentView
        documentId="doc-text"
        text="Literal **asterisks** and <unsafe> text"
      />,
    );

    expect(screen.getByText("Literal **asterisks** and <unsafe> text")).toBeInTheDocument();
    expect(screen.queryByRole("strong")).not.toBeInTheDocument();
  });

  it("keeps remote images private and lazy", () => {
    const { container } = render(
      <MarkdownDocumentView
        documentId="doc-image"
        sourceUrl="https://example.com/article"
        text="fallback"
        markdown="![diagram](/assets/diagram.png)"
      />,
    );
    const image = container.querySelector("img");

    expect(image).toHaveAttribute("src", "https://example.com/assets/diagram.png");
    expect(image).toHaveAttribute("loading", "lazy");
    expect(image).toHaveAttribute("decoding", "async");
    expect(image).toHaveAttribute("referrerpolicy", "no-referrer");
  });
});
