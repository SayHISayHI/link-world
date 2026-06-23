import { describe, expect, it } from "vitest";
import {
  analyzeMarkdownDocument,
  resolveDocumentDisplay,
} from "./markdownDocument";
import { selectCurrentDisplayHints } from "./displayHints";

describe("analyzeMarkdownDocument", () => {
  it("returns a stable empty article summary", () => {
    expect(analyzeMarkdownDocument("   ")).toMatchObject({
      toc: [],
      headingCount: 0,
      inferredMode: "article",
    });
  });

  it("builds stable Chinese and duplicate heading ids", () => {
    const summary = analyzeMarkdownDocument(
      "## 中文标题\n\n内容\n\n## 中文标题\n\n更多内容\n\n### 细节\n\n完成",
    );

    expect(summary.toc).toEqual([
      { id: "中文标题", depth: 2, label: "中文标题" },
      { id: "中文标题-1", depth: 2, label: "中文标题" },
      { id: "细节", depth: 3, label: "细节" },
    ]);
  });

  it.each([
    ["article", "## Overview\n\nA narrative paragraph with useful context."],
    [
      "tutorial",
      "## Prepare\n\n1. One\n2. Two\n3. Three\n4. Four\n\n## Finish\n\nDone.",
    ],
    [
      "reference",
      "| A | B |\n| - | - |\n| 1 | 2 |\n\n| C | D |\n| - | - |\n| 3 | 4 |",
    ],
    [
      "code-heavy",
      "```js\nconst a = 1;\n```\n\n```js\nconst b = 2;\n```\n\n```js\nconst c = 3;\n```\n\n```js\nconst d = 4;\n```",
    ],
  ])("infers the %s display mode", (mode, markdown) => {
    expect(analyzeMarkdownDocument(markdown).inferredMode).toBe(mode);
  });
});

describe("display hint resolution", () => {
  const summary = analyzeMarkdownDocument("## Article\n\nNarrative text.");

  it("uses only high-confidence valid AI hints", () => {
    expect(
      resolveDocumentDisplay(summary, {
        schemaVersion: 1,
        mode: "tutorial",
        confidence: 0.75,
      }),
    ).toEqual({ mode: "tutorial", aiApplied: true });

    expect(
      resolveDocumentDisplay(summary, {
        schemaVersion: 1,
        mode: "reference",
        confidence: 0.74,
      }),
    ).toEqual({ mode: "article", aiApplied: false });
  });

  it("ignores hints produced for a stale parsed document", () => {
    const current = selectCurrentDisplayHints("doc-current", [
      {
        parsedDocumentId: "doc-old",
        displayHints: { schemaVersion: 1, mode: "code-heavy", confidence: 0.99 },
      },
      {
        parsedDocumentId: "doc-current",
        displayHints: { schemaVersion: 1, mode: "reference", confidence: 0.9 },
      },
    ]);

    expect(current?.mode).toBe("reference");
  });

  it("does not reuse an older hint when the latest current analysis has none", () => {
    const current = selectCurrentDisplayHints("doc-current", [
      { parsedDocumentId: "doc-current" },
      {
        parsedDocumentId: "doc-current",
        displayHints: { schemaVersion: 1, mode: "reference", confidence: 0.9 },
      },
    ]);

    expect(current).toBeUndefined();
  });
});
