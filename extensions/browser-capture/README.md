# Link World Browser Capture

Minimal Manifest V3 extension for MVP browser capture.

## Local Install

1. Start the Link World desktop app.
2. Open Chromium extension management.
3. Enable developer mode.
4. Load unpacked extension from this directory: `extensions/browser-capture`.
5. Open any `http` or `https` page and click `Save current page`.

## Boundary

- Captures only the current active page after a user click.
- Submits URL, title, selected text, visible text and a sanitized DOM fragment.
- Does not read cookies, background tabs, browser history or platform sessions.
- Sends data only to `http://127.0.0.1:17321/capture`.
