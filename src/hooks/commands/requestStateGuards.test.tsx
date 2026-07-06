import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { BackgroundJob, KnowledgeObject, KnowledgeObjectDetail, SearchResult } from "../../types/api";
import { useObjectDetail } from "./useObjectDetail";
import { useObjectJobs } from "./useObjectJobs";
import { useRecentObjects } from "./useRecentObjects";
import { useSearchHybrid } from "./useSearchHybrid";

const { invokeCommandMock } = vi.hoisted(() => ({
  invokeCommandMock: vi.fn(),
}));

vi.mock("../../lib/tauri", () => ({
  invokeCommand: invokeCommandMock,
}));

afterEach(() => {
  invokeCommandMock.mockReset();
});

describe("request state guards", () => {
  it("keeps the latest recent objects response when older loads finish later", async () => {
    const first = deferred<KnowledgeObject[]>();
    const second = deferred<KnowledgeObject[]>();
    invokeCommandMock.mockImplementationOnce(() => first.promise).mockImplementationOnce(() => second.promise);

    const { result } = renderHook(() => useRecentObjects());

    act(() => {
      void result.current.loadRecentObjects({ limit: 30 });
      void result.current.loadRecentObjects({ limit: 30 });
    });

    await act(async () => {
      second.resolve([createObject({ title: "Newest title", lifecycleStatus: "parsed" })]);
      await second.promise;
    });

    await waitFor(() => {
      expect(result.current.data[0]?.title).toBe("Newest title");
    });

    await act(async () => {
      first.resolve([createObject({ title: "Stale title", lifecycleStatus: "captured" })]);
      await first.promise;
    });

    expect(result.current.data[0]?.title).toBe("Newest title");
    expect(result.current.data[0]?.lifecycleStatus).toBe("parsed");
  });

  it("keeps the latest object detail when an older detail request resolves last", async () => {
    const first = deferred<KnowledgeObjectDetail>();
    const second = deferred<KnowledgeObjectDetail>();
    invokeCommandMock.mockImplementationOnce(() => first.promise).mockImplementationOnce(() => second.promise);

    const { result } = renderHook(() => useObjectDetail());

    act(() => {
      void result.current.loadObjectDetail("obj-1");
      void result.current.loadObjectDetail("obj-1");
    });

    await act(async () => {
      second.resolve(createDetail({ title: "Parsed title", lifecycleStatus: "parsed" }));
      await second.promise;
    });

    await waitFor(() => {
      expect(result.current.data?.object.title).toBe("Parsed title");
    });

    await act(async () => {
      first.resolve(createDetail({ title: "Captured title", lifecycleStatus: "captured" }));
      await first.promise;
    });

    expect(result.current.data?.object.title).toBe("Parsed title");
    expect(result.current.data?.object.lifecycleStatus).toBe("parsed");
  });

  it("ignores in-flight object jobs after reset", async () => {
    const pending = deferred<BackgroundJob[]>();
    invokeCommandMock.mockImplementationOnce(() => pending.promise);

    const { result } = renderHook(() => useObjectJobs());

    act(() => {
      void result.current.loadObjectJobs({ objectId: "obj-1", limit: 10 });
      result.current.resetObjectJobs();
    });

    await act(async () => {
      pending.resolve([createJob()]);
      await pending.promise;
    });

    expect(result.current.data).toEqual([]);
    expect(result.current.loading).toBe(false);
  });

  it("ignores stale search results after the search is reset", async () => {
    const pending = deferred<SearchResult[]>();
    invokeCommandMock.mockImplementationOnce(() => pending.promise);

    const { result } = renderHook(() => useSearchHybrid());

    act(() => {
      void result.current.searchHybrid({ query: "wasm", limit: 10 });
      result.current.resetSearch();
    });

    await act(async () => {
      pending.resolve([createSearchResult()]);
      await pending.promise;
    });

    expect(result.current.data).toEqual([]);
    expect(result.current.loading).toBe(false);
  });
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });

  return { promise, resolve, reject };
}

function createObject(overrides: Partial<KnowledgeObject> = {}): KnowledgeObject {
  return {
    id: "obj-1",
    userId: "local",
    type: "article",
    title: "Original title",
    canonicalUrl: "https://example.com/article",
    privacyLevel: "personal",
    lifecycleStatus: "captured",
    capturedAt: "2026-07-06T00:00:00.000Z",
    updatedAt: "2026-07-06T00:00:00.000Z",
    ...overrides,
  };
}

function createDetail(overrides: Partial<KnowledgeObject> = {}): KnowledgeObjectDetail {
  return {
    object: createObject(overrides),
    snapshots: [],
    aiAnalyses: [],
    evaluations: [],
  };
}

function createJob(overrides: Partial<BackgroundJob> = {}): BackgroundJob {
  return {
    id: "job-1",
    type: "capture.fetch_url",
    status: "failed",
    objectId: "obj-1",
    attemptCount: 1,
    maxAttempts: 3,
    createdAt: "2026-07-06T00:00:00.000Z",
    updatedAt: "2026-07-06T00:00:00.000Z",
    ...overrides,
  };
}

function createSearchResult(overrides: Partial<SearchResult> = {}): SearchResult {
  return {
    object: createObject({ title: "Newest search title", lifecycleStatus: "parsed" }),
    matchedFields: ["title"],
    snippet: "Newest snippet",
    score: 0.9,
    ...overrides,
  };
}

