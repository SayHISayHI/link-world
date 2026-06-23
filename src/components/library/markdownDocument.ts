import GithubSlugger from "github-slugger";
import { toString } from "mdast-util-to-string";
import type { Root } from "mdast";
import { unified } from "unified";
import { visit } from "unist-util-visit";
import remarkGfm from "remark-gfm";
import remarkParse from "remark-parse";
import type { AIDisplayHintsV1, DocumentDisplayMode } from "../../types/api";

export interface DocumentTocItem {
  id: string;
  depth: 2 | 3 | 4;
  label: string;
}

export interface DocumentAstSummary {
  toc: DocumentTocItem[];
  headingCount: number;
  orderedListItemCount: number;
  codeBlockCount: number;
  codeCharacterRatio: number;
  tableCount: number;
  averageSectionLength: number;
  inferredMode: DocumentDisplayMode;
}

export interface DocumentDisplayResolution {
  mode: DocumentDisplayMode;
  aiApplied: boolean;
}

export interface DocumentRenderPolicy {
  mode: DocumentDisplayMode;
  tocExpanded: boolean;
  collapseLongCode: boolean;
  compactTables: boolean;
}

const markdownParser = unified().use(remarkParse).use(remarkGfm);
const EMPTY_SUMMARY: DocumentAstSummary = {
  toc: [],
  headingCount: 0,
  orderedListItemCount: 0,
  codeBlockCount: 0,
  codeCharacterRatio: 0,
  tableCount: 0,
  averageSectionLength: 0,
  inferredMode: "article",
};

export function analyzeMarkdownDocument(content: string): DocumentAstSummary {
  if (!content.trim()) {
    return EMPTY_SUMMARY;
  }

  const tree = markdownParser.parse(content) as Root;
  const slugger = new GithubSlugger();
  const toc: DocumentTocItem[] = [];
  let headingCount = 0;
  let orderedListItemCount = 0;
  let codeBlockCount = 0;
  let codeCharacters = 0;
  let tableCount = 0;

  visit(tree, "heading", (node) => {
    headingCount += 1;
    if (node.depth >= 2 && node.depth <= 4) {
      const label = toString(node).trim();
      if (label) {
        toc.push({
          id: slugger.slug(label),
          depth: node.depth as DocumentTocItem["depth"],
          label,
        });
      }
    }
  });
  visit(tree, "list", (node) => {
    if (node.ordered) {
      orderedListItemCount += node.children.length;
    }
  });
  visit(tree, "code", (node) => {
    codeBlockCount += 1;
    codeCharacters += node.value.length;
  });
  visit(tree, "table", () => {
    tableCount += 1;
  });

  const totalCharacters = toString(tree).replace(/\s/g, "").length;
  const codeCharacterRatio = codeCharacters / Math.max(totalCharacters, 1);
  const averageSectionLength = calculateAverageSectionLength(tree);
  const inferredMode = inferDisplayMode({
    headingCount,
    orderedListItemCount,
    codeBlockCount,
    codeCharacterRatio,
    tableCount,
    averageSectionLength,
  });

  return {
    toc,
    headingCount,
    orderedListItemCount,
    codeBlockCount,
    codeCharacterRatio,
    tableCount,
    averageSectionLength,
    inferredMode,
  };
}

export function resolveDocumentDisplay(
  summary: DocumentAstSummary,
  displayHints?: AIDisplayHintsV1,
): DocumentDisplayResolution {
  if (
    displayHints?.schemaVersion === 1 &&
    isDocumentDisplayMode(displayHints.mode) &&
    Number.isFinite(displayHints.confidence) &&
    displayHints.confidence >= 0.75 &&
    displayHints.confidence <= 1
  ) {
    return { mode: displayHints.mode, aiApplied: true };
  }

  return { mode: summary.inferredMode, aiApplied: false };
}

export function getDocumentRenderPolicy(mode: DocumentDisplayMode): DocumentRenderPolicy {
  switch (mode) {
    case "tutorial":
      return { mode, tocExpanded: true, collapseLongCode: false, compactTables: false };
    case "reference":
      return { mode, tocExpanded: true, collapseLongCode: true, compactTables: true };
    case "code-heavy":
      return { mode, tocExpanded: false, collapseLongCode: false, compactTables: true };
    default:
      return { mode: "article", tocExpanded: false, collapseLongCode: true, compactTables: false };
  }
}

export function displayModeLabel(mode: DocumentDisplayMode) {
  return mode
    .split("-")
    .map((part) => `${part.charAt(0).toUpperCase()}${part.slice(1)}`)
    .join(" ");
}

function calculateAverageSectionLength(tree: Root) {
  const sections: number[] = [];
  let currentLength = 0;

  for (const node of tree.children) {
    if (node.type === "heading") {
      if (currentLength > 0) {
        sections.push(currentLength);
      }
      currentLength = 0;
      continue;
    }
    currentLength += toString(node).replace(/\s/g, "").length;
  }

  if (currentLength > 0 || sections.length === 0) {
    sections.push(currentLength);
  }

  return sections.reduce((total, length) => total + length, 0) / Math.max(sections.length, 1);
}

function inferDisplayMode(summary: Omit<DocumentAstSummary, "toc" | "inferredMode">): DocumentDisplayMode {
  if (summary.codeCharacterRatio >= 0.3 || summary.codeBlockCount >= 4) {
    return "code-heavy";
  }
  if (summary.tableCount >= 2 || (summary.headingCount >= 6 && summary.averageSectionLength < 600)) {
    return "reference";
  }
  if (summary.headingCount >= 2 && summary.orderedListItemCount >= 4) {
    return "tutorial";
  }
  return "article";
}

function isDocumentDisplayMode(value: string): value is DocumentDisplayMode {
  return value === "article" || value === "tutorial" || value === "reference" || value === "code-heavy";
}
