import { useCallback, useState } from "react";
import type { AppUiError } from "../../lib/errors";
import { invokeCommand } from "../../lib/tauri";
import type {
  CreateCollectionInput,
  CreateSmartViewInput,
  KnowledgeCollection,
  KnowledgeTag,
  UpdateCollectionInput,
} from "../../types/api";

interface MutationState {
  error?: AppUiError;
  loading: boolean;
}

export function useOrganizationMutations() {
  const [state, setState] = useState<MutationState>({ loading: false });

  const run = useCallback(async <Args extends object, Result>(command: string, args: Args) => {
    setState({ loading: true });
    try {
      const result = await invokeCommand<Args, Result>(command, args);
      setState({ loading: false });
      return result;
    } catch (error) {
      setState({ error: error as AppUiError, loading: false });
      return undefined;
    }
  }, []);

  return {
    ...state,
    reset: () => setState({ loading: false }),
    createCollection: (input: CreateCollectionInput) =>
      run<{ input: CreateCollectionInput }, KnowledgeCollection>("create_collection", { input }),
    createSmartView: (input: CreateSmartViewInput) =>
      run<{ input: CreateSmartViewInput }, KnowledgeCollection>("create_smart_view", { input }),    updateCollection: (input: UpdateCollectionInput) =>
      run<{ input: UpdateCollectionInput }, KnowledgeCollection>("update_collection", { input }),
    archiveCollection: (collectionId: string) =>
      run<{ collectionId: string }, boolean>("archive_collection", { collectionId }),
    addObjectToCollection: (objectId: string, collectionId: string) =>
      run<{ objectId: string; collectionId: string }, boolean>("add_object_to_collection", {
        objectId,
        collectionId,
      }),
    removeObjectFromCollection: (objectId: string, collectionId: string) =>
      run<{ objectId: string; collectionId: string }, boolean>("remove_object_from_collection", {
        objectId,
        collectionId,
      }),
    markObjectTriaged: (objectId: string, filed: boolean) =>
      run<{ objectId: string; filed: boolean }, boolean>("mark_object_triaged", {
        objectId,
        filed,
      }),
    addUserTag: (objectId: string, name: string) =>
      run<{ objectId: string; name: string }, KnowledgeTag>("add_user_tag", { objectId, name }),
    removeObjectTag: (objectId: string, tagId: string) =>
      run<{ objectId: string; tagId: string }, boolean>("remove_object_tag", {
        objectId,
        tagId,
      }),
    acceptTagSuggestion: (suggestionId: string) =>
      run<{ suggestionId: string }, KnowledgeTag>("accept_tag_suggestion", { suggestionId }),
    rejectTagSuggestion: (suggestionId: string) =>
      run<{ suggestionId: string }, boolean>("reject_tag_suggestion", { suggestionId }),
  };
}
