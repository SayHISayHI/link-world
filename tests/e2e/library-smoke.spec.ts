import { expect, test, type Page } from "@playwright/test";

const FIXTURE_URL = "https://example.com/node-tide-smoke";
const FIXTURE_TITLE = "Node Tide Smoke Article";
const FIXTURE_BODY = "A deterministic browser smoke fixture for the core library journey.";

test.setTimeout(60_000);

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
});

test("saves, searches, opens, degrades without a model, and enters Settings", async ({
  page,
}) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));

  await page.goto("/", { waitUntil: "commit" });

  await expect(page.getByRole("heading", { name: "拾海 · Node Tide" })).toBeVisible({
    timeout: 45_000,
  });
  await expect(page.getByText("No captured objects yet.", { exact: false })).toBeVisible();
  await expect(page.getByText("Your first useful loop")).toBeVisible();
  await expect(page.getByRole("button", { name: "Configure AI (optional)" })).toBeVisible();

  const omnibox = page.getByPlaceholder("Search or paste a URL to save...");
  await omnibox.fill(FIXTURE_URL);
  await omnibox.press("Enter");

  await expect(page.getByRole("heading", { name: FIXTURE_TITLE }).last()).toBeVisible();
  await expect(page.getByText(FIXTURE_BODY)).toBeVisible({ timeout: 20_000 });

  await omnibox.fill("smoke article");
  await expect(page.getByRole("heading", { name: "Search" })).toBeVisible();
  const result = page.getByRole("button", { name: new RegExp(FIXTURE_TITLE) });
  await expect(result).toBeVisible();
  await result.click();
  await expect(page.getByText(FIXTURE_BODY)).toBeVisible({ timeout: 20_000 });

  await page.getByRole("button", { name: "Run analysis" }).click();
  await expect(page.getByText("No default model configured", { exact: true })).toBeVisible();

  await page.getByRole("button", { name: "Models", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Model providers" })).toBeVisible();
  await expect(page.getByText("No model provider configured.")).toBeVisible();

  expect(pageErrors).toEqual([]);
});

async function installTauriMock(page: Page) {
  await page.addInitScript(() => {
    type Callback = (payload: unknown) => void;
    type InvokeArgs = Record<string, unknown>;

    interface TauriInternals {
      invoke: (command: string, args?: InvokeArgs) => Promise<unknown>;
      transformCallback: (callback?: Callback, once?: boolean) => number;
      unregisterCallback: (id: number) => void;
      convertFileSrc: (path: string) => string;
    }

    interface TauriEventInternals {
      unregisterListener: (event: string, eventId: number) => void;
    }

    interface MockWindow extends Window {
      __TAURI_INTERNALS__: TauriInternals;
      __TAURI_EVENT_PLUGIN_INTERNALS__: TauriEventInternals;
    }

    const runtime = window as unknown as MockWindow;
    const callbacks = new Map<number, { callback?: Callback; once: boolean }>();
    let nextCallbackId = 1;
    let nextEventId = 1;
    let captured = false;
    const now = "2026-07-11T00:00:00Z";
    const object = {
      id: "object-e2e-smoke",
      userId: "local-user",
      type: "article",
      title: "Node Tide Smoke Article",
      canonicalUrl: "https://example.com/node-tide-smoke",
      sourcePlatform: "example.com",
      privacyLevel: "personal",
      lifecycleStatus: "parsed",
      capturedAt: now,
      updatedAt: now,
    };
    const detail = {
      object,
      parsedDocument: {
        id: "document-e2e-smoke",
        objectId: object.id,
        title: object.title,
        text: "A deterministic browser smoke fixture for the core library journey.",
        markdown: "# Node Tide Smoke Article\n\nA deterministic browser smoke fixture for the core library journey.",
        language: "en",
        wordCount: 10,
        parserId: "e2e-fixture",
        parserVersion: "1",
        contentHash: "e2e-fixture-hash",
        createdAt: now,
      },
      snapshots: [],
      aiAnalyses: [],
      evaluations: [],
    };
    const navigation = {
      systemViews: [
        { id: "inbox", label: "Inbox", count: 0, kind: "system", iconKey: "inbox" },
        { id: "all", label: "All", count: 0, kind: "system", iconKey: "library" },
      ],
      collections: [],
      topics: [],
      smartViews: [],
    };
    const ok = (data: unknown) => ({ status: "ok", data });

    runtime.__TAURI_INTERNALS__ = {
      async invoke(command, args = {}) {
        if (command === "plugin:event|listen") {
          return nextEventId++;
        }
        if (command === "plugin:event|unlisten") {
          return undefined;
        }

        switch (command) {
          case "get_startup_status":
            return ok({ mode: "ready", backendVersion: "0.1.0" });
          case "list_library_objects":
            return ok({ items: captured ? [object] : [] });
          case "get_library_navigation":
            return ok({
              ...navigation,
              systemViews: navigation.systemViews.map((item) => ({
                ...item,
                count: captured ? 1 : 0,
              })),
            });
          case "submit_capture":
            captured = true;
            return ok({ objectId: object.id, deduplicated: false });
          case "get_object_detail":
            return ok(detail);
          case "get_object_jobs":
            return ok([]);
          case "get_object_organization":
            return ok({
              objectId: object.id,
              triageStatus: "inbox",
              tags: [],
              collections: [],
              tagSuggestions: [],
            });
          case "search_library": {
            const query = String(args.query ?? "").toLowerCase();
            const matches = captured && (query.includes("smoke") || query.includes("article"));
            return ok(
              matches
                ? [
                    {
                      object,
                      matchedFields: ["title", "parsed_content"],
                      snippet: "A deterministic browser smoke fixture.",
                      score: 1,
                    },
                  ]
                : [],
            );
          }
          case "trigger_ai_enrichment":
            return ok({
              jobId: "job-e2e-ai",
              correlationId: "correlation-e2e-ai",
              status: "failed",
              failureReason:
                "ai.not_configured: Configure a default provider in Settings before running analysis.",
            });
          case "list_model_provider_configs":
            return ok([]);
          case "ping":
            return ok({ message: "pong", backendVersion: "0.1.0" });
          default:
            throw new Error(`Unexpected mocked Tauri command: ${command}`);
        }
      },
      transformCallback(callback, once = false) {
        const id = nextCallbackId++;
        callbacks.set(id, { callback, once });
        return id;
      },
      unregisterCallback(id) {
        callbacks.delete(id);
      },
      convertFileSrc(path) {
        return path;
      },
    };
    runtime.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener() {
        return undefined;
      },
    };
  });
}
