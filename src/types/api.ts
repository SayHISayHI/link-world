export type IpcErrorCode =
  | "ERR_BACKUP_INVALID"
  | "ERR_RESTORE_INVALID"
  | "ERR_DB_CONSTRAINT"
  | "ERR_DB_MIGRATION"
  | "ERR_NETWORK_TIMEOUT"
  | "ERR_PARSE_FAILED"
  | "ERR_MODEL_AUTH"
  | "ERR_MODEL_RATE_LIMIT"
  | "ERR_MODEL_NOT_FOUND"
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

export type StartupMode = "ready" | "recovery";

export type StartupRecoveryKind =
  | "database_migration"
  | "restore"
  | "database"
  | "storage"
  | "unknown";

export interface StartupMigrationRecovery {
  phase: "prepared" | "running" | string;
  backupId?: string;
  fromVersion?: number;
  targetVersion: number;
  appVersion: string;
  createdAt: string;
}

export interface StartupIssue {
  code: IpcErrorCode;
  title: string;
  message: string;
  recoveryKind: StartupRecoveryKind;
  verifiedBackupId?: string;
  migration?: StartupMigrationRecovery;
}

export interface StartupStatus {
  mode: StartupMode;
  backendVersion: string;
  issue?: StartupIssue;
}

export interface BackupSummary {
  backupId: string;
  appVersion?: string;
  createdAt?: string;
  objectFileCount: number;
  totalSizeBytes: number;
  status: "ready" | "invalid" | string;
}

export interface BackupVerification {
  backupId: string;
  valid: boolean;
  checkedFileCount: number;
  issues: string[];
}
export interface RestorePreparation {
  backupId: string;
  safetyBackupId: string;
  restartRequired: boolean;
}

export interface RestoreStatus {
  backupId: string;
  safetyBackupId: string;
  status: "succeeded" | "rolled_back" | "failed" | string;
  completedAt: string;
  message?: string;
}

export interface PortableExportSummary {
  exportId: string;
  exportRoot: string;
  format: string;
  objectCount: number;
  skippedSecretCount: number;
  markdownFileCount: number;
  jsonFileCount: number;
  createdAt: string;
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
  snapshotId?: string;
  parsedDocumentId?: string;
  jobId?: string;
  deduplicated: boolean;
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

export type DocumentDisplayMode = "article" | "tutorial" | "reference" | "code-heavy";

export interface AIDisplayHintsV1 {
  schemaVersion: 1;
  mode: DocumentDisplayMode;
  confidence: number;
  reason?: string;
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
  displayHints?: AIDisplayHintsV1;
  trace?: AITrace;
  createdAt: string;
}

export type ModelApiFamily =
  | "openai_chat_completions"
  | "openai_responses"
  | "anthropic_messages"
  | "google_generative_ai"
  | "ollama";

export interface ModelProviderConfig {
  id?: string;
  provider: string;
  apiFamily: ModelApiFamily;
  chatBaseUrl?: string;
  embeddingsBaseUrl?: string;
  apiKey?: string;
  defaultChatModel?: string;
  defaultEmbeddingModel?: string;
  capabilities: Array<"chat" | "embedding" | "rerank" | "vision" | string>;
  enabled?: boolean;
}

export interface ModelProviderConfigView
  extends Omit<ModelProviderConfig, "apiKey" | "enabled" | "id"> {
  id: string;
  hasApiKey: boolean;
  enabled: boolean;
  isDefault: boolean;
}

export interface ModelProviderTestResult {
  provider: string;
  apiFamily: ModelApiFamily;
  model: string;
  latencyMs: number;
}

export interface AIEnrichmentRunResult {
  jobId: string;
  correlationId: string;
  analysisId?: string;
  status: "succeeded" | "failed" | string;
  failureReason?: string;
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

export interface TriggerEvaluationResponse {
  runId: string;
}

export interface KnowledgeObjectDetail {
  object: KnowledgeObject;
  parsedDocument?: ParsedDocument;
  snapshots: SourceSnapshot[];
  aiAnalyses: AIAnalysis[];
  evaluations: EvaluationRun[];
}

export interface SearchResult {
  object: KnowledgeObject;
  matchedFields: string[];
  snippet?: string;
  score: number;
}

export interface RebuildSearchIndexResponse {
  jobId: string;
  status: JobStatus;
  stage: "queued" | "preparing" | "indexing" | "finalizing" | "completed" | "cancelled" | "failed" | string;
  expectedObjects: number;
  indexedObjects: number;
  progressPercent: number;
  cancellable: boolean;
  failureReason?: string;
}

export interface SearchIndexHealthResponse {
  healthy: boolean;
  expectedIndexedObjects: number;
  actualIndexedRows: number;
  missingObjects: number;
  staleObjects: number;
  orphanedRows: number;
  duplicateRows: number;
  missingObjectIds: string[];
  staleObjectIds: string[];
  orphanedObjectIds: string[];
  duplicateObjectIds: string[];
}

export interface ReindexObjectResponse {
  jobId: string;
  objectId: string;
  indexed: boolean;
}

export type JobStatus = "queued" | "running" | "succeeded" | "failed" | "cancelled" | "blocked";

export interface BackgroundJob {
  id: string;
  type:
    | "capture.fetch_url"
    | "parser.extract_document"
    | "ai.enrich_object"
    | "embedding.create_chunks"
    | "evaluation.run"
    | "search.rebuild_index"
    | "search.reindex_object"
    | "review.schedule_object"
    | "storage.purge_deleted_object"
    | "sync.push_changes"
    | "sync.pull_changes"
    | string;
  status: JobStatus;
  objectId?: string;
  attemptCount: number;
  maxAttempts: number;
  nextRunAt?: string;
  lastError?: string;
  createdAt: string;
  updatedAt: string;
}

export interface DatabaseHealth {
  healthy: boolean;
  quickCheck: string;
  foreignKeyViolations: number;
  appliedMigrationVersion?: number;
  sizeBytes?: number;
}

export interface ObjectStoreHealth {
  healthy: boolean;
  sizeBytes?: number;
  fileCount: number;
  issue?: string;
}

export interface FailedJobSummary {
  jobId: string;
  jobType: string;
  status: JobStatus;
  objectId?: string;
  lastError?: string;
  updatedAt: string;
}

export interface JobMetrics {
  queued: number;
  running: number;
  failed: number;
  blocked: number;
  cancelled: number;
  recentFailures: FailedJobSummary[];
}

export interface ModelMetrics {
  configuredCount: number;
  enabledCount: number;
  defaultChatConfigured: boolean;
  status: "configured" | "not_configured_normal_degradation" | "missing_default_chat_config" | string;
}

export interface DiagnosticsPrivacySummary {
  supportBundleAvailable: boolean;
  redaction: string[];
}

export interface LocalMetricsSnapshot {
  appVersion: string;
  dataDir: string;
  databasePath: string;
  objectStorePath: string;
  databaseHealth: DatabaseHealth;
  objectStoreHealth: ObjectStoreHealth;
  jobs: JobMetrics;
  models: ModelMetrics;
  privacy: DiagnosticsPrivacySummary;
}

export interface SupportBundleSummary {
  bundleId: string;
  createdAt: string;
  filePath: string;
  sizeBytes: number;
  sha256: string;
  includedSections: string[];
  redaction: string[];
}

export type DeleteObjectMode = "soft_delete" | "purge" | "export_then_delete";

export interface DeleteObjectResponse {
  jobId?: string;
}
