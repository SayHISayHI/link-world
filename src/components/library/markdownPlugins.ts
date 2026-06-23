import type { Element, Root, Text } from "hast";
import { defaultSchema } from "rehype-sanitize";
import rehypeSanitize from "rehype-sanitize";
import rehypeSlug from "rehype-slug";
import remarkGfm from "remark-gfm";
import type { PluggableList, Transformer } from "unified";
import { visit } from "unist-util-visit";

export type LinkWorldCalloutKind = "note" | "tip" | "important" | "warning" | "caution";

const CALLOUT_PATTERN = /^\s*\[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\]\s*(?:\r?\n)?/i;

export const linkWorldRemarkPlugins: PluggableList = [remarkGfm];
export const linkWorldRehypePlugins: PluggableList = [
  [rehypeSanitize, defaultSchema],
  rehypeSlug,
  rehypeLinkWorldCallouts,
];

export function rehypeLinkWorldCallouts(): Transformer<Root> {
  return (tree) => {
    visit(tree, "element", (node: Element) => {
      if (node.tagName !== "blockquote") {
        return;
      }

      const firstText = findFirstText(node);
      if (!firstText) {
        return;
      }

      const match = CALLOUT_PATTERN.exec(firstText.value);
      if (!match) {
        return;
      }

      const kind = match[1].toLowerCase() as LinkWorldCalloutKind;
      firstText.value = firstText.value.slice(match[0].length);
      node.properties ??= {};
      node.properties["data-lw-callout"] = kind;
    });
  };
}

function findFirstText(element: Element): Text | undefined {
  for (const child of element.children) {
    if (child.type === "text" && child.value.trim()) {
      return child;
    }
    if (child.type === "element") {
      const nested = findFirstText(child);
      if (nested) {
        return nested;
      }
    }
  }
  return undefined;
}
