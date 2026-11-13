import { useMemo, useState } from "react";
import { KeyRound, RefreshCw, ShieldCheck, Trash2 } from "lucide-react";
import { useSourceCredentials } from "../../hooks/commands/useSourceCredentials";
import type { SourcePlatformCredential, SourceProbeResponse } from "../../types/api";
import { Button } from "../ui/button";
import { useI18n, type Translator } from "../../i18n";

export function SourcesSettings() {
  const { t } = useI18n();
  const {
    data,
    error,
    loading,
    saving,
    probing,
    probe,
    probeError,
    deleteCredential,
    load,
    probeUrl,
    saveCredential,
  } = useSourceCredentials();
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [probePlatformId, setProbePlatformId] = useState<string>();

  const platforms = useMemo(() => data?.platforms ?? [], [data]);

  const save = async (platform: SourcePlatformCredential) => {
    if (!platform.cookieDomain) {
      return;
    }

    const value = drafts[platform.id] ?? "";
    const saved = await saveCredential(platform.cookieDomain, value);
    if (saved) {
      setDrafts((current) => ({ ...current, [platform.id]: "" }));
    }
  };

  const remove = async (platform: SourcePlatformCredential) => {
    if (!platform.cookieDomain) {
      return;
    }
    await deleteCredential(platform.cookieDomain);
  };

  return (
    <div className="mx-auto max-w-5xl p-8">
      <div className="flex items-start justify-between gap-5">
        <div>
          <h2 className="text-xl font-semibold">{t("Sources")}</h2>
          <p className="mt-2 max-w-2xl text-sm leading-6 text-muted-foreground">
            {t(
              "Cookies let the source adapters read content that requires a signed-in session. They are stored in the system secret store, are never shown again after saving, and never leave the machine.",
            )}
          </p>
        </div>
        <Button variant="ghost" onClick={() => void load()} disabled={loading}>
          <RefreshCw className={loading ? "h-4 w-4 animate-spin" : "h-4 w-4"} aria-hidden="true" />
          {t("Refresh")}
        </Button>
      </div>

      {error ? (
        <div className="mt-6 rounded-md border border-red-200 bg-red-50 p-3 text-sm text-red-800 dark:border-red-900 dark:bg-red-950/30 dark:text-red-200">
          <p className="font-medium">{t(error.title)}</p>
          <p className="mt-1">{t(error.message)}</p>
        </div>
      ) : null}

      <div className="mt-6 space-y-3">
        {platforms.map((platform) => (
          <SourcePlatformRow
            key={platform.id}
            platform={platform}
            draft={drafts[platform.id] ?? ""}
            saving={saving}
            onDraftChange={(value) =>
              setDrafts((current) => ({ ...current, [platform.id]: value }))
            }
            onSave={() => void save(platform)}
            onRemove={() => void remove(platform)}
            onProbe={(url) => {
              setProbePlatformId(platform.id);
              void probeUrl(url);
            }}
            probing={probing}
            probe={probePlatformId === platform.id ? probe : undefined}
            t={t}
          />
        ))}
        {!loading && platforms.length === 0 && !error ? (
          <p className="text-sm text-muted-foreground">{t("No source adapters are available.")}</p>
        ) : null}
      </div>

      {probeError ? (
        <div className="mt-6 rounded-md border border-red-200 bg-red-50 p-3 text-sm text-red-800 dark:border-red-900 dark:bg-red-950/30 dark:text-red-200">
          <p className="font-medium">{t(probeError.title)}</p>
          <p className="mt-1">{t(probeError.message)}</p>
        </div>
      ) : null}
    </div>
  );
}

interface SourcePlatformRowProps {
  platform: SourcePlatformCredential;
  draft: string;
  saving: boolean;
  probing: boolean;
  probe?: SourceProbeResponse;
  t: Translator;
  onDraftChange: (value: string) => void;
  onSave: () => void;
  onRemove: () => void;
  onProbe: (url: string) => void;
}

function SourcePlatformRow({
  platform,
  draft,
  saving,
  probing,
  probe,
  t,
  onDraftChange,
  onSave,
  onRemove,
  onProbe,
}: SourcePlatformRowProps) {
  const [probeUrl, setProbeUrl] = useState("");

  return (
    <section className="rounded-lg border border-border bg-surface p-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <h3 className="text-sm font-semibold">{platform.label}</h3>
          <p className="mt-1 text-xs text-muted-foreground">
            {platform.cookieDomain
              ? t("Cookie domain: {domain}", { domain: platform.cookieDomain })
              : t("No cookie required for this platform.")}
          </p>
        </div>
        <CredentialStatus platform={platform} t={t} />
      </div>

      {platform.cookieDomain ? (
        <div className="mt-4 space-y-3">
          <label className="block text-xs font-medium text-muted-foreground">
            {t("Paste the Cookie request header from a signed-in browser session")}
            <input
              aria-label={t("Cookie for {platform}", { platform: platform.label })}
              className="mt-1 w-full rounded-md border border-border bg-background px-3 py-2 font-mono text-xs"
              onChange={(event) => onDraftChange(event.target.value)}
              placeholder="a1=…; web_session=…"
              type="password"
              value={draft}
            />
          </label>
          <div className="flex flex-wrap gap-2">
            <Button
              aria-label={t("Save cookie for {platform}", { platform: platform.label })}
              className="h-8 px-2 text-xs"
              disabled={saving || draft.trim().length === 0}
              onClick={onSave}
            >
              <KeyRound className="h-4 w-4" aria-hidden="true" />
              {t("Save cookie")}
            </Button>
            {platform.configured ? (
              <Button
                aria-label={t("Remove cookie for {platform}", { platform: platform.label })}
                className="h-8 px-2 text-xs"
                disabled={saving}
                onClick={onRemove}
                variant="ghost"
              >
                <Trash2 className="h-4 w-4" aria-hidden="true" />
                {t("Remove cookie")}
              </Button>
            ) : null}
          </div>
        </div>
      ) : null}

      <div className="mt-4 border-t border-border pt-3">
        <label className="block text-xs font-medium text-muted-foreground">
          {t("Check a link to prove the adapter still works")}
          <input
            aria-label={t("Sample link for {platform}", { platform: platform.label })}
            className="mt-1 w-full rounded-md border border-border bg-background px-3 py-2 text-xs"
            onChange={(event) => setProbeUrl(event.target.value)}
            placeholder="https://…"
            value={probeUrl}
          />
        </label>
        <Button
          aria-label={t("Check link for {platform}", { platform: platform.label })}
          className="mt-2 h-8 px-2 text-xs"
          disabled={probing || probeUrl.trim().length === 0}
          onClick={() => onProbe(probeUrl.trim())}
          variant="secondary"
        >
          <ShieldCheck className="h-4 w-4" aria-hidden="true" />
          {t("Check link")}
        </Button>
        {probe ? <ProbeResult probe={probe} t={t} /> : null}
      </div>
    </section>
  );
}

function CredentialStatus({
  platform,
  t,
}: {
  platform: SourcePlatformCredential;
  t: Translator;
}) {
  if (!platform.cookieDomain) {
    return (
      <span className="rounded-sm bg-muted px-2 py-1 text-[11px] text-muted-foreground">
        {t("Public access")}
      </span>
    );
  }

  if (!platform.configured) {
    return (
      <span className="rounded-sm bg-amber-100 px-2 py-1 text-[11px] text-amber-900 dark:bg-amber-900/30 dark:text-amber-200">
        {t("Cookie missing")}
      </span>
    );
  }

  const checked = platform.updatedAt
    ? t("Saved {when}", { when: new Date(platform.updatedAt).toLocaleString() })
    : t("Cookie saved");

  return (
    <span className="rounded-sm bg-emerald-100 px-2 py-1 text-[11px] text-emerald-900 dark:bg-emerald-900/30 dark:text-emerald-200">
      {checked}
    </span>
  );
}

function ProbeResult({ probe, t }: { probe: SourceProbeResponse; t: Translator }) {
  const tone =
    probe.status === "ok"
      ? "border-emerald-200 bg-emerald-50 text-emerald-900 dark:border-emerald-900 dark:bg-emerald-950/30 dark:text-emerald-200"
      : "border-amber-200 bg-amber-50 text-amber-900 dark:border-amber-900 dark:bg-amber-950/30 dark:text-amber-200";

  return (
    <div className={`mt-3 rounded-md border p-3 text-xs leading-5 ${tone}`}>
      <p className="font-medium">{probeLabel(probe, t)}</p>
      {probe.status === "ok" ? (
        <p className="mt-1">
          {t("{title} · {count} media · {chars} characters", {
            title: probe.title ?? t("Untitled"),
            count: probe.assets.length,
            chars: probe.textLength,
          })}
        </p>
      ) : null}
      {probe.message ? <p className="mt-1 font-mono text-[11px]">{probe.message}</p> : null}
    </div>
  );
}

function probeLabel(probe: SourceProbeResponse, t: Translator) {
  switch (probe.status) {
    case "ok":
      return t("Adapter works");
    case "login_required":
      return t("Cookie is missing or expired");
    case "structure_changed":
      return t("The platform changed its payload");
    case "network_unreachable":
      return t("This machine cannot reach the platform");
    case "no_adapter":
      return t("No adapter for this link");
    default:
      return t("Check failed");
  }
}
