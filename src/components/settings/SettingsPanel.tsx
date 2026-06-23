import { useEffect, useMemo, useState } from "react";
import { Check, KeyRound, Plus, Shield, Trash2 } from "lucide-react";
import {
  MODEL_API_FAMILY_OPTIONS,
  MODEL_PROVIDER_PRESETS,
} from "../../config/modelProviders";
import { useModelProviderConfigs } from "../../hooks/commands/useModelProviderConfigs";
import type {
  ModelApiFamily,
  ModelProviderConfig,
  ModelProviderConfigView,
} from "../../types/api";
import { Button } from "../ui/button";
import { StorageSettings } from "./StorageSettings";

export type SettingsPanelName =
  | "models"
  | "privacy"
  | "capture"
  | "plugins"
  | "storage"
  | "diagnostics"
  | "about";

interface SettingsPanelProps {
  panel: SettingsPanelName;
  onPanelChange: (panel: SettingsPanelName) => void;
}

const settingsSections: Array<{ id: SettingsPanelName; label: string }> = [
  { id: "models", label: "Models" },
  { id: "privacy", label: "Privacy" },
  { id: "capture", label: "Capture" },
  { id: "plugins", label: "Plugins" },
  { id: "storage", label: "Storage" },
  { id: "diagnostics", label: "Diagnostics" },
  { id: "about", label: "About" },
];

const emptyDraft: ModelProviderConfig = {
  provider: "openai",
  apiFamily: "openai_chat_completions",
  chatBaseUrl: "https://api.openai.com/v1",
  defaultChatModel: "gpt-4.1-mini",
  capabilities: ["chat"],
  enabled: true,
};

export function SettingsPanel({ panel, onPanelChange }: SettingsPanelProps) {
  return (
    <div className="flex h-screen min-w-0 bg-background">
      <aside className="w-52 shrink-0 border-r border-border bg-surface p-4">
        <h1 className="px-2 text-base font-semibold">Settings</h1>
        <nav className="mt-5 space-y-1" aria-label="Settings sections">
          {settingsSections.map((section) => (
            <Button
              key={section.id}
              variant={panel === section.id ? "secondary" : "ghost"}
              className="w-full justify-start"
              onClick={() => onPanelChange(section.id)}
            >
              {section.label}
            </Button>
          ))}
        </nav>
      </aside>
      <main className="min-w-0 flex-1 overflow-y-auto">
        {panel === "models" ? (
          <ModelSettings />
        ) : panel === "storage" ? (
          <StorageSettings />
        ) : (
          <SettingsBoundary panel={panel} />
        )}
      </main>
    </div>
  );
}

function ModelSettings() {
  const {
    configs,
    error,
    loading,
    mutating,
    testError,
    testLoading,
    testResult,
    clearTestResult,
    deleteConfig,
    loadConfigs,
    saveConfig,
    setDefault,
    testConfig,
  } = useModelProviderConfigs();
  const [selectedId, setSelectedId] = useState<string>();
  const [draft, setDraft] = useState<ModelProviderConfig>(emptyDraft);

  useEffect(() => {
    void loadConfigs();
  }, [loadConfigs]);

  const selectedConfig = useMemo(
    () => configs.find((config) => config.id === selectedId),
    [configs, selectedId],
  );

  const selectConfig = (config: ModelProviderConfigView) => {
    setSelectedId(config.id);
    setDraft({
      id: config.id,
      provider: config.provider,
      apiFamily: config.apiFamily,
      chatBaseUrl: config.chatBaseUrl,
      embeddingsBaseUrl: config.embeddingsBaseUrl,
      defaultChatModel: config.defaultChatModel,
      defaultEmbeddingModel: config.defaultEmbeddingModel,
      capabilities: config.capabilities,
      enabled: config.enabled,
    });
    clearTestResult();
  };

  const startNewConfig = () => {
    setSelectedId(undefined);
    setDraft(emptyDraft);
    clearTestResult();
  };

  const updateDraft = (updates: Partial<ModelProviderConfig>) => {
    setDraft((current) => ({ ...current, ...updates }));
    clearTestResult();
  };

  const updateProvider = (provider: string) => {
    const preset = MODEL_PROVIDER_PRESETS[provider.trim().toLowerCase()];
    setDraft((current) => ({
      ...current,
      provider,
      ...(preset
        ? {
            apiFamily: preset.apiFamily,
            chatBaseUrl: preset.chatBaseUrl,
            defaultChatModel: preset.chatModel,
          }
        : {}),
    }));
    clearTestResult();
  };

  const handleSave = async () => {
    const saved = await saveConfig(normalizeDraft(draft));
    if (saved) {
      selectConfig(saved);
    }
  };

  const handleDelete = async () => {
    if (!selectedId) {
      return;
    }
    const confirmed = window.confirm(
      selectedConfig?.isDefault
        ? "Delete the default model config? AI analysis will stop until another default is selected."
        : "Delete this model config?",
    );
    if (confirmed && (await deleteConfig(selectedId))) {
      startNewConfig();
    }
  };

  return (
    <div className="mx-auto max-w-6xl p-8">
      <div className="flex items-start justify-between gap-5">
        <div>
          <h2 className="text-xl font-semibold">Model providers</h2>
          <p className="mt-2 max-w-2xl text-sm leading-6 text-muted-foreground">
            Configure reusable model connections here. Object pages use the selected default but
            never expose or edit credentials.
          </p>
        </div>
        <Button variant="secondary" onClick={startNewConfig}>
          <Plus className="h-4 w-4" aria-hidden="true" />
          Add provider
        </Button>
      </div>

      <div className="mt-7 grid gap-6 lg:grid-cols-[300px_minmax(0,1fr)]">
        <section>
          <h3 className="text-sm font-semibold">Configured providers</h3>
          <div className="mt-3 space-y-2">
            {loading ? <p className="text-sm text-muted-foreground">Loading providers...</p> : null}
            {!loading && configs.length === 0 ? (
              <div className="rounded-lg border border-dashed border-border p-5 text-sm text-muted-foreground">
                No model provider configured.
              </div>
            ) : null}
            {configs.map((config) => (
              <button
                key={config.id}
                type="button"
                className={
                  "w-full rounded-lg border p-3 text-left transition-colors " +
                  (selectedId === config.id
                    ? "border-accent bg-surface"
                    : "border-border bg-background hover:bg-surface")
                }
                onClick={() => selectConfig(config)}
              >
                <div className="flex items-center justify-between gap-3">
                  <span className="font-medium">{config.provider}</span>
                  {config.isDefault ? (
                    <span className="rounded-full bg-emerald-100 px-2 py-0.5 text-[11px] text-emerald-800">
                      Default
                    </span>
                  ) : null}
                </div>
                <p className="mt-1 truncate text-xs text-muted-foreground">
                  {config.defaultChatModel ?? "No chat model"}
                </p>
                <p className="mt-1 text-[11px] text-muted-foreground">
                  {config.enabled ? "Enabled" : "Disabled"} /{" "}
                  {config.hasApiKey ? "credential available" : "no credential"}
                </p>
              </button>
            ))}
          </div>
        </section>

        <section className="rounded-xl border border-border bg-surface p-5">
          <div className="flex items-center gap-2">
            <KeyRound className="h-4 w-4" aria-hidden="true" />
            <h3 className="text-sm font-semibold">
              {selectedId ? "Edit provider" : "New provider"}
            </h3>
          </div>
          <div className="mt-5 grid gap-4 md:grid-cols-2">
            <Field label="Provider">
              <input
                className={inputClass}
                value={draft.provider}
                onChange={(event) => updateProvider(event.target.value)}
                placeholder="openai, anthropic, ollama, or custom"
                list="settings-model-provider-suggestions"
              />
              <datalist id="settings-model-provider-suggestions">
                {Object.keys(MODEL_PROVIDER_PRESETS).map((provider) => (
                  <option key={provider} value={provider} />
                ))}
              </datalist>
            </Field>
            <Field label="API protocol">
              <select
                className={inputClass}
                value={draft.apiFamily}
                onChange={(event) =>
                  updateDraft({ apiFamily: event.target.value as ModelApiFamily })
                }
              >
                {MODEL_API_FAMILY_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Chat base URL" wide>
              <input
                className={inputClass}
                value={draft.chatBaseUrl ?? ""}
                onChange={(event) =>
                  updateDraft({ chatBaseUrl: event.target.value })
                }
                placeholder="https://api.openai.com/v1"
              />
            </Field>
            <Field label="Chat model">
              <input
                className={inputClass}
                value={draft.defaultChatModel ?? ""}
                onChange={(event) =>
                  updateDraft({ defaultChatModel: event.target.value })
                }
                placeholder="gpt-4.1-mini"
              />
            </Field>
            <Field label="API key">
              <input
                className={inputClass}
                value={draft.apiKey ?? ""}
                onChange={(event) =>
                  updateDraft({ apiKey: event.target.value })
                }
                placeholder={
                  selectedConfig?.hasApiKey ? "Configured - leave blank to keep it" : "Optional for local providers"
                }
                type="password"
                autoComplete="new-password"
              />
            </Field>
          </div>

          <label className="mt-4 flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={draft.enabled ?? true}
              onChange={(event) =>
                updateDraft({ enabled: event.target.checked })
              }
            />
            Enable this provider
          </label>

          <div className="mt-5 flex flex-wrap gap-2">
            <Button onClick={handleSave} disabled={mutating || testLoading}>
              {mutating ? "Saving..." : "Save"}
            </Button>
            <Button
              variant="secondary"
              onClick={() => void testConfig(normalizeDraft(draft))}
              disabled={mutating || testLoading}
            >
              {testLoading ? "Testing..." : "Test connection"}
            </Button>
            {selectedId && !selectedConfig?.isDefault ? (
              <Button
                variant="secondary"
                onClick={() => void setDefault(selectedId)}
                disabled={mutating || !(draft.enabled ?? true)}
              >
                <Check className="h-4 w-4" aria-hidden="true" />
                Set as default
              </Button>
            ) : null}
            {selectedId ? (
              <Button variant="ghost" onClick={handleDelete} disabled={mutating}>
                <Trash2 className="h-4 w-4" aria-hidden="true" />
                Delete
              </Button>
            ) : null}
          </div>

          {testResult ? (
            <p className="mt-4 rounded-md border border-emerald-200 bg-emerald-50 p-3 text-sm text-emerald-800">
              Connected to {testResult.provider} / {testResult.model} in {testResult.latencyMs} ms.
            </p>
          ) : null}
          {error || testError ? (
            <div className="mt-4 rounded-md border border-red-200 bg-red-50 p-3 text-sm text-red-800">
              <p className="font-medium">{(error ?? testError)?.title}</p>
              <p className="mt-1">{(error ?? testError)?.message}</p>
            </div>
          ) : null}

          <div className="mt-6 flex gap-3 rounded-lg border border-border bg-background p-4 text-xs leading-5 text-muted-foreground">
            <Shield className="mt-0.5 h-4 w-4 shrink-0" aria-hidden="true" />
            <p>
              API keys are stored in Windows Credential Manager. Database rows contain only an
              opaque credential reference; saved keys are never returned to the UI.
            </p>
          </div>
        </section>
      </div>
    </div>
  );
}

function SettingsBoundary({
  panel,
}: {
  panel: Exclude<SettingsPanelName, "models" | "storage">;
}) {
  const copy: Record<
    Exclude<SettingsPanelName, "models" | "storage">,
    [string, string]
  > = {
    privacy: [
      "Privacy",
      "Per-object AI permissions and cloud-processing defaults will be managed here. Both remain off by default.",
    ],
    capture: ["Capture", "Capture source defaults and extension connectivity will be managed here."],
    plugins: ["Plugins", "Plugin manifests, permissions, updates, and isolation status will be managed here."],
    diagnostics: ["Diagnostics", "Runtime health, job failures, and redacted support bundles will be managed here."],
    about: ["About", "Version, release channel, licenses, and update controls will be managed here."],
  };
  const [title, description] = copy[panel];

  return (
    <div className="mx-auto max-w-4xl p-8">
      <h2 className="text-xl font-semibold">{title}</h2>
      <p className="mt-3 max-w-2xl text-sm leading-6 text-muted-foreground">{description}</p>
      <div className="mt-6 rounded-lg border border-dashed border-border p-5 text-sm text-muted-foreground">
        This section is intentionally bounded for the current milestone.
      </div>
    </div>
  );
}

function Field({
  children,
  label,
  wide = false,
}: {
  children: React.ReactNode;
  label: string;
  wide?: boolean;
}) {
  return (
    <label className={wide ? "block md:col-span-2" : "block"}>
      <span className="text-xs font-medium">{label}</span>
      <div className="mt-1">{children}</div>
    </label>
  );
}

function normalizeDraft(draft: ModelProviderConfig): ModelProviderConfig {
  return {
    ...draft,
    id: draft.id?.trim() || undefined,
    provider: draft.provider.trim(),
    chatBaseUrl: draft.chatBaseUrl?.trim(),
    apiKey: draft.apiKey?.trim() || undefined,
    defaultChatModel: draft.defaultChatModel?.trim(),
    capabilities: ["chat"],
    enabled: draft.enabled ?? true,
  };
}

const inputClass =
  "h-9 w-full rounded-md border border-border bg-background px-3 text-sm outline-none focus:ring-2 focus:ring-accent";

