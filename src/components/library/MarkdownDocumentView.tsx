import {
  createContext,
  memo,
  useContext,
  useMemo,
  useState,
  type ComponentPropsWithoutRef,
  type ReactNode,
} from "react";
import ReactMarkdown, { type Components, type ExtraProps, type UrlTransform } from "react-markdown";
import { Check, ChevronDown, Clipboard, Link as LinkIcon, Sparkles } from "lucide-react";
import { cn } from "../../lib/cn";
import type { AIDisplayHintsV1 } from "../../types/api";
import {
  analyzeMarkdownDocument,
  displayModeLabel,
  getDocumentRenderPolicy,
  resolveDocumentDisplay,
  type DocumentRenderPolicy,
  type DocumentTocItem,
} from "./markdownDocument";
import {
  nodeTideRehypePlugins,
  nodeTideRemarkPlugins,
  type NodeTideCalloutKind,
} from "./markdownPlugins";

interface MarkdownDocumentViewProps {
  documentId: string;
  markdown?: string;
  text: string;
  sourceUrl?: string;
  displayHints?: AIDisplayHintsV1;
}

const DEFAULT_RENDER_POLICY = getDocumentRenderPolicy("article");
const DocumentRenderContext = createContext<DocumentRenderPolicy>(DEFAULT_RENDER_POLICY);
const CALLOUT_STYLES: Record<NodeTideCalloutKind, { label: string; className: string }> = {
  note: { label: "Note", className: "border-blue-500 bg-blue-50 text-blue-950" },
  tip: { label: "Tip", className: "border-emerald-500 bg-emerald-50 text-emerald-950" },
  important: { label: "Important", className: "border-violet-500 bg-violet-50 text-violet-950" },
  warning: { label: "Warning", className: "border-amber-500 bg-amber-50 text-amber-950" },
  caution: { label: "Caution", className: "border-red-500 bg-red-50 text-red-950" },
};
const TOC_INDENT: Record<DocumentTocItem["depth"], string> = {
  2: "pl-0",
  3: "pl-4",
  4: "pl-8",
};

function Heading({
  level,
  id,
  children,
}: {
  level: 1 | 2 | 3 | 4;
  id?: string;
  children: ReactNode;
}) {
  const Tag = `h${level}` as const;
  const sizeClass = {
    1: "mt-8 text-2xl",
    2: "mt-8 text-xl",
    3: "mt-6 text-lg",
    4: "mt-5 text-base",
  }[level];

  return (
    <Tag id={id} className={cn("group scroll-mt-5 font-semibold leading-tight first:mt-0", sizeClass)}>
      {children}
      {id ? (
        <a
          href={`#${id}`}
          aria-label="Copy link to this section"
          className="ml-2 inline-flex align-middle text-muted-foreground opacity-0 transition-opacity hover:text-accent group-hover:opacity-100 focus:opacity-100"
        >
          <LinkIcon className="h-3.5 w-3.5" aria-hidden="true" />
        </a>
      ) : null}
    </Tag>
  );
}

function CodeBlock({ children }: { children: ReactNode }) {
  const policy = useContext(DocumentRenderContext);
  const code = extractReactText(children).replace(/\n$/, "");
  const isLong = code.split("\n").length > 40;
  const [expanded, setExpanded] = useState(!isLong || !policy.collapseLongCode);
  const [copyState, setCopyState] = useState<"idle" | "copied" | "error">("idle");

  const handleCopy = async () => {
    setCopyState((await copyTextToClipboard(code)) ? "copied" : "error");
  };

  return (
    <div className="group/code relative my-5 overflow-hidden rounded-lg border border-slate-800 bg-slate-950 shadow-sm">
      <div className="flex h-9 items-center justify-end gap-2 border-b border-slate-800 px-2 text-xs text-slate-300">
        {isLong ? (
          <button
            type="button"
            className="rounded px-2 py-1 hover:bg-slate-800 focus:outline-none focus:ring-2 focus:ring-accent"
            onClick={() => setExpanded((value) => !value)}
            aria-expanded={expanded}
          >
            {expanded ? "Collapse" : `Expand ${code.split("\n").length} lines`}
          </button>
        ) : null}
        <button
          type="button"
          className="inline-flex items-center gap-1 rounded px-2 py-1 hover:bg-slate-800 focus:outline-none focus:ring-2 focus:ring-accent"
          onClick={handleCopy}
        >
          {copyState === "copied" ? <Check className="h-3.5 w-3.5" aria-hidden="true" /> : <Clipboard className="h-3.5 w-3.5" aria-hidden="true" />}
          {copyState === "copied" ? "Copied" : copyState === "error" ? "Copy failed" : "Copy"}
        </button>
      </div>
      <div className={cn("relative", !expanded && "max-h-80 overflow-hidden")}>
        <pre className="overflow-x-auto p-4 text-[13px] leading-6 text-slate-100">{children}</pre>
        {!expanded ? <div className="pointer-events-none absolute inset-x-0 bottom-0 h-20 bg-gradient-to-t from-slate-950" /> : null}
      </div>
    </div>
  );
}

function Callout({ node, children }: ComponentPropsWithoutRef<"blockquote"> & ExtraProps) {
  const property = node?.properties?.["data-lw-callout"];
  const kind = typeof property === "string" && property in CALLOUT_STYLES ? (property as NodeTideCalloutKind) : undefined;

  if (!kind) {
    return (
      <blockquote className="my-5 rounded-r-md border-l-4 border-accent/50 bg-muted/70 px-4 py-1 text-foreground">
        {children}
      </blockquote>
    );
  }

  const style = CALLOUT_STYLES[kind];
  return (
    <aside data-callout-kind={kind} className={cn("my-5 rounded-r-md border-l-4 px-4 py-3", style.className)}>
      <p className="mb-1 text-xs font-semibold uppercase tracking-wide">{style.label}</p>
      {children}
    </aside>
  );
}

function MarkdownTable({ children }: { children?: ReactNode }) {
  const policy = useContext(DocumentRenderContext);
  return (
    <div className="my-5 overflow-x-auto rounded-lg border border-border">
      <table
        data-density={policy.compactTables ? "compact" : "comfortable"}
        className={cn("w-full min-w-[520px] border-collapse text-left text-sm", policy.compactTables && "text-xs")}
      >
        {children}
      </table>
    </div>
  );
}

function DocumentToc({ items, defaultOpen }: { items: DocumentTocItem[]; defaultOpen: boolean }) {
  return (
    <details className="mb-6 rounded-lg border border-border bg-muted/40 p-4" open={defaultOpen}>
      <summary className="flex cursor-pointer list-none items-center justify-between text-sm font-semibold">
        On this page
        <ChevronDown className="h-4 w-4 text-muted-foreground" aria-hidden="true" />
      </summary>
      <nav aria-label="Table of contents" className="mt-3 border-t border-border pt-3">
        <ol className="space-y-1.5 text-sm text-muted-foreground">
          {items.map((item) => (
            <li key={item.id} className={TOC_INDENT[item.depth]}>
              <a className="hover:text-accent hover:underline" href={`#${item.id}`}>
                {item.label}
              </a>
            </li>
          ))}
        </ol>
      </nav>
    </details>
  );
}

const components: Components = {
  h1: ({ id, children }) => <Heading level={1} id={id}>{children}</Heading>,
  h2: ({ id, children }) => <Heading level={2} id={id}>{children}</Heading>,
  h3: ({ id, children }) => <Heading level={3} id={id}>{children}</Heading>,
  h4: ({ id, children }) => <Heading level={4} id={id}>{children}</Heading>,
  p: ({ children }) => <p className="my-4 text-[15px] leading-7 text-foreground">{children}</p>,
  ul: ({ children }) => <ul className="my-4 list-disc space-y-2 pl-6 text-[15px] leading-7">{children}</ul>,
  ol: ({ children }) => <ol className="my-4 list-decimal space-y-2 pl-6 text-[15px] leading-7">{children}</ol>,
  li: ({ children }) => <li className="pl-1 marker:text-muted-foreground">{children}</li>,
  blockquote: Callout,
  pre: ({ children }) => <CodeBlock>{children}</CodeBlock>,
  code: ({ className, children, node: _node, ...props }) => (
    <code className={cn("rounded bg-muted px-1.5 py-0.5 font-mono text-[0.9em] text-foreground", className)} {...props}>
      {children}
    </code>
  ),
  table: MarkdownTable,
  thead: ({ children }) => <thead className="bg-muted text-foreground">{children}</thead>,
  th: ({ children }) => <th className="border-b border-border px-4 py-3 font-semibold">{children}</th>,
  td: ({ children }) => <td className="border-b border-border px-4 py-3 align-top last:border-r-0">{children}</td>,
  a: ({ children, href }) => {
    const isInternalAnchor = href?.startsWith("#") ?? false;
    return (
      <a
        href={href}
        target={isInternalAnchor ? undefined : "_blank"}
        rel={isInternalAnchor ? undefined : "noreferrer noopener"}
        className="font-medium text-accent underline decoration-accent/30 underline-offset-4 hover:decoration-accent"
      >
        {children}
      </a>
    );
  },
  img: ({ alt, src }) => (
    <img
      src={src}
      alt={alt ?? ""}
      loading="lazy"
      decoding="async"
      referrerPolicy="no-referrer"
      className="my-5 max-h-[70vh] max-w-full rounded-lg border border-border object-contain"
    />
  ),
  hr: () => <hr className="my-8 border-border" />,
  input: ({ node: _node, ...props }) => <input {...props} disabled className="mr-2 accent-accent" />,
};

export const MarkdownDocumentView = memo(function MarkdownDocumentView({
  documentId,
  markdown,
  text,
  sourceUrl,
  displayHints,
}: MarkdownDocumentViewProps) {
  const markdownContent = markdown?.trim();
  const content = markdownContent || text.trim();
  const summary = useMemo(() => analyzeMarkdownDocument(markdownContent ?? ""), [markdownContent]);
  const resolution = useMemo(() => resolveDocumentDisplay(summary, displayHints), [displayHints, summary]);
  const policy = useMemo(() => getDocumentRenderPolicy(resolution.mode), [resolution.mode]);
  const urlTransform = useMemo(() => createSafeUrlTransform(sourceUrl), [sourceUrl]);

  if (!markdownContent) {
    return <p className="mt-4 whitespace-pre-wrap text-[15px] leading-7 text-foreground">{content}</p>;
  }

  return (
    <DocumentRenderContext.Provider value={policy}>
      <div className="document-markdown mt-4 min-w-0 text-foreground" data-document-id={documentId} data-display-mode={policy.mode}>
        {resolution.aiApplied ? (
          <div
            className="mb-4 inline-flex items-center gap-1.5 rounded-full border border-violet-200 bg-violet-50 px-2.5 py-1 text-xs font-medium text-violet-800"
            title={displayHints?.reason}
          >
            <Sparkles className="h-3.5 w-3.5" aria-hidden="true" />
            AI layout · {displayModeLabel(policy.mode)}
          </div>
        ) : null}
        {summary.toc.length >= 3 ? <DocumentToc items={summary.toc} defaultOpen={policy.tocExpanded} /> : null}
        <ReactMarkdown
          key={`${documentId}:${policy.mode}`}
          remarkPlugins={nodeTideRemarkPlugins}
          rehypePlugins={nodeTideRehypePlugins}
          components={components}
          skipHtml
          urlTransform={urlTransform}
        >
          {content}
        </ReactMarkdown>
      </div>
    </DocumentRenderContext.Provider>
  );
});

function createSafeUrlTransform(sourceUrl?: string): UrlTransform {
  return (value, key) => {
    const normalized = value.trim();
    if (key === "href" && normalized.startsWith("#")) {
      return normalized;
    }

    try {
      const url = sourceUrl ? new URL(normalized, sourceUrl) : new URL(normalized);
      if (url.protocol === "http:" || url.protocol === "https:") {
        return url.href;
      }
      if (key === "href" && url.protocol === "mailto:") {
        return url.href;
      }
    } catch {
      return "";
    }

    return "";
  };
}

function extractReactText(value: ReactNode): string {
  if (typeof value === "string" || typeof value === "number") {
    return String(value);
  }
  if (Array.isArray(value)) {
    return value.map(extractReactText).join("");
  }
  if (value && typeof value === "object" && "props" in value) {
    return extractReactText((value as { props?: { children?: ReactNode } }).props?.children);
  }
  return "";
}

async function copyTextToClipboard(value: string) {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(value);
      return true;
    }
  } catch {
    // Fall through to the compatibility path used by restricted webviews.
  }

  const textarea = document.createElement("textarea");
  textarea.value = value;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.select();

  try {
    return document.execCommand("copy");
  } catch {
    return false;
  } finally {
    textarea.remove();
  }
}
