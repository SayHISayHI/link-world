import { useState, type FormEvent } from "react";
import { Check, Inbox, Plus, Tag, X } from "lucide-react";
import type { AppUiError } from "../../lib/errors";
import type { NavigationItem, ObjectOrganization } from "../../types/api";
import { Button } from "../ui/button";

interface OrganizationPanelProps {
  organization?: ObjectOrganization;
  collections: NavigationItem[];
  loading: boolean;
  mutationLoading: boolean;
  error?: AppUiError;
  onMarkFiled: (filed: boolean) => void;
  onToggleCollection: (collectionId: string, selected: boolean) => void;
  onAddTag: (name: string) => void;
  onRemoveTag: (tagId: string) => void;
  onAcceptSuggestion: (suggestionId: string) => void;
  onRejectSuggestion: (suggestionId: string) => void;
}

export function OrganizationPanel({
  organization,
  collections,
  loading,
  mutationLoading,
  error,
  onMarkFiled,
  onToggleCollection,
  onAddTag,
  onRemoveTag,
  onAcceptSuggestion,
  onRejectSuggestion,
}: OrganizationPanelProps) {
  const [tagName, setTagName] = useState("");
  const selectedCollections = new Set(
    organization?.collections.map((collection) => collection.id) ?? [],
  );

  const submitTag = (event: FormEvent) => {
    event.preventDefault();
    const name = tagName.trim();
    if (!name) return;
    onAddTag(name);
    setTagName("");
  };

  return (
    <section className="mt-4 border-t border-border pt-4">
      <div className="flex items-center justify-between gap-3">
        <h3 className="text-sm font-semibold">Organization</h3>
        {organization ? (
          <Button
            variant="ghost"
            className="h-8 px-2 text-xs"
            disabled={mutationLoading}
            onClick={() => onMarkFiled(organization.triageStatus !== "filed")}
            title={organization.triageStatus === "filed" ? "Move back to Inbox" : "Mark as filed"}
          >
            {organization.triageStatus === "filed" ? (
              <Inbox className="h-4 w-4" aria-hidden="true" />
            ) : (
              <Check className="h-4 w-4" aria-hidden="true" />
            )}
            {organization.triageStatus === "filed" ? "Inbox" : "File"}
          </Button>
        ) : null}
      </div>

      {loading && !organization ? (
        <p className="mt-3 text-xs text-muted-foreground">Loading organization...</p>
      ) : null}
      {error ? (
        <div className="mt-3 border-l-2 border-red-400 pl-2 text-xs text-red-700">
          <p className="font-medium">Organization update failed</p>
          <p className="mt-1">{error.message}</p>
        </div>
      ) : null}

      {organization ? (
        <div className="mt-3 space-y-4 text-xs">
          <div>
            <p className="font-medium text-foreground">Collections</p>
            {collections.length ? (
              <div className="mt-2 max-h-32 space-y-1 overflow-y-auto">
                {collections.map((collection) => {
                  const checked = selectedCollections.has(collection.id);
                  return (
                    <label
                      key={collection.id}
                      className="flex min-h-7 cursor-pointer items-center gap-2 text-muted-foreground hover:text-foreground"
                    >
                      <input
                        type="checkbox"
                        checked={checked}
                        disabled={mutationLoading}
                        onChange={() => onToggleCollection(collection.id, !checked)}
                        className="h-3.5 w-3.5 accent-[hsl(var(--accent))]"
                      />
                      <span className="min-w-0 flex-1 truncate">{collection.label}</span>
                    </label>
                  );
                })}
              </div>
            ) : (
              <p className="mt-1 text-muted-foreground">Create a collection from the sidebar.</p>
            )}
          </div>

          <div>
            <p className="font-medium text-foreground">Topics</p>
            <div className="mt-2 flex flex-wrap gap-1.5">
              {organization.tags.map((tag) => (
                <span
                  key={tag.id}
                  className="inline-flex min-h-7 items-center gap-1 rounded-sm border border-border bg-muted px-2 text-muted-foreground"
                >
                  <Tag className="h-3 w-3" aria-hidden="true" />
                  {tag.name}
                  <button
                    type="button"
                    disabled={mutationLoading}
                    onClick={() => onRemoveTag(tag.id)}
                    className="ml-0.5 text-muted-foreground hover:text-red-700"
                    title={`Remove ${tag.name}`}
                    aria-label={`Remove ${tag.name}`}
                  >
                    <X className="h-3 w-3" aria-hidden="true" />
                  </button>
                </span>
              ))}
              {organization.tags.length === 0 ? (
                <span className="text-muted-foreground">No accepted topics</span>
              ) : null}
            </div>
            <form className="mt-2 flex gap-1" onSubmit={submitTag}>
              <input
                value={tagName}
                maxLength={80}
                onChange={(event) => setTagName(event.target.value)}
                className="h-8 min-w-0 flex-1 rounded-sm border border-border bg-surface px-2 outline-none focus:border-accent"
                placeholder="Add topic"
                aria-label="Add topic"
              />
              <button
                type="submit"
                disabled={mutationLoading || !tagName.trim()}
                className="flex h-8 w-8 items-center justify-center rounded-sm border border-border text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-40"
                title="Add topic"
                aria-label="Add topic"
              >
                <Plus className="h-4 w-4" aria-hidden="true" />
              </button>
            </form>
          </div>

          {organization.tagSuggestions.length ? (
            <div>
              <p className="font-medium text-foreground">AI suggestions</p>
              <div className="mt-2 space-y-2">
                {organization.tagSuggestions.map((suggestion) => (
                  <div key={suggestion.id} className="border-l-2 border-violet-300 pl-2">
                    <div className="flex items-center gap-2">
                      <span className="min-w-0 flex-1 font-medium text-foreground">
                        {suggestion.name}
                      </span>
                      {suggestion.confidence !== undefined ? (
                        <span className="text-[11px] tabular-nums text-muted-foreground">
                          {Math.round(suggestion.confidence * 100)}%
                        </span>
                      ) : null}
                      <button
                        type="button"
                        disabled={mutationLoading}
                        onClick={() => onAcceptSuggestion(suggestion.id)}
                        className="text-accent hover:text-foreground"
                        title={`Accept ${suggestion.name}`}
                        aria-label={`Accept ${suggestion.name}`}
                      >
                        <Check className="h-4 w-4" aria-hidden="true" />
                      </button>
                      <button
                        type="button"
                        disabled={mutationLoading}
                        onClick={() => onRejectSuggestion(suggestion.id)}
                        className="text-muted-foreground hover:text-red-700"
                        title={`Reject ${suggestion.name}`}
                        aria-label={`Reject ${suggestion.name}`}
                      >
                        <X className="h-4 w-4" aria-hidden="true" />
                      </button>
                    </div>
                    {suggestion.rationale ? (
                      <p className="mt-1 leading-4 text-muted-foreground">{suggestion.rationale}</p>
                    ) : null}
                  </div>
                ))}
              </div>
            </div>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
