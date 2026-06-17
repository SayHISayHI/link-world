const LINK_WORLD_MAX_TEXT_CHARS = 20000;
const LINK_WORLD_MAX_HTML_CHARS = 120000;

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (message?.type !== "LINK_WORLD_COLLECT_CURRENT_PAGE") {
    return false;
  }

  try {
    sendResponse({
      ok: true,
      payload: collectCurrentPage(),
    });
  } catch (error) {
    sendResponse({
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    });
  }

  return true;
});

function collectCurrentPage() {
  const selectedText = truncateText(String(window.getSelection?.() ?? ""), LINK_WORLD_MAX_TEXT_CHARS);
  const root = findReadableRoot();
  const clone = root.cloneNode(true);
  sanitizeClone(clone);

  return {
    url: window.location.href,
    title: document.title,
    selectedText,
    domText: truncateText(clone.textContent ?? "", LINK_WORLD_MAX_TEXT_CHARS),
    domHtml: truncateText(clone.innerHTML, LINK_WORLD_MAX_HTML_CHARS),
    sourcePlatform: window.location.hostname,
    capturedAt: new Date().toISOString(),
  };
}

function findReadableRoot() {
  return (
    document.querySelector("article") ??
    document.querySelector("main") ??
    document.querySelector('[role="main"]') ??
    document.querySelector('[itemprop="articleBody"]') ??
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
      if (attribute.name.startsWith("on") || attribute.name === "style") {
        node.removeAttribute(attribute.name);
      }
    }
  });
}

function truncateText(value, maxChars) {
  const normalized = value.replace(/\s+/g, " ").trim();
  if (normalized.length <= maxChars) {
    return normalized;
  }

  return normalized.slice(0, maxChars);
}
