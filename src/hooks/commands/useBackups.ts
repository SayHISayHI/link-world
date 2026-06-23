import { useCallback, useState } from "react";
import type { AppUiError } from "../../lib/errors";
import { invokeCommand } from "../../lib/tauri";
import type { BackupSummary, BackupVerification } from "../../types/api";

interface BackupState {
  backups: BackupSummary[];
  creating: boolean;
  error?: AppUiError;
  loading: boolean;
  verificationById: Record<string, BackupVerification>;
  verifyingId?: string;
}

const initialState: BackupState = {
  backups: [],
  creating: false,
  loading: false,
  verificationById: {},
};

export function useBackups() {
  const [state, setState] = useState<BackupState>(initialState);

  const loadBackups = useCallback(async () => {
    setState((current) => ({ ...current, error: undefined, loading: true }));
    try {
      const backups = await invokeCommand<Record<string, never>, BackupSummary[]>(
        "list_backups",
        {},
      );
      setState((current) => ({ ...current, backups, loading: false }));
      return backups;
    } catch (error) {
      setState((current) => ({
        ...current,
        error: error as AppUiError,
        loading: false,
      }));
      return undefined;
    }
  }, []);

  const createBackup = useCallback(async () => {
    setState((current) => ({ ...current, creating: true, error: undefined }));
    try {
      const backup = await invokeCommand<Record<string, never>, BackupSummary>(
        "create_backup",
        {},
      );
      setState((current) => ({
        ...current,
        backups: [backup, ...current.backups.filter((item) => item.backupId !== backup.backupId)],
        creating: false,
      }));
      return backup;
    } catch (error) {
      setState((current) => ({
        ...current,
        creating: false,
        error: error as AppUiError,
      }));
      return undefined;
    }
  }, []);

  const verifyBackup = useCallback(async (backupId: string) => {
    setState((current) => ({
      ...current,
      error: undefined,
      verifyingId: backupId,
    }));
    try {
      const verification = await invokeCommand<
        { backupId: string },
        BackupVerification
      >("verify_backup", { backupId });
      setState((current) => ({
        ...current,
        verificationById: {
          ...current.verificationById,
          [backupId]: verification,
        },
        verifyingId: undefined,
      }));
      return verification;
    } catch (error) {
      setState((current) => ({
        ...current,
        error: error as AppUiError,
        verifyingId: undefined,
      }));
      return undefined;
    }
  }, []);

  return {
    ...state,
    createBackup,
    loadBackups,
    verifyBackup,
  };
}
