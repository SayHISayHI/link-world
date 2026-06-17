const LINK_WORLD_ENDPOINT = "http://127.0.0.1:17321/capture";

const saveButton = document.querySelector("#save");
const statusElement = document.querySelector("#status");

saveButton.addEventListener("click", () => {
  void saveCurrentPage();
});

async function saveCurrentPage() {
  setBusy(true, "Saving...");

  try {
    const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
    if (!tab?.id || !isSupportedPageUrl(tab.url)) {
      throw new Error("Current page cannot be captured.");
    }

    const payload = await collectPayload(tab.id, tab.url, tab.title);
    const response = await fetch(LINK_WORLD_ENDPOINT, {
      method: "POST",
      headers: {
        "content-type": "application/json",
      },
      body: JSON.stringify(payload),
    });
    const result = await response.json().catch(() => undefined);

    if (!response.ok || !result?.ok) {
      throw new Error(result?.error ?? `Link World returned HTTP ${response.status}.`);
    }

    setBusy(false, "Saved.");
    window.setTimeout(() => window.close(), 700);
  } catch (error) {
    setBusy(false, error instanceof Error ? error.message : String(error));
  }
}

async function collectPayload(tabId, fallbackUrl, fallbackTitle) {
  const response = await sendCollectMessage(tabId).catch(async () => {
    await chrome.scripting.executeScript({
      target: { tabId },
      files: ["content_script.js"],
    });
    return sendCollectMessage(tabId);
  });

  if (!response?.ok || !response.payload) {
    throw new Error(response?.error ?? "Unable to read current page content.");
  }

  return {
    ...response.payload,
    url: response.payload.url || fallbackUrl,
    title: response.payload.title || fallbackTitle,
  };
}

function sendCollectMessage(tabId) {
  return chrome.tabs.sendMessage(tabId, {
    type: "LINK_WORLD_COLLECT_CURRENT_PAGE",
  });
}

function isSupportedPageUrl(url) {
  return typeof url === "string" && /^https?:\/\//i.test(url);
}

function setBusy(isBusy, message) {
  saveButton.disabled = isBusy;
  statusElement.textContent = message;
}
