# Node Tide Test Fixtures

Fixtures are deterministic sample inputs for tests and local development.

Rules:

- Do not include real user data.
- Do not include real API keys, tokens, cookies or sessions.
- Keep samples small.
- Prefer fake but realistic content.
- Include both success and failure cases.

## Files

- `capture_payloads.json`: sample `RawCaptureItem` payloads.
- `model_responses.json`: sample model outputs for AI analysis, document display hints and evaluation.
- `external_errors.json`: sample external API failure responses.

## Usage

Tests should load fixtures by filename and never mutate them in place. If a test needs to modify fixture data, clone it in memory or copy it to a temp directory.

Parser and renderer regressions should be reduced to synthetic semantic HTML or Markdown. Do not commit full HTML copied from a real article. Display-hint fixtures must stay optional and include `schemaVersion`, an allowed mode and a bounded confidence value.
