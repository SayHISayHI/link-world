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

export interface EvaluationRun {
  id: string;
  objectId: string;
  evaluatorType: string;
  evaluatorVersion: string;
  status: 'planned' | 'running' | 'passed' | 'failed' | 'skipped' | 'blocked';
  score?: number;
  verdict: 'high_value' | 'useful' | 'situational' | 'low_value' | 'unsafe' | 'unknown';
  dimensions: Record<string, number>;
  evidence: EvidenceItem[];
  artifacts: EvaluationArtifact[];
  limitations: string[];
  nextActions: unknown[];
  createdAt: string;
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
  snippet?: string;
  score: number;
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
  analysisId?: string;
  status: 'succeeded' | 'failed' | string;
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
    | 'analysis.created'
    | 'evaluation.planned'
    | 'evaluation.completed'
    | 'object.deleted'
    | 'privacy.changed'
    | 'plugin.permission.changed';
  version: number;
  userId: string;
  objectId?: string;
  occurredAt: string;
  causationId?: string;
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

export interface LocalMetricsSnapshot {
  captureSuccessRate?: number;
  parseSuccessRate?: number;
  aiEnrichmentSuccessRate?: number;
  evaluationSuccessRate?: number;
  jobQueueDepth: number;
  databaseSizeBytes?: number;
  objectStoreSizeBytes?: number;
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
// ==========================================

/**
 * 模块：Capture (采集引擎)
 */
export interface CaptureCommands {
  // 提交一个新的采集请求。后端将存储快照并异步开始解析流程。
  // invoke('submit_capture', { item: RawCaptureItem })
  submit_capture: (args: { item: RawCaptureItem }) => Promise<IpcResponse<{ objectId: string }>>;
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

  // 执行基于 SQLite FTS5 和可选向量索引的混合搜索
  // invoke('search_hybrid', { query, limit })
  search_hybrid: (args: { query: string; limit: number }) => Promise<IpcResponse<SearchResult[]>>;
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

  // 手动触发指定对象的深度 AI 评估 (Evaluation Engine)
  // invoke('trigger_evaluation', { objectId, evaluatorType })
  trigger_evaluation: (args: {
    objectId: string;
    evaluatorType: string;
  }) => Promise<IpcResponse<{ runId: string }>>;

  // 查询评估运行详情，前端可用于展示 evidence、artifact 和失败原因。
  // invoke('get_evaluation_run', { runId })
  get_evaluation_run: (args: { runId: string }) => Promise<IpcResponse<EvaluationRun>>;
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
}
