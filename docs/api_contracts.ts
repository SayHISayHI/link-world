/**
 * Link World - Tauri IPC 接口契约与数据结构规范
 *
 * 此文件为文档约束文件，并非运行代码。
 * 目的：前端 (React) 与后端 (Rust) 必须遵循此处的接口签名进行通信。
 * 所有 Tauri 命令需通过 invoke('command_name', args) 调用。
 */

// ==========================================
// 1. 核心枚举与数据结构
// ==========================================

export type PrivacyLevel = 'public' | 'personal' | 'sensitive' | 'secret';

export type ObjectLifecycle =
  | 'captured'
  | 'parsed'
  | 'enriched'
  | 'evaluated'
  | 'triaged'
  | 'archived'
  | 'deleted'
  | 'failed';

export type KnowledgeObjectType =
  | 'article'
  | 'social_post'
  | 'thread'
  | 'prompt'
  | 'github_repo'
  | 'tool'
  | 'tutorial'
  | 'paper'
  | 'video'
  | 'podcast'
  | 'conversation'
  | 'note'
  | 'dataset'
  | 'file'
  | 'collection';

export type CaptureSource =
  | 'url'
  | 'dom'
  | 'selection'
  | 'screenshot'
  | 'file'
  | 'api'
  | 'import'
  | 'local_automation';

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
  // Present for newly stored captures and best-effort for deduplicated existing objects.
  snapshotId?: string;
  parsedDocumentId?: string;
  // Present only when a concrete background job exists; omitted for duplicate URL submissions without a job.
  jobId?: string;
  // true means the backend selected an existing active object for the same normalized canonical URL.
  deduplicated: boolean;
}

export interface CaptureAsset {
  id: string;
  kind: 'image' | 'video' | 'audio' | 'file' | 'snapshot';
  mimeType: string;
  uri: string;
  sha256?: string;
}

export interface PermissionContext {
  acquisitionMode: 'user_action' | 'official_api' | 'import' | 'local_automation';
  userConfirmed: boolean;
  platformTermsHint?: string;
  allowedForCloudProcessing: boolean;
  allowedForThirdPartyAI: boolean;
}

export interface SourceSnapshot {
  id: string;
  objectId: string;
  snapshotType: 'html' | 'markdown' | 'text' | 'pdf_blob' | 'json' | 'screenshot';
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

export type DocumentDisplayMode = 'article' | 'tutorial' | 'reference' | 'code-heavy';

export interface AIDisplayHintsV1 {
  schemaVersion: 1;
  mode: DocumentDisplayMode;
  confidence: number;
  reason?: string; // normalized to at most 160 characters by the backend
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

export interface AITrace {
  provider: string;
  model: string;
  capability: 'chat' | 'embedding' | 'rerank' | 'vision';
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

export interface EvaluatorCapability {
  schemaVersion: 1;
  evaluatorType: string;
  evaluatorVersion: string;
  displayName: string;
  supportedObjectTypes: string[];
  executionKind: 'local_deterministic' | 'model_assisted' | 'sandboxed' | string;
  requiresNetwork: boolean;
  requiresModel: boolean;
  requiresSandbox: boolean;
  planSchemaVersion: 1;
  inputSchemaVersion: 1;
  outputSchemaVersion: 1;
}
export interface EvaluationRun {
  id: string;
  requestId?: string;
  correlationId?: string;
  objectId: string;
  evaluatorType: string;
  evaluatorVersion: string;
  planSchemaVersion: number;
  inputSchemaVersion: number;
  outputSchemaVersion: number;
  status: 'planned' | 'running' | 'passed' | 'failed' | 'skipped' | 'blocked';
  score?: number;
  verdict: 'high_value' | 'useful' | 'situational' | 'low_value' | 'unsafe' | 'unknown';
  dimensions: Record<string, number>;
  evidence: EvidenceItem[];
  artifacts: EvaluationArtifact[];
  limitations: string[];
  nextActions: unknown[];
  failureReason?: string;
  createdAt: string;
  completedAt?: string;
}

export interface EvidenceItem {
  source: 'original_content' | 'internal_library' | 'external_check' | 'sandbox_run' | 'user_feedback';
  text: string;
  reference?: string;
}

export interface EvaluationArtifact {
  kind: 'log' | 'screenshot' | 'diff' | 'test_output' | 'generated_prompt' | 'report';
  uri: string;
  metadata?: Record<string, unknown>;
}

export interface SearchResult {
  object: KnowledgeObject;
  matchedFields: string[];
  // Omitted for secret objects even when the object itself is returned by local search.
  snippet?: string;
  score: number;
}

export interface SearchIndexHealthResponse {
  healthy: boolean;
  expectedIndexedObjects: number;
  actualIndexedRows: number;
  missingObjects: number;
  staleObjects: number;
  orphanedRows: number;
  duplicateRows: number;
  // Sample object ids only, capped by the backend; never includes content/snippets.
  missingObjectIds: string[];
  staleObjectIds: string[];
  orphanedObjectIds: string[];
  duplicateObjectIds: string[];
}

export interface RebuildSearchIndexResponse {
  jobId: string;
  status: JobStatus;
  stage: 'queued' | 'preparing' | 'indexing' | 'finalizing' | 'completed' | 'cancelled' | 'failed' | string;
  expectedObjects: number;
  indexedObjects: number;
  progressPercent: number;
  // false during finalizing because the staging FTS table is being atomically swapped into place.
  cancellable: boolean;
  failureReason?: string;
}

export type ModelApiFamily =
  | 'openai_chat_completions'
  | 'openai_responses'
  | 'anthropic_messages'
  | 'google_generative_ai'
  | 'ollama';

export interface ModelProviderConfig {
  id?: string;
  provider: string;
  apiFamily: ModelApiFamily;
  chatBaseUrl?: string;
  embeddingsBaseUrl?: string;
  apiKey?: string;
  defaultChatModel?: string;
  defaultEmbeddingModel?: string;
  capabilities: Array<'chat' | 'embedding' | 'rerank' | 'vision'>;
  enabled?: boolean;
}

export interface ModelProviderConfigView
  extends Omit<ModelProviderConfig, 'apiKey' | 'enabled' | 'id'> {
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
  status: 'succeeded' | 'failed' | string;
  // Failed AI jobs use stable `ai.*` prefixes such as ai.timeout, ai.model_auth,
  // ai.rate_limit, ai.output_schema, ai.not_configured and ai.policy_denied.
  // Text after the prefix is sanitized user-facing recovery guidance.
  failureReason?: string;
}
export type JobStatus = 'queued' | 'running' | 'succeeded' | 'failed' | 'cancelled' | 'blocked';

export interface BackgroundJob {
  id: string;
  type:
    | 'capture.fetch_url'
    | 'parser.extract_document'
    | 'ai.enrich_object'
    | 'embedding.create_chunks'
    | 'evaluation.run'
    | 'search.rebuild_index'
    | 'search.reindex_object'
    | 'review.schedule_object'
    | 'storage.purge_deleted_object'
    | 'sync.push_changes'
    | 'sync.pull_changes';
  status: JobStatus;
  objectId?: string;
  attemptCount: number;
  maxAttempts: number;
  nextRunAt?: string;
  // Failed jobs expose only stable recovery prefixes: `capture.*`, `ai.*`, `search.*` or `job.*`.
  // Raw provider/network/SQLite errors and content never cross IPC or support-bundle boundaries.
  lastError?: string;
  createdAt: string;
  updatedAt: string;
}

export interface DomainEvent<TPayload = unknown> {
  id: string;
  type:
    | 'capture.submitted'
    | 'snapshot.created'
    | 'object.parsed'
    | 'object.failed'
    | 'analysis.requested'
    | 'analysis.created'
    | 'analysis.failed'
    | 'evaluation.planned'
    | 'evaluation.completed'
    | 'evaluation.failed'
    | 'object.deleted'
    | 'privacy.changed'
    | 'plugin.permission.changed';
  version: number;
  userId: string;
  objectId?: string;
  occurredAt: string;
  causationId?: string;
  // Required for critical lifecycle events. Capture and AI jobs persist and reuse their submission UUID.
  correlationId?: string;
  payload: TPayload;
}

export interface PluginManifest {
  id: string;
  name: string;
  version: string;
  kind: 'connector' | 'parser' | 'evaluator' | 'model_provider' | 'sync_provider' | 'exporter';
  permissions: PluginPermission[];
  enabled: boolean;
}

export interface PluginPermission {
  kind:
    | 'network'
    | 'filesystem'
    | 'browser_automation'
    | 'read_public_objects'
    | 'read_personal_objects'
    | 'read_sensitive_objects'
    | 'call_third_party_ai';
  scope?: string;
  required: boolean;
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
  // Sanitized; URL query/fragment and credential references are redacted.
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
  // `not_configured_normal_degradation` is healthy for non-AI workflows.
  status: 'configured' | 'not_configured_normal_degradation' | 'missing_default_chat_config' | string;
}

export interface DiagnosticsPrivacySummary {
  supportBundleAvailable: boolean;
  redaction: string[];
}

export interface LocalMetricsSnapshot {
  appVersion: string;
  // Local display only. Exported support bundle files never contain these absolute paths.
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
  // Local-only response for opening the generated file; not embedded in the bundle JSON.
  filePath: string;
  sizeBytes: number;
  sha256: string;
  includedSections: string[];
  redaction: string[];
}

export interface BackupSummary {
  backupId: string;
  appVersion?: string;
  createdAt?: string;
  objectFileCount: number;
  totalSizeBytes: number;
  status: 'ready' | 'invalid';
}

export interface BackupVerification {
  backupId: string;
  valid: boolean;
  checkedFileCount: number;
  issues: string[];
}

export interface RestorePreparation {
  correlationId: string;
  backupId: string;
  safetyBackupId: string;
  restartRequired: boolean;
}

export interface RestoreStatus {
  correlationId?: string;
  backupId: string;
  safetyBackupId: string;
  status: 'succeeded' | 'rolled_back' | 'failed';
  completedAt: string;
  message?: string;
}

export interface PortableExportSummary {
  exportId: string;
  exportRoot: string;
  format: 'markdown_json_directory';
  objectCount: number;
  skippedSecretCount: number;
  markdownFileCount: number;
  jsonFileCount: number;
  createdAt: string;
}

export type StartupMode = 'ready' | 'recovery';

export type StartupRecoveryKind =
  | 'database_migration'
  | 'restore'
  | 'database'
  | 'storage'
  | 'unknown';

export interface StartupMigrationRecovery {
  phase: 'prepared' | 'running' | string;
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
// 标准的 IPC 响应包裹器
export interface IpcResponse<T> {
  status: 'success' | 'error';
  data?: T;
  error?: {
    code: IpcErrorCode;
    message: string;
    details?: string;
  };
}

export type IpcErrorCode =
  | 'ERR_BACKUP_INVALID'
  | 'ERR_RESTORE_INVALID'
  | 'ERR_DB_CONSTRAINT'
  | 'ERR_DB_MIGRATION'
  | 'ERR_NETWORK_TIMEOUT'
  | 'ERR_PARSE_FAILED'
  | 'ERR_MODEL_AUTH'
  | 'ERR_MODEL_RATE_LIMIT'
  | 'ERR_MODEL_NOT_FOUND'
  | 'ERR_MODEL_OUTPUT_SCHEMA'
  | 'ERR_POLICY_DENIED'
  | 'ERR_PLUGIN_PERMISSION'
  | 'ERR_JOB_NOT_FOUND'
  | 'ERR_OBJECT_NOT_FOUND'
  | 'ERR_SECRET_STORAGE'
  | 'ERR_UNKNOWN';

// ==========================================
// 2. 暴露给前端的 Tauri Commands 签名

/**
 * 模块：System / Startup
 */
export interface SystemCommands {
  // 查询当前进程是否完整 ready，或是否处于受限 startup recovery 模式。
  // invoke('get_startup_status')
  get_startup_status: () => Promise<IpcResponse<StartupStatus>>;

  // 安排应用重启；用于用户修复环境问题后重试启动，或 recovery 页面手动重启。
  restart_app: () => Promise<IpcResponse<boolean>>;

  // 普通 ready 模式下的后端健康检查；recovery UI 不依赖该命令。
  ping: () => Promise<IpcResponse<{ message: string; backendVersion: string }>>;
}
// ==========================================

/**
 * 模块：Capture (采集引擎)
 */
export interface CaptureCommands {
  // 提交采集请求。普通 URL capture 会创建对象和后台 job；重复 normalized canonical URL 会返回已有对象并设置 deduplicated=true。
  // invoke('submit_capture', { item: RawCaptureItem })
  submit_capture: (args: { item: RawCaptureItem }) => Promise<IpcResponse<SubmitCaptureResponse>>;
}

/**
 * 模块：Library (核心知识库查阅)
 */
export interface LibraryCommands {
  // 获取最近的知识对象列表 (支持分页与类型过滤)
  // invoke('get_recent_objects', { limit, offset, filterType })
  get_recent_objects: (args: {
    limit: number;
    offset: number;
    filterType?: KnowledgeObjectType | 'inbox' | 'failed';
  }) => Promise<IpcResponse<KnowledgeObject[]>>;

  // 获取对象的完整详情 (包含解析正文、AI 分析、快照和评估记录)
  // invoke('get_object_detail', { objectId })
  get_object_detail: (args: { objectId: string }) => Promise<IpcResponse<{
    object: KnowledgeObject;
    parsedDocument?: ParsedDocument;
    snapshots: SourceSnapshot[];
    aiAnalyses: AIAnalysis[];
    evaluations: EvaluationRun[];
  }>>;

  // 执行基于 SQLite FTS5 和可选向量索引的混合搜索；filterType 复用 Library 列表语义。
  // invoke('search_hybrid', { query, limit, filterType })
  search_hybrid: (args: {
    query: string;
    limit: number;
    filterType?: KnowledgeObjectType | 'inbox' | 'failed';
  }) => Promise<IpcResponse<SearchResult[]>>;
}

/**
 * 模块：Agent / Settings (AI 模型配置与管理)
 */
export interface AgentCommands {
  // 列出全部配置；只返回 hasApiKey 和 isDefault，不返回 apiKey 或 secretRef。
  // invoke('list_model_provider_configs')
  list_model_provider_configs: () => Promise<IpcResponse<ModelProviderConfigView[]>>;

  // 新建或按 config.id 更新配置。新建时由后端生成稳定 UUID。
  // invoke('save_model_provider_config', { config })
  save_model_provider_config: (args: {
    config: ModelProviderConfig;
  }) => Promise<IpcResponse<ModelProviderConfigView>>;

  // 删除配置及其 OS credential；若它是默认项，同时清除默认设置。
  delete_model_provider_config: (args: { configId: string }) => Promise<IpcResponse<boolean>>;

  // 选择唯一默认 Chat 配置；目标必须 enabled 且声明 chat capability。
  set_default_model_provider: (args: { configId: string }) => Promise<IpcResponse<boolean>>;

  // Compatibility commands for the original single-provider UI. New UI must use list/save.
  get_model_provider_config: () => Promise<IpcResponse<ModelProviderConfigView | null>>;
  update_model_provider_config: (args: {
    config: ModelProviderConfig;
  }) => Promise<IpcResponse<boolean>>;

  // 使用候选配置执行最小 JSON 请求。带 id 且省略 apiKey 时复用该配置的 secret。
  // invoke('test_model_provider_config', { config })
  test_model_provider_config: (args: {
    config: ModelProviderConfig;
  }) => Promise<IpcResponse<ModelProviderTestResult>>;

  // 使用当前默认 Chat 配置分析指定对象；不进行隐式 provider failover。
  trigger_ai_enrichment: (args: {
    objectId: string;
  }) => Promise<IpcResponse<AIEnrichmentRunResult>>;

  // 返回版本化 evaluator capability；当前 local deterministic evaluator 不访问网络、模型或 sandbox。
  list_evaluator_capabilities: () => Promise<IpcResponse<EvaluatorCapability[]>>;

  // requestId 是客户端生成的 UUID 幂等键；同一 object/evaluator/requestId 返回同一 run/job。
  // 省略 requestId 仅用于旧客户端兼容，后端会生成一次性 UUID。
  trigger_evaluation: (args: {
    objectId: string;
    evaluatorType: string;
    requestId?: string;
  }) => Promise<IpcResponse<{
    runId: string;
    jobId: string;
    requestId: string;
    correlationId: string;
    status: 'planned' | 'running' | 'passed' | 'failed';
    reused: boolean;
  }>>;

  // 查询评估运行详情，前端可用于展示版本、evidence、artifact 和稳定失败原因。
  // invoke('get_evaluation_run', { runId })
  get_evaluation_run: (args: { runId: string }) => Promise<IpcResponse<EvaluationRun>>;
}

/**
 * 模块：Backup / Storage
 */
export interface BackupCommands {
  // 创建同机 restore point；不接受任意路径参数。startup recovery 模式下禁用。
  create_backup: () => Promise<IpcResponse<BackupSummary>>;

  // 列出正式 backup 目录；staging 目录不会返回；startup recovery 模式可用。
  list_backups: () => Promise<IpcResponse<BackupSummary[]>>;

  // 校验 manifest、payload SHA-256 与 SQLite quick_check；startup recovery 模式可用。
  verify_backup: (args: {
    backupId: string;
  }) => Promise<IpcResponse<BackupVerification>>;

  // 重新校验目标、创建 safety backup，并迁移和验证私有候选目录；返回 transaction correlationId。startup recovery 模式会临时连接 live DB 但不运行普通 migration。
  prepare_restore: (args: { backupId: string }) => Promise<IpcResponse<RestorePreparation>>;

  // 返回最近一次恢复结果及可选 correlationId（旧结果可缺省）；不返回正文、文件清单或绝对路径；startup recovery 模式可用。
  get_restore_status: () => Promise<IpcResponse<RestoreStatus | null>>;

  // 仅在 pending restore 已存在时安排应用重启；文件替换发生在下次数据库初始化前；startup recovery 模式可用。
  restart_to_apply_restore: () => Promise<IpcResponse<boolean>>;
}
/**
 * 模块：Portable Export
 */
export interface PortableExportCommands {
  // 导出全库非 secret 对象到 app data/exports 下的 Markdown + JSON 目录。
  // 不接受任意路径参数；不导出 credential reference、内部 job、source snapshot storage URI 或 secret 对象。
  // startup recovery 模式禁用。
  // invoke('export_library')
  export_library: () => Promise<IpcResponse<PortableExportSummary>>;
}

/**
 * 模块：Jobs / Events / Diagnostics
 */
export interface OperationsCommands {
  // 查询后台任务状态，用于详情页、设置页和问题排查。
  // invoke('get_background_job', { jobId })
  get_background_job: (args: { jobId: string }) => Promise<IpcResponse<BackgroundJob>>;

  // 查询对象相关的最近后台任务。
  // invoke('get_object_jobs', { objectId, limit })
  get_object_jobs: (args: { objectId: string; limit: number }) => Promise<IpcResponse<BackgroundJob[]>>;

  // 重新运行失败任务。仅允许针对可重试错误。
  // invoke('retry_background_job', { jobId })
  retry_background_job: (args: { jobId: string }) => Promise<IpcResponse<boolean>>;

  // 获取本地指标快照。默认只在本机展示，不自动上传。
  // invoke('get_local_metrics_snapshot')
  get_local_metrics_snapshot: () => Promise<IpcResponse<LocalMetricsSnapshot>>;

  // 显式确认后，在 app data/support-bundles 下原子生成脱敏 JSON；不接受任意路径且不自动上传。
  // invoke('export_support_bundle', { confirmed: true })
  export_support_bundle: (args: { confirmed: boolean }) => Promise<IpcResponse<SupportBundleSummary>>;
}

/**
 * 模块：Plugins
 */
export interface PluginCommands {
  // 列出已安装插件及其权限声明。
  // invoke('list_plugins')
  list_plugins: () => Promise<IpcResponse<PluginManifest[]>>;

  // 启用或禁用插件。
  // invoke('set_plugin_enabled', { pluginId, enabled })
  set_plugin_enabled: (args: { pluginId: string; enabled: boolean }) => Promise<IpcResponse<boolean>>;

  // 更新插件权限授权。
  // invoke('update_plugin_permissions', { pluginId, permissions })
  update_plugin_permissions: (args: {
    pluginId: string;
    permissions: PluginPermission[];
  }) => Promise<IpcResponse<boolean>>;
}

/**
 * 模块：Library maintenance
 */
export interface MaintenanceCommands {
  // 删除对象，默认进入 tombstone 并异步清理派生产物。
  // invoke('delete_object', { objectId, mode })
  delete_object: (args: {
    objectId: string;
    mode: 'soft_delete' | 'purge' | 'export_then_delete';
  }) => Promise<IpcResponse<{ jobId?: string }>>;

  // 重新索引单个对象。
  // invoke('reindex_object', { objectId })
  reindex_object: (args: { objectId: string }) => Promise<IpcResponse<{ jobId: string }>>;

  // 只读检查 FTS 索引一致性；修复仍通过 rebuild/reindex 执行。
  // invoke('check_search_index')
  check_search_index: () => Promise<IpcResponse<SearchIndexHealthResponse>>;

  // 启动全库 FTS rebuild。命令立即返回 running job，前端需轮询 status 或监听事件。
  // invoke('rebuild_search_index')
  rebuild_search_index: () => Promise<IpcResponse<RebuildSearchIndexResponse>>;

  // 查询全库 FTS rebuild 进度。
  // invoke('get_search_index_rebuild_status', { jobId })
  get_search_index_rebuild_status: (args: {
    jobId: string;
  }) => Promise<IpcResponse<RebuildSearchIndexResponse>>;

  // 取消全库 FTS rebuild；只在 cancellable=true 时生效。finalizing 阶段不取消。
  // invoke('cancel_search_index_rebuild', { jobId })
  cancel_search_index_rebuild: (args: {
    jobId: string;
  }) => Promise<IpcResponse<RebuildSearchIndexResponse>>;
}
