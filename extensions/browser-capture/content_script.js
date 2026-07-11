const NODE_TIDE_MAX_SELECTED_TEXT_BYTES = 60 * 1024;
const NODE_TIDE_MAX_TEXT_BYTES = 80 * 1024;
const NODE_TIDE_MAX_HTML_BYTES = 280 * 1024;
const NODE_TIDE_MAX_PAYLOAD_BYTES = 480 * 1024;

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (message?.type !== "NODE_TIDE_COLLECT_CURRENT_PAGE") {
    return false;
  }

  try {
    sendResponse({ ok: true, payload: collectCurrentPage() });
  } catch (error) {
    sendResponse({
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    });
  }

  return true;
});

function collectCurrentPage() {
  const root = findCaptureRoot();
  const clone = root.cloneNode(true);
  sanitizeClone(clone);

  return fitPayloadToBudget({
    url: window.location.href,
    canonicalUrl: extractCanonicalUrl(),
    title: extractTitle(root),
    author: extractAuthor(),
    description: extractDescription(),
    publishedAt: extractPublishedAt(),
    language: document.documentElement.lang || undefined,
    selectedText: truncateUtf8(
      normalizeText(String(window.getSelection?.() ?? "")),
      NODE_TIDE_MAX_SELECTED_TEXT_BYTES,
    ),
    domText: truncateUtf8(normalizeText(clone.textContent ?? ""), NODE_TIDE_MAX_TEXT_BYTES),
    domHtml: truncateUtf8(clone.outerHTML, NODE_TIDE_MAX_HTML_BYTES),
    sourcePlatform: window.location.hostname,
    capturedAt: new Date().toISOString(),
  });
}

function findCaptureRoot() {
  return (
    document.querySelector('[itemprop="articleBody"]') ??
    document.querySelector("article") ??
    document.querySelector("main") ??
    document.querySelector('[role="main"]') ??
    document.body ??
    document.documentElement
  );
}

function sanitizeClone(root) {
  const blockedSelector = [
    "script",
    "style",
    "noscript",
    "template",
    "iframe",
    "canvas",
    "svg",
    "video",
    "audio",
    "form",
    "input",
    "button",
    "nav",
    "footer",
    "[hidden]",
    '[aria-hidden="true"]',
  ].join(",");

  root.querySelectorAll(blockedSelector).forEach((node) => node.remove());
  root.querySelectorAll("*").forEach((node) => {
    for (const attribute of [...node.attributes]) {
      const name = attribute.name.toLowerCase();
      if (name.startsWith("on") || name === "style" || name === "contenteditable") {
        node.removeAttribute(attribute.name);
      }
    }

    for (const attributeName of ["href", "src"]) {
      const value = node.getAttribute(attributeName);
      if (value && /^\s*javascript:/i.test(value)) {
        node.removeAttribute(attributeName);
      }
    }
  });
}

function extractTitle(root) {
  return firstNonEmpty([
    readContent('[itemprop="headline"]'),
    readContent('meta[property="og:title"]'),
    normalizeText(root.closest("article")?.querySelector("h1")?.textContent ?? ""),
    normalizeText(document.querySelector("h1")?.textContent ?? ""),
    document.title,
  ]);
}

function extractAuthor() {
  return firstNonEmpty([
    readContent('[itemprop="author"] [itemprop="name"]'),
    readContent('meta[name="author"]'),
    readContent('meta[property="article:author"]'),
    normalizeText(document.querySelector('[rel="author"]')?.textContent ?? ""),
  ]);
}

function extractDescription() {
  return firstNonEmpty([
    readContent('meta[name="description"]'),
    readContent('meta[property="og:description"]'),
  ]);
}

function extractPublishedAt() {
  return firstNonEmpty([
    readContent('[itemprop="datePublished"]'),
    readContent('meta[property="article:published_time"]'),
    document.querySelector("article time[datetime]")?.getAttribute("datetime"),
    document.querySelector("time[datetime]")?.getAttribute("datetime"),
  ]);
}

function extractCanonicalUrl() {
  const candidate = document.querySelector('link[rel="canonical"]')?.getAttribute("href");
  if (!candidate) {
    return window.location.href;
  }

  try {
    const url = new URL(candidate, document.baseURI);
    return /^https?:$/.test(url.protocol) ? url.href : window.location.href;
  } catch {
    return window.location.href;
  }
}

function readContent(selector) {
  const element = document.querySelector(selector);
  if (!element) {
    return undefined;
  }
  return normalizeText(element.getAttribute("content") ?? element.textContent ?? "");
}

function firstNonEmpty(values) {
  return values.find((value) => typeof value === "string" && value.trim())?.trim();
}

function normalizeText(value) {
  return value.replace(/\s+/g, " ").trim();
}

function fitPayloadToBudget(payload) {
  const encoder = new TextEncoder();
  const payloadBytes = () => encoder.encode(JSON.stringify(payload)).length;
  const reducibleFields = ["domHtml", "domText", "selectedText"];

  for (let iteration = 0; iteration < 24 && payloadBytes() > NODE_TIDE_MAX_PAYLOAD_BYTES; iteration += 1) {
    const field = reducibleFields.find(
      (name) => encoder.encode(payload[name] ?? "").length > 4096,
    );
    if (!field) {
      break;
    }

    const currentBytes = encoder.encode(payload[field]).length;
    const excessBytes = payloadBytes() - NODE_TIDE_MAX_PAYLOAD_BYTES;
    const reduction = Math.max(excessBytes + 2048, Math.ceil(currentBytes * 0.15));
    payload[field] = truncateUtf8(payload[field], Math.max(4096, currentBytes - reduction));
  }

  return payload;
}

function truncateUtf8(value, maxBytes) {
  const encoder = new TextEncoder();
  const bytes = encoder.encode(value);
  if (bytes.length <= maxBytes) {
    return value;
  }

  return new TextDecoder()
    .decode(bytes.slice(0, maxBytes))
    .replace(/\uFFFD$/, "")
    .trimEnd();
}
