// Frozen browser-storage keys from the former Link World identity.
// Renaming them drops every user's saved locale, theme and layout preferences,
// so a new key needs a migration that reads the legacy one first.
// See docs/storage_abi.md.
export const LEGACY_UI_STORAGE_KEY = "link-world-ui";
