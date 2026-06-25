import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CaptureBar } from "./CaptureBar";

describe("CaptureBar", () => {
  it("renders capture failure codes as user-facing recovery text", () => {
    render(
      <CaptureBar
        value="https://example.com/private"
        loading={false}
        job={{
          status: "failed",
          failureReason:
            "capture.http_forbidden: The server returned HTTP 403. Open it in your browser and save it with the browser extension.",
        }}
        onChange={vi.fn()}
        onSubmit={vi.fn()}
      />,
    );

    expect(screen.getByText("Browser access required")).toBeInTheDocument();
    expect(screen.getByText(/browser extension/)).toBeInTheDocument();
    expect(screen.queryByText(/capture.http_forbidden/)).not.toBeInTheDocument();
  });

  it("submits the entered URL", () => {
    const onSubmit = vi.fn();
    render(
      <CaptureBar
        value="https://example.com/article"
        loading={false}
        onChange={vi.fn()}
        onSubmit={onSubmit}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit).toHaveBeenCalledOnce();
  });

  it("renders duplicate URL submissions as an existing saved item", () => {
    render(
      <CaptureBar
        value=""
        loading={false}
        job={{
          status: "deduplicated",
          lifecycleStatus: "already saved",
        }}
        onChange={vi.fn()}
        onSubmit={vi.fn()}
      />,
    );

    expect(screen.getByText("Already saved")).toBeInTheDocument();
    expect(screen.getByText(/existing library item/)).toBeInTheDocument();
    expect(screen.queryByText(/Capture job deduplicated/)).not.toBeInTheDocument();
  });
});
