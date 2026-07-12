# Node Tide Browser Capture

Minimal Manifest V3 extension for MVP browser capture.

## Local Install

1. Start the Node Tide desktop app.
2. Open Chromium extension management.
3. Enable developer mode.
4. Load unpacked extension from this directory: `extensions/browser-capture`.
5. Open any `http` or `https` page and click `Save current page`.

## Boundary

- Captures only the current active page after a user click.
- Submits a sanitized rendered DOM fragment plus canonical URL, title, author and publication metadata.
- Leaves document parsing and Markdown generation to the shared Rust parser used by every capture path.
- Does not read cookies, background tabs, browser history or platform sessions.
- Sends data only to `http://127.0.0.1:17321/capture`.

## Manual Chrome Release Smoke

Run this checklist against every Windows release candidate because CI does not install an unpacked extension:

1. Start the installed Node Tide desktop candidate and confirm the loopback capture service is ready.
2. Load this directory as an unpacked extension in a clean Chrome profile.
3. Save a synthetic `https` article and confirm the popup reports success without exposing response bodies or local paths.
4. Verify the object becomes `parsed`, its title/body boundaries are correct, and local search finds both title and body text.
5. Save an explicit text selection and verify only that selection is stored, not the surrounding page.
6. Stop the desktop app and verify the extension reports a bounded connection failure with no retry loop.
7. Record Chrome version, extension manifest version, app commit, result, and any waiver in the Windows Alpha evidence.
