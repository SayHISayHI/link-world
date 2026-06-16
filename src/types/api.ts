export type IpcErrorCode =
  | "ERR_DB_CONSTRAINT"
  | "ERR_DB_MIGRATION"
  | "ERR_NETWORK_TIMEOUT"
  | "ERR_PARSE_FAILED"
  | "ERR_MODEL_AUTH"
  | "ERR_MODEL_RATE_LIMIT"
  | "ERR_MODEL_OUTPUT_SCHEMA"
  | "ERR_POLICY_DENIED"
  | "ERR_PLUGIN_PERMISSION"
  | "ERR_JOB_NOT_FOUND"
  | "ERR_OBJECT_NOT_FOUND"
  | "ERR_SECRET_STORAGE"
  | "ERR_UNKNOWN";

export interface IpcError {
  code: IpcErrorCode;
  message: string;
  details?: string;
}

export interface IpcResponse<T> {
  status: "success" | "error";
  data?: T;
  error?: IpcError;
}

export interface PingResponse {
  message: string;
  backendVersion: string;
}

export type ObjectLifecycle =
  | "captured"
  | "parsed"
  | "enriched"
  | "evaluated"
  | "triaged"
  | "archived"
  | "deleted"
  | "failed";

export interface KnowledgeObjectSummary {
  id: string;
  title: string;
  type: "article" | "github_repo" | "prompt";
  lifecycleStatus: ObjectLifecycle;
  summary?: string;
}

