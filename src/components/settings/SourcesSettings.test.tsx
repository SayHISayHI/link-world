import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SourcesSettings } from "./SourcesSettings";

const useSourceCredentialsMock = vi.hoisted(() => vi.fn());

vi.mock("../../hooks/commands/useSourceCredentials", () => ({
  useSourceCredentials: useSourceCredentialsMock,
}));

const deleteCredential = vi.fn();
const load = vi.fn();
const probeUrl = vi.fn();
const saveCredential = vi.fn();

function hookState(overrides: Record<string, unknown> = {}) {
  return {
    data: {
      backend: "file_0600",
      platforms: [
        {
          id: "source.x",
          label: "X / Twitter",
          cookieDomain: null,
          configured: false,
          updatedAt: null,
        },
        {
          id: "source.xiaohongshu",
          label: "小红书",
          cookieDomain: "xiaohongshu.com",
          configured: true,
          updatedAt: "2026-11-13T08:00:00Z",
        },
      ],
    },
    loading: false,
    saving: false,
    probing: false,
    load,
    saveCredential,
    deleteCredential,
    probeUrl,
    ...overrides,
  };
}

beforeEach(() => {
  deleteCredential.mockReset();
  load.mockReset();
  probeUrl.mockReset();
  saveCredential.mockReset();
  useSourceCredentialsMock.mockReturnValue(hookState());
});

describe("SourcesSettings", () => {
  it("shows which platforms need a cookie and which already have one", () => {
    render(<SourcesSettings />);

    expect(screen.getByText("Public access")).toBeInTheDocument();
    expect(screen.getByText(/Cookie domain: xiaohongshu.com/)).toBeInTheDocument();
    expect(screen.getByText(/^Saved /)).toBeInTheDocument();
  });

  it("saves a pasted cookie for the platform domain and clears the field", () => {
    render(<SourcesSettings />);

    const input = screen.getByLabelText("Cookie for 小红书");
    fireEvent.change(input, { target: { value: "a1=abc; web_session=def" } });
    fireEvent.click(screen.getByRole("button", { name: "Save cookie for 小红书" }));

    expect(saveCredential).toHaveBeenCalledWith(
      "xiaohongshu.com",
      "a1=abc; web_session=def",
    );
  });

  it("removes a stored cookie", () => {
    render(<SourcesSettings />);

    fireEvent.click(screen.getByRole("button", { name: "Remove cookie for 小红书" }));

    expect(deleteCredential).toHaveBeenCalledWith("xiaohongshu.com");
  });

  it("probes a sample link and reports a working adapter", () => {
    useSourceCredentialsMock.mockReturnValue(
      hookState({
        probe: {
          matchedAdapter: "source.xiaohongshu",
          resolved: true,
          status: "ok",
          title: "一周早餐备餐",
          author: "阿栗",
          textLength: 805,
          assets: [{ kind: "image", url: "https://cdn.example.com/one.jpg" }],
          excerpt: "正文",
          message: null,
        },
      }),
    );

    render(<SourcesSettings />);

    const input = screen.getByLabelText("Sample link for 小红书");
    fireEvent.change(input, { target: { value: "https://xhslink.cn/o/abc" } });
    fireEvent.click(screen.getByRole("button", { name: "Check link for 小红书" }));

    expect(probeUrl).toHaveBeenCalledWith("https://xhslink.cn/o/abc");
    expect(screen.getByText("Adapter works")).toBeInTheDocument();
    expect(
      screen.getByText(/一周早餐备餐 · 1 media · 805 characters/),
    ).toBeInTheDocument();
  });

  it("separates an unreachable network from a changed platform", () => {
    useSourceCredentialsMock.mockReturnValue(
      hookState({
        probe: {
          matchedAdapter: "source.douyin",
          resolved: false,
          status: "network_unreachable",
          title: null,
          author: null,
          textLength: 0,
          assets: [],
          excerpt: null,
          message: "network timeout",
        },
      }),
    );

    render(<SourcesSettings />);

    fireEvent.change(screen.getByLabelText("Sample link for X / Twitter"), {
      target: { value: "https://v.douyin.com/abc/" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Check link for X / Twitter" }));

    expect(
      screen.getByText("This machine cannot reach the platform"),
    ).toBeInTheDocument();
  });

  it("tells the user when the cookie is missing or expired", () => {
    useSourceCredentialsMock.mockReturnValue(
      hookState({
        probe: {
          matchedAdapter: "source.xiaohongshu",
          resolved: false,
          status: "login_required",
          title: null,
          author: null,
          textLength: 0,
          assets: [],
          excerpt: null,
          message: "xiaohongshu payload missing",
        },
      }),
    );

    render(<SourcesSettings />);

    fireEvent.change(screen.getByLabelText("Sample link for 小红书"), {
      target: { value: "https://xhslink.cn/o/abc" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Check link for 小红书" }));

    expect(screen.getByText("Cookie is missing or expired")).toBeInTheDocument();
  });
});
