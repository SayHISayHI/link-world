import type { EventCallback, UnlistenFn } from "@tauri-apps/api/event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { listenMock } = vi.hoisted(() => ({
  listenMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
}));

import { subscribeToLibraryEvents } from "./libraryEvents";

describe("subscribeToLibraryEvents", () => {
  beforeEach(() => {
    listenMock.mockReset();
  });

  it("registers every backend update event and routes payloads", async () => {
    const listeners = new Map<string, EventCallback<unknown>>();
    const unlisten = vi.fn();
    listenMock.mockImplementation(
      async (eventName: string, callback: EventCallback<unknown>): Promise<UnlistenFn> => {
        listeners.set(eventName, callback);
        return unlisten;
      },
    );
    const handlers = {
      onObjectsUpdated: vi.fn(),
      onCaptureJobCompleted: vi.fn(),
      onAIEnrichmentCompleted: vi.fn(),
      onEvaluationCompleted: vi.fn(),
      onSearchIndexRebuilt: vi.fn(),
      onObjectReindexed: vi.fn(),
    };

    const unsubscribe = await subscribeToLibraryEvents(handlers);

    expect(Array.from(listeners.keys())).toEqual([
      "library://objects-updated",
      "capture://job-completed",
      "ai://enrichment-completed",
      "evaluation://completed",
      "search://index-rebuilt",
      "search://object-reindexed",
    ]);

    listeners.get("library://objects-updated")?.({
      event: "library://objects-updated",
      id: 1,
      payload: undefined,
    });
    listeners.get("capture://job-completed")?.({
      event: "capture://job-completed",
      id: 2,
      payload: { objectId: "object-1", status: "succeeded" },
    });

    expect(handlers.onObjectsUpdated).toHaveBeenCalledOnce();
    expect(handlers.onCaptureJobCompleted).toHaveBeenCalledWith({
      objectId: "object-1",
      status: "succeeded",
    });

    unsubscribe();
    expect(unlisten).toHaveBeenCalledTimes(6);
  });

  it("cleans up listeners registered before a subscription failure", async () => {
    const unlisten = vi.fn();
    listenMock
      .mockResolvedValueOnce(unlisten)
      .mockRejectedValueOnce(new Error("event permission denied"));

    await expect(
      subscribeToLibraryEvents({
        onObjectsUpdated: vi.fn(),
        onCaptureJobCompleted: vi.fn(),
        onAIEnrichmentCompleted: vi.fn(),
        onEvaluationCompleted: vi.fn(),
        onSearchIndexRebuilt: vi.fn(),
        onObjectReindexed: vi.fn(),
      }),
    ).rejects.toThrow("event permission denied");
    expect(unlisten).toHaveBeenCalledOnce();
  });
});
