import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { invokeMock } from "./mocks";

// Helper to check if running in Tauri
export const isTauri = () => {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
};

// Helper to check if Mock mode is enabled
const useMockMode = () => {
  return import.meta.env.VITE_USE_MOCK === "true";
};

// Safe invoke wrapper
const invoke = async <T>(cmd: string, args?: any): Promise<T> => {
  // If running in Tauri, use real Tauri invoke
  if (isTauri()) {
    return tauriInvoke(cmd, args);
  }

  // If not in Tauri, check if Mock mode is enabled
  if (useMockMode()) {
    return invokeMock<T>(cmd, args);
  }

  // If not in Tauri and Mock mode is disabled, throw error
  console.warn(`[API] invoke ${cmd}`, args);
  throw new Error(
    "Tauri API not available. Please run 'bun tauri dev' or enable Mock mode with 'VITE_USE_MOCK=true'.",
  );
};

export interface QueryColumn {
  name: string;
  type: string;
}

export interface SingleResultSet {
  data: any[];
  rowCount: number;
  columns: QueryColumn[];
  index: number;
  statement: string;
}

export interface QueryResult {
  data: any[];
  rowCount: number;
  columns: QueryColumn[];
  timeTakenMs: number;
  success: boolean;
  error?: string;
  resultSets?: SingleResultSet[];
}

export interface RedisDatabaseInfo {
  index: number;
  name: string;
  selected: boolean;
  keyCount?: number;
}

export interface RedisServerInfo {
  sections: Record<string, Record<string, string>>;
  dbsize: number;
}

export interface RedisSlowlogEntry {
  id: number;
  timestamp: number;
  durationMs: number;
  command: string;
}

export interface RedisKeyInfo {
  key: string;
  keyType: string;
  ttl: number;
}

export interface RedisScanResponse {
  cursor: string;
  keys: RedisKeyInfo[];
  isPartial: boolean;
}

export type RedisConnectionMode = "standalone" | "cluster" | "sentinel";

export type RedisValue =
  | { kind: "string"; value: string }
  | { kind: "hash"; value: Record<string, string> }
  | { kind: "list"; value: string[] }
  | { kind: "set"; value: string[] }
  | { kind: "zSet"; value: { member: string; score: number }[] }
  | { kind: "stream"; value: { id: string; fields: Record<string, string> }[] }
  | { kind: "json"; value: string }
  | { kind: "none"; value?: null };

export interface RedisBitmapBit {
  offset: number;
  value: boolean;
}

export interface RedisGeoMember {
  member: string;
  longitude: number;
  latitude: number;
}

export interface RedisGeoPosition {
  longitude: number;
  latitude: number;
}

export interface RedisGeoSearchResult {
  member: string;
  distance?: number;
  hash?: number;
  position?: RedisGeoPosition;
}

export interface RedisKeyExtra {
  subtype?: string | null;
  streamInfo?: {
    length: number;
    radixTreeKeys: number;
    radixTreeNodes: number;
    groups: number;
    lastGeneratedId: string;
    firstEntry?: { id: string; fields: Record<string, string> } | null;
    lastEntry?: { id: string; fields: Record<string, string> } | null;
  } | null;
  streamGroups?: RedisStreamGroup[] | null;
  hllCount?: number | null;
  geoCount?: number | null;
  bitmapCount?: number | null;
}

export interface RedisKeyValue {
  key: string;
  keyType: string;
  ttl: number;
  value: RedisValue;
  valueTotalLen: number | null;
  valueOffset: number;
  isBinary?: boolean;
  extra?: RedisKeyExtra | null;
  objectEncoding?: string | null;
  memoryUsage?: number | null;
  objectIdletime?: number | null;
  objectRefcount?: number | null;
  keyExists?: boolean | null;
}

export interface RedisSetKeyPayload {
  key: string;
  value: RedisValue;
  ttlSeconds?: number | null;
  setNx?: boolean;
  setXx?: boolean;
  setPx?: number | null;
  setKeepttl?: boolean;
}

export interface RedisMutationResult {
  success: boolean;
  affected: number;
}

export interface RedisListSetItem {
  index: number;
  value: string;
}

export interface RedisStreamEntry {
  id: string;
  fields: Record<string, string>;
}

export interface RedisStreamGroup {
  name: string;
  consumers: number;
  pending: number;
  lastDeliveredId: string;
  entriesRead?: number | null;
  lag?: number | null;
}

export interface RedisStreamView {
  entries: RedisStreamEntry[];
  totalLen: number;
  startId: string;
  endId: string;
  count: number;
  nextStartId?: string | null;
  streamInfo?: RedisKeyExtra["streamInfo"];
  groups: RedisStreamGroup[];
}

export interface RedisXPendingSummary {
  count: number;
  minId: string;
  maxId: string;
  consumers: [string, number][];
}

export interface RedisXPendingEntry {
  id: string;
  consumer: string;
  idleMs: number;
  deliveryCount: number;
}

export interface RedisXClaimEntry {
  id: string;
  fields: Record<string, string>;
  idleMs?: number;
  deliveryCount?: number;
}

export interface RedisKeyPatchPayload {
  key: string;
  ttlSeconds: number | null;
  hashSet?: Record<string, string>;
  hashDel?: string[];
  setAdd?: string[];
  setRem?: string[];
  zsetAdd?: { member: string; score: number }[];
  zsetRem?: string[];
  listRpush?: string[];
  listLpush?: string[];
  listSet?: RedisListSetItem[];
  listRem?: string[];
  listLpop?: number;
  listRpop?: number;
  streamAdd?: RedisStreamEntry[];
  streamDel?: string[];
  bitmapSet?: RedisBitmapBit[];
  stringIncrBy?: string;
  hashIncrBy?: Record<string, string>;
  zsetIncrBy?: { member: string; score: number }[];
  stringIncrByInt?: number;
}

export interface RedisRawResult {
  output: string;
}

export interface RedisZRangeByScoreResult {
  members: { member: string; score: number }[];
  total: number;
}

export interface RedisZRangeByLexResult {
  members: string[];
  total: number;
}

export type RedisSetOperation = "inter" | "union" | "diff";

export type RedisLInsertPosition = "before" | "after";

export type RedisLMoveDirection = "left" | "right";

export interface RedisBatchKeyOp {
  op: "del" | "unlink" | "expire" | "persist";
  key: string;
  ttlSeconds?: number;
}

export interface RedisBatchKeyOpResult {
  key: string;
  op: string;
  success: boolean;
  affected: number;
}

export interface RedisMgetEntry {
  key: string;
  value: string | null;
  exists: boolean;
}

export interface RedisClusterNode {
  id: string;
  addr: string;
  flags: string[];
  masterId: string | null;
  pingSent: number;
  pongRecv: number;
  configEpoch: number;
  linkState: string;
  slotRange: string | null;
}

export interface RedisClusterInfo {
  info: Record<string, string>;
  nodes: RedisClusterNode[];
}

export interface ElasticsearchConnectionInfo {
  clusterName?: string | null;
  clusterUuid?: string | null;
  version?: string | null;
  tagline?: string | null;
}

export interface ElasticsearchIndexInfo {
  name: string;
  health?: string | null;
  status?: string | null;
  uuid?: string | null;
  primaryShards?: string | null;
  replicaShards?: string | null;
  docsCount?: number | null;
  storeSize?: string | null;
  isSystem: boolean;
}

export interface ElasticsearchSearchHit {
  index: string;
  id: string;
  score?: number | null;
  source: any;
  fields?: any;
}

export interface ElasticsearchSearchResponse {
  hits: ElasticsearchSearchHit[];
  total: number;
  tookMs: number;
  aggregations?: any;
}

export interface ElasticsearchDocument {
  index: string;
  id: string;
  found: boolean;
  source?: any;
  fields?: any;
}

export interface ElasticsearchMutationResult {
  index?: string | null;
  id?: string | null;
  result?: string | null;
  status: number;
}

export interface ElasticsearchIndexOperationResult {
  index?: string | null;
  acknowledged?: boolean | null;
  shardsAcknowledged?: boolean | null;
  status: number;
}

export interface ElasticsearchRawResponse {
  status: number;
  body: string;
  json?: any;
  tookMs: number;
}

export interface ElasticsearchBulkExportResult {
  filePath: string;
  index: string;
  documents: number;
  batches: number;
  timeTakenMs: number;
}

export interface ElasticsearchBulkImportResult {
  filePath: string;
  index: string;
  totalActions: number;
  successful: number;
  failed: number;
  errors: string[];
  timeTakenMs: number;
}

export interface MongodbConnectionInfo {
  version?: string;
  nodeCount?: number;
}

export interface MongodbDatabaseInfo {
  name: string;
  sizeOnDisk?: number;
  empty?: boolean;
}

export interface MongodbCollectionInfo {
  name: string;
  database: string;
  documentCount?: number;
  size?: number;
}

export type SqlExecutionSource =
  | "sql_editor"
  | "table_view_save"
  | "execute_by_conn"
  | "unknown";

export interface SqlExecutionLog {
  id: number;
  sql: string;
  source?: string | null;
  connectionId?: number | null;
  database?: string | null;
  success: boolean;
  error?: string | null;
  executedAt: string;
}

export interface RedisCommandLog {
  id: number;
  command: string;
  connectionId?: number | null;
  database?: string | null;
  success: boolean;
  error?: string | null;
  executedAt: string;
}

import {
  DRIVER_REGISTRY,
  type Driver,
  type ImportDriverCapability,
} from "@/lib/driver-registry";
export type { Driver, ImportDriverCapability } from "@/lib/driver-registry";

export const normalizeImportDriver = (driver: string): string => {
  const normalized = (driver || "").trim().toLowerCase();
  if (normalized === "postgresql" || normalized === "pgsql") {
    return "postgres";
  }
  return normalized;
};

export const getImportDriverCapability = (
  driver: string,
): ImportDriverCapability => {
  const normalized = normalizeImportDriver(driver);
  const config = DRIVER_REGISTRY.find((d) => d.id === normalized);
  return config?.importCapability ?? "unsupported";
};

// ── Sync types ────────────────────────────────────────────

export type SyncProviderType = "S3" | "WebDAV";

export interface SyncConfig {
  providerType: SyncProviderType;
  endpoint?: string;
  region?: string;
  bucket?: string;
  accessKeyId?: string;
  secretAccessKey?: string;
  pathPrefix?: string;
  serverUrl?: string;
  username?: string;
  password?: string;
}

export interface SyncStatus {
  enabled: boolean;
  providerType?: SyncProviderType;
  endpoint?: string;
  lastSyncAt?: string;
  lastSyncResult?: string;
  deviceId?: string;
}

export interface SyncResult {
  action: string;
  timestamp: string;
  remoteDeviceId?: string;
}

export interface ConnectionForm {
  driver: Driver;
  name?: string;
  host?: string;
  port?: number;
  database?: string;
  schema?: string;
  username?: string;
  password?: string;
  ssl?: boolean;
  sslMode?: "require" | "verify_ca";
  sslCaCert?: string;
  filePath?: string;
  sshEnabled?: boolean;
  sshHost?: string;
  sshPort?: number;
  sshUsername?: string;
  sshPassword?: string;
  sshKeyPath?: string;
  mode?: RedisConnectionMode;
  seedNodes?: string[];
  sentinels?: string[];
  connectTimeoutMs?: number;
  serviceName?: string;
  sentinelPassword?: string;
  authMode?: string;
  apiKeyId?: string;
  apiKeySecret?: string;
  apiKeyEncoded?: string;
  cloudId?: string;
  authSource?: string;
}

export interface SavedConnection {
  id: number;
  uuid: string;
  name: string;
  dbType: string;
  host: string;
  port: number;
  database: string;
  username: string;
  ssl: boolean;
  sslMode?: "require" | "verify_ca";
  sslCaCert?: string | null;
  filePath?: string | null;
  sshEnabled: boolean;
  sshHost?: string | null;
  sshPort?: number | null;
  sshUsername?: string | null;
  sshPassword?: string | null;
  sshKeyPath?: string | null;
  mode?: RedisConnectionMode | null;
  seedNodes?: string[] | null;
  sentinels?: string[] | null;
  connectTimeoutMs?: number | null;
  serviceName?: string | null;
  sentinelPassword?: string | null;
  authMode?: "none" | "basic" | "api_key" | null;
  apiKeyId?: string | null;
  apiKeySecret?: string | null;
  apiKeyEncoded?: string | null;
  cloudId?: string | null;
  authSource?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface ImportResult {
  imported: SavedConnection[];
  skipped: number;
}

export interface CreateDatabasePayload {
  name: string;
  ifNotExists?: boolean;
  charset?: string;
  collation?: string;
  encoding?: string;
  lcCollate?: string;
  lcCtype?: string;
}
export interface TestConnectionResult {
  success: boolean;
  message: string;
  latencyMs?: number;
}

export interface ColumnSchema {
  name: string;
  type: string;
}

export interface ColumnInfo {
  name: string;
  type: string;
  nullable: boolean;
  defaultValue?: string | null;
  primaryKey: boolean;
  comment?: string | null;
  defaultConstraintName?: string | null;
}

export interface IndexInfo {
  name: string;
  unique: boolean;
  indexType?: string | null;
  columns: string[];
}

export interface ForeignKeyInfo {
  name: string;
  column: string;
  referencedSchema?: string | null;
  referencedTable: string;
  referencedColumn: string;
  onUpdate?: string | null;
  onDelete?: string | null;
}

export interface ClickHouseTableExtra {
  engine: string;
  partitionKey?: string | null;
  sortingKey?: string | null;
  primaryKeyExpr?: string | null;
  samplingKey?: string | null;
  ttlExpr?: string | null;
  createTableQuery?: string | null;
}

export interface CassandraTableExtra {
  partitionKey: string[];
  clusteringColumns: string[];
  compactionStrategy: string;
  bloomFilterFpChance: number;
  caching: any;
  gcGraceSeconds: number;
  defaultTimeToLive: number;
}

export type SpecialTypeCategory = "bitmap" | "geo" | "hyperloglog";

export interface SpecialTypeSummary {
  columnName: string;
  category: SpecialTypeCategory;
  typeName: string;
  declaredLength?: string | null;
  memoryUsageBytes?: number | null;
  memoryUsageDisplay?: string | null;
  rawType: string;
  notes?: string | null;
}

export interface TableMetadata {
  columns: ColumnInfo[];
  indexes: IndexInfo[];
  foreignKeys: ForeignKeyInfo[];
  clickhouseExtra?: ClickHouseTableExtra | null;
  cassandraExtra?: CassandraTableExtra | null;
  specialTypeSummaries: SpecialTypeSummary[];
}

export interface SchemaForeignKey {
  name: string;
  sourceTable: string;
  sourceSchema?: string | null;
  sourceColumn: string;
  targetTable: string;
  targetSchema?: string | null;
  targetColumn: string;
  onUpdate?: string | null;
  onDelete?: string | null;
}

export type RoutineType = "procedure" | "function";

export interface RoutineInfo {
  schema: string;
  name: string;
  type: RoutineType;
}

export interface EventInfo {
  schema: string;
  name: string;
  status: string;
  eventType: string;
  executeAt: string | null;
  intervalValue: string | null;
  lastExecuted: string | null;
  definition: string | null;
}

export interface SequenceInfo {
  schema: string;
  name: string;
  dataType: string;
  startValue: string | null;
  increment: string | null;
}

export interface TypeInfo {
  schema: string;
  name: string;
  category: string;
}

export interface SynonymInfo {
  schema: string;
  name: string;
  baseObjectType: string;
}

export interface PackageInfo {
  schema: string;
  name: string;
  objectType: string;
}

export interface TableSchema {
  schema: string;
  name: string;
  columns: ColumnSchema[];
}

export interface SchemaOverview {
  tables: TableSchema[];
}

export interface SavedQuery {
  id: number;
  name: string;
  query: string;
  description?: string | null;
  connectionId?: number | null;
  database?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface SqliteConnectionIssue {
  id: number;
  connectionId: number;
  connectionName: string;
  filePath: string;
  issueType:
    | "locked"
    | "corrupted"
    | "permission_denied"
    | "not_found"
    | string;
  description: string;
  detectedAt: string;
  resolvedAt?: string | null;
}

export interface AIProviderConfig {
  id: number;
  name: string;
  providerType: AIProviderType;
  baseUrl: string;
  model: string;
  hasApiKey: boolean;
  isDefault: boolean;
  enabled: boolean;
  extraJson?: string | null;
  createdAt: string;
  updatedAt: string;
}

export type AIProviderType = string;

export interface AIProviderForm {
  name: string;
  providerType?: AIProviderType;
  baseUrl: string;
  model: string;
  apiKey?: string;
  isDefault?: boolean;
  enabled?: boolean;
  extraJson?: string;
}

export interface AIUsage {
  promptTokens?: number | null;
  completionTokens?: number | null;
  totalTokens?: number | null;
}

export interface AIConversation {
  id: number;
  title: string;
  scenario: string;
  connectionId?: number | null;
  database?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface AIMessage {
  id: number;
  conversationId: number;
  role: "system" | "developer" | "user" | "assistant" | "tool" | string;
  content: string;
  promptVersion?: string | null;
  model?: string | null;
  tokenIn?: number | null;
  tokenOut?: number | null;
  latencyMs?: number | null;
  createdAt: string;
}

export interface AIConversationDetail {
  conversation: AIConversation;
  messages: AIMessage[];
}

export interface AITableSummary {
  schema: string;
  name: string;
  columns: { name: string; type: string; nullable?: boolean }[];
}

export interface AISchemaOverview {
  tables: AITableSummary[];
}

export interface AISelectedTableRef {
  schema: string;
  name: string;
}

export interface AIChatRequest {
  requestId: string;
  providerId?: number;
  conversationId?: number;
  scenario: "sql_generate" | "sql_optimize" | "sql_explain" | string;
  input: string;
  title?: string;
  connectionId?: number;
  database?: string;
  schemaOverview?: AISchemaOverview;
  selectedTables?: AISelectedTableRef[];
}

export interface AIChatResponse {
  conversationId: number;
  userMessageId: number;
  assistantMessageId: number;
}

export type TransferFormat =
  | "csv"
  | "json"
  | "sql_dml"
  | "sql_ddl"
  | "sql_full";
export type ExportScope =
  | "current_page"
  | "filtered"
  | "full_table"
  | "query_result";

export interface ExportResult {
  filePath: string;
  rowCount: number;
}

export interface ImportSqlResult {
  filePath: string;
  totalStatements: number;
  successStatements: number;
  failedAt?: number;
  failedBatch?: number;
  failedStatementPreview?: string;
  error?: string;
  timeTakenMs: number;
  rolledBack: boolean;
}

export const api = {
  query: {
    execute: (
      id: number,
      query: string,
      database?: string,
      source?: SqlExecutionSource,
      queryId?: string,
    ) =>
      invoke<QueryResult>("execute_query", {
        id,
        query,
        database,
        source,
        queryId,
      }),
    cancel: (uuid: string, queryId: string) =>
      invoke<boolean>("cancel_query", { uuid, queryId }),
    executeByConn: (form: ConnectionForm, sql: string) =>
      invoke<QueryResult>("execute_by_conn", { form, sql }),
  },
  sqlLogs: {
    list: (limit = 100) =>
      invoke<SqlExecutionLog[]>("list_sql_execution_logs", { limit }),
  },
  redisLogs: {
    list: (limit = 100) =>
      invoke<RedisCommandLog[]>("list_redis_command_logs", { limit }),
  },
  metadata: {
    listTables: (id: number, database?: string, schema?: string) =>
      invoke<{ schema: string; name: string; type: string }[]>("list_tables", {
        id,
        database,
        schema,
      }),
    listRoutines: (id: number, database?: string, schema?: string) =>
      invoke<RoutineInfo[]>("list_routines", {
        id,
        database,
        schema,
      }),
    getTableStructure: (id: number, schema: string, table: string) =>
      invoke<{ columns: { name: string; type: string; nullable: boolean }[] }>(
        "get_table_structure",
        { id, schema, table },
      ),
    getTableDDL: (
      id: number,
      database: string | undefined,
      schema: string,
      table: string,
    ) => invoke<string>("get_table_ddl", { id, database, schema, table }),
    getRoutineDDL: (
      id: number,
      database: string | undefined,
      schema: string,
      name: string,
      routineType: RoutineType,
    ) =>
      invoke<string>("get_routine_ddl", {
        id,
        database,
        schema,
        name,
        routineType,
      }),
    getTableMetadata: (
      id: number,
      database: string | undefined,
      schema: string,
      table: string,
    ) =>
      invoke<TableMetadata>("get_table_metadata", {
        id,
        database,
        schema,
        table,
      }),
    listTablesByConn: (form: ConnectionForm) =>
      invoke<{ schema: string; name: string; type: string }[]>(
        "list_tables_by_conn",
        { form },
      ),
    listDatabases: (form: ConnectionForm) =>
      invoke<string[]>("list_databases", { form }),
    listDatabasesById: (id: number) =>
      invoke<string[]>("list_databases_by_id", { id }),
    getSchemaOverview: (id: number, database?: string, schema?: string) =>
      invoke<SchemaOverview>("get_schema_overview", { id, database, schema }),
    getSchemaForeignKeys: (id: number, database?: string, schema?: string) =>
      invoke<SchemaForeignKey[]>("get_schema_foreign_keys", { id, database, schema }),
    listEvents: (connectionId: string, database: string) =>
      invoke<EventInfo[]>("list_events", { connectionId, database }),
    listSequences: (connectionId: string, database: string) =>
      invoke<SequenceInfo[]>("list_sequences", { connectionId, database }),
    listTypes: (connectionId: string, database: string) =>
      invoke<TypeInfo[]>("list_types", { connectionId, database }),
    listSynonyms: (connectionId: string, database: string) =>
      invoke<SynonymInfo[]>("list_synonyms", { connectionId, database }),
    listPackages: (connectionId: string, database: string) =>
      invoke<PackageInfo[]>("list_packages", { connectionId, database }),
  },
  tableData: {
    get: (params: {
      id: number;
      database?: string;
      schema: string;
      table: string;
      page: number;
      limit: number;
      filter?: string;
      sortColumn?: string;
      sortDirection?: "asc" | "desc";
      orderBy?: string;
    }) =>
      invoke<{
        data: any[];
        total: number;
        page: number;
        limit: number;
        executionTimeMs: number;
      }>("get_table_data", params),
    getByConn: (
      form: ConnectionForm,
      schema: string,
      table: string,
      page: number,
      limit: number,
    ) =>
      invoke<{
        data: any[];
        total: number;
        page: number;
        limit: number;
        executionTimeMs: number;
      }>("get_table_data_by_conn", { form, schema, table, page, limit }),
  },
  transfer: {
    exportTable: (params: {
      id: number;
      database?: string;
      schema: string;
      table: string;
      driver: string;
      format: TransferFormat;
      scope: Exclude<ExportScope, "query_result">;
      filter?: string;
      orderBy?: string;
      sortColumn?: string;
      sortDirection?: "asc" | "desc";
      page?: number;
      limit?: number;
      filePath?: string;
      chunkSize?: number;
    }) => invoke<ExportResult>("export_table_data", params),
    exportDatabase: (params: {
      id: number;
      database: string;
      driver: string;
      format: "sql_dml" | "sql_ddl" | "sql_full";
      filePath?: string;
      chunkSize?: number;
    }) => invoke<ExportResult>("export_database_sql", params),
    exportQueryResult: (params: {
      id: number;
      database?: string;
      sql: string;
      driver: string;
      format: TransferFormat;
      filePath?: string;
    }) => invoke<ExportResult>("export_query_result", params),
    importSqlFile: (params: {
      id: number;
      database?: string;
      filePath: string;
      driver: string;
    }) => invoke<ImportSqlResult>("import_sql_file", params),
  },
  connections: {
    list: () => invoke<SavedConnection[]>("get_connections"),
    create: (form: ConnectionForm) =>
      invoke<SavedConnection>("create_connection", { form }),
    update: (id: number, form: ConnectionForm) =>
      invoke<SavedConnection>("update_connection", { id, form }),
    delete: (id: number) => invoke<void>("delete_connection", { id }),
    createDatabase: (id: number, payload: CreateDatabasePayload) =>
      invoke<void>("create_database_by_id", { id, payload }),
    getMysqlCharsets: (id: number) =>
      invoke<string[]>("get_mysql_charsets_by_id", { id }),
    getMysqlCollations: (id: number, charset?: string) =>
      invoke<string[]>("get_mysql_collations_by_id", { id, charset }),
    testEphemeral: (form: ConnectionForm) =>
      invoke<TestConnectionResult>("test_connection_ephemeral", { form }),
    listSqliteIssues: () =>
      invoke<SqliteConnectionIssue[]>("list_sqlite_issues"),
    importFromFile: (filePath: string) =>
      invoke<ImportResult>("import_connections", { filePath }),
  },
  redis: {
    listDatabases: (id: number) =>
      invoke<RedisDatabaseInfo[]>("redis_list_databases", { id }),
    scanKeys: (params: {
      id: number;
      database?: string;
      cursor?: string;
      pattern?: string;
      limit?: number;
    }) => invoke<RedisScanResponse>("redis_scan_keys", params),
    getKey: (id: number, database: string | undefined, key: string) =>
      invoke<RedisKeyValue>("redis_get_key", { id, database, key }),
    setKey: (
      id: number,
      database: string | undefined,
      payload: RedisSetKeyPayload,
    ) =>
      invoke<RedisMutationResult>("redis_set_key", { id, database, payload }),
    updateKey: (
      id: number,
      database: string | undefined,
      payload: RedisSetKeyPayload,
    ) =>
      invoke<RedisMutationResult>("redis_update_key", {
        id,
        database,
        payload,
      }),
    deleteKey: (id: number, database: string | undefined, key: string) =>
      invoke<RedisMutationResult>("redis_delete_key", { id, database, key }),
    renameKey: (
      id: number,
      database: string | undefined,
      oldKey: string,
      newKey: string,
      force?: boolean,
    ) =>
      invoke<RedisMutationResult>("redis_rename_key", {
        id,
        database,
        oldKey,
        newKey,
        force,
      }),
    setTtl: (
      id: number,
      database: string | undefined,
      key: string,
      ttlSeconds?: number | null,
    ) =>
      invoke<RedisMutationResult>("redis_set_ttl", {
        id,
        database,
        key,
        ttlSeconds,
      }),
    getKeyPage: (
      id: number,
      database: string | undefined,
      key: string,
      offset: number,
      limit: number,
    ) =>
      invoke<RedisKeyValue>("redis_get_key_page", {
        id,
        database,
        key,
        offset,
        limit,
      }),
    getStreamRange: (
      id: number,
      database: string | undefined,
      key: string,
      startId: string,
      count: number,
    ) =>
      invoke<RedisStreamEntry[]>("redis_get_stream_range", {
        id,
        database,
        key,
        startId,
        count,
      }),
    getStreamView: (
      id: number,
      database: string | undefined,
      key: string,
      startId: string,
      endId: string,
      count: number,
    ) =>
      invoke<RedisStreamView>("redis_get_stream_view", {
        id,
        database,
        key,
        startId,
        endId,
        count,
      }),
    xgroupCreate: (
      id: number,
      database: string | undefined,
      key: string,
      group: string,
      startId: string,
      mkstream?: boolean,
    ) =>
      invoke<boolean>("redis_xgroup_create", {
        id,
        database,
        key,
        group,
        startId,
        mkstream,
      }),
    xgroupDel: (
      id: number,
      database: string | undefined,
      key: string,
      group: string,
    ) => invoke<boolean>("redis_xgroup_del", { id, database, key, group }),
    xgroupSetId: (
      id: number,
      database: string | undefined,
      key: string,
      group: string,
      startId: string,
    ) =>
      invoke<boolean>("redis_xgroup_setid", {
        id,
        database,
        key,
        group,
        startId,
      }),
    xack: (
      id: number,
      database: string | undefined,
      key: string,
      group: string,
      ids: string[],
    ) => invoke<number>("redis_xack", { id, database, key, group, ids }),
    xpending: (
      id: number,
      database: string | undefined,
      key: string,
      group: string,
      start?: string,
      end?: string,
      count?: number,
      consumer?: string,
    ) =>
      invoke<RedisXPendingSummary | RedisXPendingEntry[]>("redis_xpending", {
        id,
        database,
        key,
        group,
        start,
        end,
        count,
        consumer,
      }),
    xclaim: (
      id: number,
      database: string | undefined,
      key: string,
      group: string,
      consumer: string,
      minIdleMs: number,
      ids: string[],
    ) =>
      invoke<RedisXClaimEntry[]>("redis_xclaim", {
        id,
        database,
        key,
        group,
        consumer,
        minIdleMs,
        ids,
      }),
    xtrim: (
      id: number,
      database: string | undefined,
      key: string,
      strategy: string,
      threshold: string,
      approximate?: boolean,
    ) =>
      invoke<number>("redis_xtrim", {
        id,
        database,
        key,
        strategy,
        threshold,
        approximate,
      }),
    xreadgroup: (
      id: number,
      database: string | undefined,
      key: string,
      group: string,
      consumer: string,
      startId: string,
      count?: number,
    ) =>
      invoke<RedisStreamEntry[]>("redis_xreadgroup", {
        id,
        database,
        key,
        group,
        consumer,
        startId,
        count,
      }),
    executeRaw: (id: number, database: string | undefined, command: string) =>
      invoke<RedisRawResult>("redis_execute_raw", { id, database, command }),
    patchKey: (
      id: number,
      database: string | undefined,
      payload: RedisKeyPatchPayload,
    ) =>
      invoke<RedisMutationResult>("redis_patch_key", { id, database, payload }),
    bitmapGetBit: (
      id: number,
      database: string | undefined,
      key: string,
      offset: number,
    ) => invoke<boolean>("redis_bitmap_get_bit", { id, database, key, offset }),
    bitmapCount: (
      id: number,
      database: string | undefined,
      key: string,
      start?: number,
      end?: number,
    ) =>
      invoke<number>("redis_bitmap_count", { id, database, key, start, end }),
    bitmapPos: (
      id: number,
      database: string | undefined,
      key: string,
      bit: boolean,
      start?: number,
      end?: number,
      count?: number,
    ) =>
      invoke<number[]>("redis_bitmap_pos", {
        id,
        database,
        key,
        bit,
        start,
        end,
        count,
      }),
    hllPfadd: (
      id: number,
      database: string | undefined,
      key: string,
      elements: string[],
    ) => invoke<boolean>("redis_hll_pfadd", { id, database, key, elements }),
    geoAdd: (
      id: number,
      database: string | undefined,
      key: string,
      members: RedisGeoMember[],
    ) => invoke<number>("redis_geo_add", { id, database, key, members }),
    geoPos: (
      id: number,
      database: string | undefined,
      key: string,
      members: string[],
    ) =>
      invoke<(RedisGeoPosition | null)[]>("redis_geo_pos", {
        id,
        database,
        key,
        members,
      }),
    geoDist: (
      id: number,
      database: string | undefined,
      key: string,
      member1: string,
      member2: string,
      unit?: string,
    ) =>
      invoke<number>("redis_geo_dist", {
        id,
        database,
        key,
        member1,
        member2,
        unit,
      }),
    geoSearch: (
      id: number,
      database: string | undefined,
      key: string,
      params: {
        member?: string;
        longitude?: number;
        latitude?: number;
        radius: number;
        unit: string;
        withCoord?: boolean;
        withDist?: boolean;
        withHash?: boolean;
        count?: number;
      },
    ) =>
      invoke<RedisGeoSearchResult[]>("redis_geo_search", {
        id,
        database,
        key,
        ...params,
      }),
    serverInfo: (id: number, database: string | undefined) =>
      invoke<RedisServerInfo>("redis_server_info", { id, database }),
    serverConfig: (id: number, database: string | undefined) =>
      invoke<Record<string, string>>("redis_server_config", { id, database }),
    slowlogGet: (id: number, database: string | undefined, count?: number) =>
      invoke<RedisSlowlogEntry[]>("redis_slowlog_get", { id, database, count }),
    zrangebyscore: (
      id: number,
      database: string | undefined,
      key: string,
      min: string,
      max: string,
      offset?: number,
      limit?: number,
    ) =>
      invoke<RedisZRangeByScoreResult>("redis_zrangebyscore", {
        id,
        database,
        key,
        min,
        max,
        offset,
        limit,
      }),
    zrank: (
      id: number,
      database: string | undefined,
      key: string,
      member: string,
      reverse?: boolean,
    ) =>
      invoke<number | null>("redis_zrank", {
        id,
        database,
        key,
        member,
        reverse,
      }),
    setOperation: (
      id: number,
      database: string | undefined,
      keys: string[],
      op: RedisSetOperation,
    ) => invoke<string[]>("redis_set_operation", { id, database, keys, op }),
    sismember: (
      id: number,
      database: string | undefined,
      key: string,
      member: string,
    ) => invoke<boolean>("redis_sismember", { id, database, key, member }),
    smove: (
      id: number,
      database: string | undefined,
      source: string,
      destination: string,
      member: string,
    ) =>
      invoke<boolean>("redis_smove", {
        id,
        database,
        source,
        destination,
        member,
      }),
    batchKeyOps: (
      id: number,
      database: string | undefined,
      operations: RedisBatchKeyOp[],
    ) =>
      invoke<RedisBatchKeyOpResult[]>("redis_batch_key_ops", {
        id,
        database,
        operations,
      }),
    mget: (id: number, database: string | undefined, keys: string[]) =>
      invoke<RedisMgetEntry[]>("redis_mget", { id, database, keys }),
    mset: (
      id: number,
      database: string | undefined,
      entries: Record<string, string>,
    ) => invoke<RedisMutationResult>("redis_mset", { id, database, entries }),
    clusterInfo: (id: number, database: string | undefined) =>
      invoke<RedisClusterInfo>("redis_cluster_info", { id, database }),
    zscore: (
      id: number,
      database: string | undefined,
      key: string,
      member: string,
    ) => invoke<number | null>("redis_zscore", { id, database, key, member }),
    zmscore: (
      id: number,
      database: string | undefined,
      key: string,
      members: string[],
    ) =>
      invoke<(number | null)[]>("redis_zmscore", {
        id,
        database,
        key,
        members,
      }),
    zrangebylex: (
      id: number,
      database: string | undefined,
      key: string,
      min: string,
      max: string,
      offset?: number,
      limit?: number,
    ) =>
      invoke<RedisZRangeByLexResult>("redis_zrangebylex", {
        id,
        database,
        key,
        min,
        max,
        offset,
        limit,
      }),
    zlexcount: (
      id: number,
      database: string | undefined,
      key: string,
      min: string,
      max: string,
    ) => invoke<number>("redis_zlexcount", { id, database, key, min, max }),
    zpopmin: (
      id: number,
      database: string | undefined,
      key: string,
      count?: number,
    ) =>
      invoke<{ member: string; score: number }[]>("redis_zpopmin", {
        id,
        database,
        key,
        count,
      }),
    zpopmax: (
      id: number,
      database: string | undefined,
      key: string,
      count?: number,
    ) =>
      invoke<{ member: string; score: number }[]>("redis_zpopmax", {
        id,
        database,
        key,
        count,
      }),
    lindex: (
      id: number,
      database: string | undefined,
      key: string,
      index: number,
    ) => invoke<string | null>("redis_lindex", { id, database, key, index }),
    lpos: (
      id: number,
      database: string | undefined,
      key: string,
      element: string,
      rank?: number,
      count?: number,
      maxlen?: number,
    ) =>
      invoke<number[]>("redis_lpos", {
        id,
        database,
        key,
        element,
        rank,
        count,
        maxlen,
      }),
    ltrim: (
      id: number,
      database: string | undefined,
      key: string,
      start: number,
      stop: number,
    ) => invoke<boolean>("redis_ltrim", { id, database, key, start, stop }),
    linsert: (
      id: number,
      database: string | undefined,
      key: string,
      position: RedisLInsertPosition,
      pivot: string,
      element: string,
    ) =>
      invoke<number>("redis_linsert", {
        id,
        database,
        key,
        position,
        pivot,
        element,
      }),
    lmove: (
      id: number,
      database: string | undefined,
      source: string,
      destination: string,
      srcDirection: RedisLMoveDirection,
      dstDirection: RedisLMoveDirection,
    ) =>
      invoke<string | null>("redis_lmove", {
        id,
        database,
        source,
        destination,
        srcDirection,
        dstDirection,
      }),
  },
  elasticsearch: {
    testConnection: (id: number) =>
      invoke<ElasticsearchConnectionInfo>("elasticsearch_test_connection", {
        id,
      }),
    listIndices: (id: number) =>
      invoke<ElasticsearchIndexInfo[]>("elasticsearch_list_indices", { id }),
    getIndexMapping: (id: number, index: string) =>
      invoke<any>("elasticsearch_get_index_mapping", { id, index }),
    createIndex: (params: { id: number; index: string; body?: any }) =>
      invoke<ElasticsearchIndexOperationResult>(
        "elasticsearch_create_index",
        params,
      ),
    deleteIndex: (id: number, index: string) =>
      invoke<ElasticsearchIndexOperationResult>("elasticsearch_delete_index", {
        id,
        index,
      }),
    refreshIndex: (id: number, index: string) =>
      invoke<ElasticsearchIndexOperationResult>("elasticsearch_refresh_index", {
        id,
        index,
      }),
    openIndex: (id: number, index: string) =>
      invoke<ElasticsearchIndexOperationResult>("elasticsearch_open_index", {
        id,
        index,
      }),
    closeIndex: (id: number, index: string) =>
      invoke<ElasticsearchIndexOperationResult>("elasticsearch_close_index", {
        id,
        index,
      }),
    searchDocuments: (params: {
      id: number;
      index: string;
      query?: string;
      dsl?: string;
      from: number;
      size: number;
    }) =>
      invoke<ElasticsearchSearchResponse>(
        "elasticsearch_search_documents",
        params,
      ),
    getDocument: (id: number, index: string, documentId: string) =>
      invoke<ElasticsearchDocument>("elasticsearch_get_document", {
        id,
        index,
        documentId,
      }),
    upsertDocument: (params: {
      id: number;
      index: string;
      documentId?: string;
      source: any;
      refresh?: boolean;
    }) =>
      invoke<ElasticsearchMutationResult>(
        "elasticsearch_upsert_document",
        params,
      ),
    deleteDocument: (params: {
      id: number;
      index: string;
      documentId: string;
      refresh?: boolean;
    }) =>
      invoke<ElasticsearchMutationResult>(
        "elasticsearch_delete_document",
        params,
      ),
    exportDocuments: (params: {
      id: number;
      index: string;
      query?: string;
      dsl?: string;
      filePath: string;
      batchSize?: number;
    }) =>
      invoke<ElasticsearchBulkExportResult>(
        "elasticsearch_export_documents",
        params,
      ),
    importDocuments: (params: {
      id: number;
      index: string;
      filePath: string;
      batchSize?: number;
      refresh?: boolean;
    }) =>
      invoke<ElasticsearchBulkImportResult>(
        "elasticsearch_import_documents",
        params,
      ),
    executeRaw: (params: {
      id: number;
      method: string;
      path: string;
      body?: string;
    }) => invoke<ElasticsearchRawResponse>("elasticsearch_execute_raw", params),
  },
  mongodb: {
    testConnection: (id: number) =>
      invoke<MongodbConnectionInfo>("mongodb_test_connection", { id }),
    testConnectionEphemeral: (form: ConnectionForm) =>
      invoke<TestConnectionResult>("mongodb_test_connection_ephemeral", {
        form,
      }),
    listDatabases: (id: number) =>
      invoke<MongodbDatabaseInfo[]>("mongodb_list_databases", { id }),
    listCollections: (id: number, database: string) =>
      invoke<MongodbCollectionInfo[]>("mongodb_list_collections", {
        id,
        database,
      }),
  },
  queries: {
    list: () => invoke<SavedQuery[]>("get_saved_queries"),
    create: (data: {
      name: string;
      query: string;
      description?: string;
      connectionId?: number;
      database?: string;
    }) => invoke<SavedQuery>("save_query", data),
    update: (
      id: number,
      data: {
        name: string;
        query: string;
        description?: string;
        connectionId?: number;
        database?: string;
      },
    ) => invoke<SavedQuery>("update_saved_query", { id, ...data }),
    delete: (id: number) => invoke<void>("delete_saved_query", { id }),
  },
  ai: {
    providers: {
      list: () => invoke<AIProviderConfig[]>("ai_list_providers"),
      create: (config: AIProviderForm) =>
        invoke<AIProviderConfig>("ai_create_provider", { config }),
      update: (id: number, config: AIProviderForm) =>
        invoke<AIProviderConfig>("ai_update_provider", { id, config }),
      delete: (id: number) => invoke<void>("ai_delete_provider", { id }),
      setDefault: (id: number) =>
        invoke<void>("ai_set_default_provider", { id }),
      clearApiKey: (providerType: string) =>
        invoke<void>("ai_clear_provider_api_key", {
          provider_type: providerType,
        }),
    },
    chat: {
      start: (request: AIChatRequest) =>
        invoke<AIChatResponse>("ai_chat_start", { request }),
      continue: (request: AIChatRequest) =>
        invoke<AIChatResponse>("ai_chat_continue", { request }),
    },
    conversations: {
      list: (filters?: { connectionId?: number; database?: string }) =>
        invoke<AIConversation[]>("ai_list_conversations", {
          connectionId: filters?.connectionId,
          database: filters?.database,
        }),
      get: (conversationId: number) =>
        invoke<AIConversationDetail>("ai_get_conversation", {
          conversationId,
        }),
      delete: (conversationId: number) =>
        invoke<void>("ai_delete_conversation", { conversationId }),
    },
  },
  system: {
    listFonts: () => invoke<string[]>("list_system_fonts"),
  },
  sync: {
    testConnection: (config: SyncConfig): Promise<void> =>
      invoke("sync_test_connection", { config }),
    configure: (config: SyncConfig, syncPassword: string): Promise<void> =>
      invoke("sync_configure", { config, syncPassword }),
    getStatus: (): Promise<SyncStatus> =>
      invoke("sync_get_status"),
    syncNow: (syncPassword: string): Promise<SyncResult> =>
      invoke("sync_now", { syncPassword }),
    forcePush: (syncPassword: string): Promise<void> =>
      invoke("sync_force_push", { syncPassword }),
    forcePull: (syncPassword: string): Promise<void> =>
      invoke("sync_force_pull", { syncPassword }),
    disable: (): Promise<void> =>
      invoke("sync_disable"),
    updatePassword: (oldPassword: string, newPassword: string): Promise<void> =>
      invoke("sync_update_password", { oldPassword, newPassword }),
  },
};
