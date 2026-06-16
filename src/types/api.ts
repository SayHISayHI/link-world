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

export type PrivacyLevel = "public" | "personal" | "sensitive" | "secret";

export type CaptureSource =
  | "url"
  | "dom"
  | "selection"
  | "screenshot"
  | "file"
  | "api"
  | "import"
  | "local_automation";

export interface PermissionContext {
  acquisitionMode: "user_action" | "official_api" | "import" | "local_automation";
  userConfirmed: boolean;
  platformTermsHint?: string;
  allowedForCloudProcessing: boolean;
  allowedForThirdPartyAI: boolean;
}

export interface CaptureAsset {
  id: string;
  kind: "image" | "video" | "audio" | "file" | "snapshot";
  mimeType: string;
  uri: string;
  sha256?: string;
}

export interface RawCaptureItem {
  id?: string;
  userId?: string;
  sourceType: CaptureSource;
  sourcePlatform?: string;
  sourceUrl?: string;
  canonicalUrl?: string;
  title?: string;
  author?: string;
  capturedAt?: string;
  rawHtml?: string;
  rawText?: string;
  assets?: CaptureAsset[];
  metadata: Record<string, unknown>;
  privacyLevel: PrivacyLevel;
  permissionContext: PermissionContext;
}

export interface SubmitCaptureResponse {
  objectId: string;
  snapshotId: string;
  parsedDocumentId?: string;
  jobId: string;
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

export type KnowledgeObjectType =
  | "article"
  | "social_post"
  | "thread"
  | "prompt"
  | "github_repo"
  | "tool"
  | "tutorial"
  | "paper"
  | "video"
  | "podcast"
  | "conversation"
  | "note"
  | "dataset"
  | "file"
  | "collection";

export interface KnowledgeObject {
  id: string;
  userId: string;
  type: KnowledgeObjectType;
  title?: string;
  canonicalUrl?: string;
  sourcePlatform?: string;
  author?: string;
  privacyLevel: PrivacyLevel;
  lifecycleStatus: ObjectLifecycle;
  failureReason?: string;
  capturedAt: string;
  updatedAt: string;
}

export interface SourceSnapshot {
  id: string;
  objectId: string;
  snapshotType: "html" | "markdown" | "text" | "pdf_blob" | "json" | "screenshot" | string;
  storageUri: string;
  contentHash: string;
  parserId?: string;
  parserVersion?: string;
  capturedAt: string;
}

export interface ParsedDocument {
  id: string;
  objectId: string;
  sourceSnapshotId?: string;
  title?: string;
  text: string;
  markdown?: string;
  language?: string;
  wordCount?: number;
  parserId: string;
  parserVersion: string;
  contentHash: string;
  createdAt: string;
}

export interface AITrace {
  provider: string;
  model: string;
  capability: "chat" | "embedding" | "rerank" | "vision" | string;
  promptTemplateId?: string;
  promptTemplateVersion?: string;
  inputSnapshotId?: string;
  inputParsedDocumentId?: string;
  inputHash?: string;
  outputHash?: string;
  promptTokens?: number;
  completionTokens?: number;
  estimatedCostUsd?: number;
  latencyMs?: number;
}

export interface AIAnalysis {
  id: string;
  objectId: string;
  parsedDocumentId?: string;
  analysisType: string;
  schemaVersion: number;
  summary?: string;
  category?: string;
  tags: string[];
  keyPoints: unknown[];
  claims: unknown[];
  actionItems: unknown[];
  risks: unknown[];
  qualityScore?: number;
  confidence?: number;
  trace?: AITrace;
  createdAt: string;
}

export interface EvaluationArtifact {
  kind: string;
  uri: string;
  metadata?: unknown;
}

export interface EvaluationRun {
  id: string;
  objectId: string;
  evaluatorType: string;
  evaluatorVersion: string;
  status: "planned" | "running" | "passed" | "failed" | "skipped" | "blocked" | string;
  score?: number;
  verdict: "high_value" | "useful" | "situational" | "low_value" | "unsafe" | "unknown" | string;
  dimensions: unknown;
  evidence: unknown[];
  artifacts: EvaluationArtifact[];
  limitations: string[];
  nextActions: unknown[];
  failureReason?: string;
  createdAt: string;
  completedAt?: string;
}

export interface KnowledgeObjectDetail {
  object: KnowledgeObject;
  parsedDocument?: ParsedDocument;
  snapshots: SourceSnapshot[];
  aiAnalyses: AIAnalysis[];
  evaluations: EvaluationRun[];
}
