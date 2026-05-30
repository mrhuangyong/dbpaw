import {
  QueryResult,
  SqlExecutionLog,
  RedisCommandLog,
  TableMetadata,
  SchemaOverview,
  SchemaForeignKey,
  ConnectionForm,
  TestConnectionResult,
  SavedQuery,
  ExportResult,
  ImportSqlResult,
  AIProviderConfig,
  AIConversation,
  AIConversationDetail,
} from "./api";

/**
 * Mock data layer - provides mock implementation for all API commands
 * Used for frontend standalone development and debugging in non-Tauri environments
 */

// ==================== Mock Data ====================

export const mockConnections: any[] = [
  {
    id: 1,
    uuid: "mock-1",
    name: "PostgreSQL Dev",
    dbType: "postgres",
    host: "localhost",
    port: 5432,
    database: "testdb",
    username: "postgres",
    ssl: false,
    sshEnabled: false,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
  {
    id: 2,
    uuid: "mock-2",
    name: "SQLite Local",
    dbType: "sqlite",
    host: "",
    port: 0,
    database: "",
    username: "",
    ssl: false,
    filePath: "/path/to/database.db",
    sshEnabled: false,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
  {
    id: 3,
    uuid: "mock-3",
    name: "PostgreSQL JSONB Test",
    dbType: "postgres",
    host: "localhost",
    port: 5432,
    database: "jsondb",
    username: "postgres",
    ssl: false,
    sshEnabled: false,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
];

export const mockTables: { schema: string; name: string; type: string }[] = [
  { schema: "public", name: "users", type: "table" },
  { schema: "public", name: "posts", type: "table" },
  { schema: "public", name: "comments", type: "table" },
  { schema: "public", name: "tags", type: "table" },
  { schema: "public", name: "orders", type: "table" },
  { schema: "public", name: "order_items", type: "table" },
  { schema: "public", name: "products", type: "table" },
  { schema: "public", name: "product_categories", type: "table" },
  { schema: "public", name: "categories", type: "table" },
  { schema: "public", name: "payments", type: "table" },
  { schema: "public", name: "refunds", type: "table" },
  { schema: "public", name: "invoices", type: "table" },
  { schema: "public", name: "addresses", type: "table" },
  { schema: "public", name: "audit_logs", type: "table" },
  { schema: "public", name: "sessions", type: "table" },
  { schema: "public", name: "roles", type: "table" },
  { schema: "public", name: "user_roles", type: "table" },
  { schema: "analytics", name: "page_views", type: "table" },
  { schema: "analytics", name: "events", type: "table" },
  { schema: "analytics", name: "funnels", type: "table" },
  // complex-type test table — SELECT * FROM json_test returns mockComplexTypeData
  { schema: "public", name: "json_test", type: "table" },
  // array-type test table — SELECT * FROM pg_arrays returns mockArrayTypeData
  { schema: "public", name: "pg_arrays", type: "table" },
  { schema: "public", name: "special_types", type: "table" },
];

export const mockTableStructure = {
  columns: [
    { name: "id", type: "integer", nullable: false },
    { name: "username", type: "varchar", nullable: false },
    { name: "email", type: "varchar", nullable: false },
    { name: "created_at", type: "timestamp", nullable: true },
    { name: "updated_at", type: "timestamp", nullable: true },
  ],
};

export const mockTableMetadata: TableMetadata = {
  columns: [
    {
      name: "id",
      type: "integer",
      nullable: false,
      primaryKey: true,
      comment: "User ID",
    },
    {
      name: "username",
      type: "varchar",
      nullable: false,
      primaryKey: false,
      comment: "Username",
    },
    {
      name: "email",
      type: "varchar",
      nullable: false,
      primaryKey: false,
      comment: "Email address",
    },
    {
      name: "password_hash",
      type: "varchar",
      nullable: false,
      primaryKey: false,
      comment: "Password hash",
    },
    {
      name: "created_at",
      type: "timestamp",
      nullable: true,
      defaultValue: "CURRENT_TIMESTAMP",
      primaryKey: false,
      comment: "Created timestamp",
    },
    {
      name: "updated_at",
      type: "timestamp",
      nullable: true,
      defaultValue: "CURRENT_TIMESTAMP",
      primaryKey: false,
      comment: "Updated timestamp",
    },
  ],
  indexes: [
    {
      name: "users_pkey",
      unique: true,
      indexType: "btree",
      columns: ["id"],
    },
    {
      name: "users_email_idx",
      unique: false,
      indexType: "btree",
      columns: ["email"],
    },
    {
      name: "users_username_idx",
      unique: false,
      indexType: "btree",
      columns: ["username"],
    },
  ],
  foreignKeys: [
    {
      name: "fk_user_role",
      column: "role_id",
      referencedTable: "roles",
      referencedColumn: "id",
      onUpdate: "CASCADE",
      onDelete: "SET NULL",
    },
  ],
  specialTypeSummaries: [],
};

export const mockSchemaForeignKeys: SchemaForeignKey[] = [
  {
    name: "fk_user_role",
    sourceTable: "users",
    sourceColumn: "role_id",
    targetTable: "roles",
    targetColumn: "id",
    onUpdate: "CASCADE",
    onDelete: "SET NULL",
  },
  {
    name: "fk_order_user",
    sourceTable: "orders",
    sourceColumn: "user_id",
    targetTable: "users",
    targetColumn: "id",
    onUpdate: "NO ACTION",
    onDelete: "CASCADE",
  },
  {
    name: "fk_order_item_order",
    sourceTable: "order_items",
    sourceColumn: "order_id",
    targetTable: "orders",
    targetColumn: "id",
    onUpdate: "NO ACTION",
    onDelete: "CASCADE",
  },
];

export const mockSchemaOverview: SchemaOverview = {
  tables: [
    {
      schema: "public",
      name: "users",
      columns: [
        { name: "id", type: "integer" },
        { name: "username", type: "varchar" },
        { name: "email", type: "varchar" },
        { name: "created_at", type: "timestamp" },
      ],
    },
    {
      schema: "public",
      name: "posts",
      columns: [
        { name: "id", type: "integer" },
        { name: "user_id", type: "integer" },
        { name: "title", type: "varchar" },
        { name: "content", type: "text" },
        { name: "created_at", type: "timestamp" },
      ],
    },
    {
      schema: "public",
      name: "comments",
      columns: [
        { name: "id", type: "integer" },
        { name: "post_id", type: "integer" },
        { name: "user_id", type: "integer" },
        { name: "content", type: "text" },
        { name: "created_at", type: "timestamp" },
      ],
    },
  ],
};

export const mockTableData = {
  data: [
    {
      id: 1,
      username: "alice",
      email: "alice@example.com",
      password_hash: "hashed_password_1",
      created_at: "2024-01-15 10:30:00",
      updated_at: "2024-01-15 10:30:00",
      // object with 4 keys → abbreviated as {role, department, ... +2}
      metadata: {
        role: "admin",
        department: "engineering",
        level: 5,
        active: true,
      },
      // array with 3 items → [3 items]
      tags: ["vip", "beta-tester", "early-adopter"],
      settings: null,
    },
    {
      id: 2,
      username: "bob",
      email: "bob@example.com",
      password_hash: "hashed_password_2",
      created_at: "2024-01-16 11:45:00",
      updated_at: "2024-01-16 11:45:00",
      // object with 2 keys → inline JSON
      metadata: { role: "user", department: "marketing" },
      // array with 1 item → inline JSON
      tags: ["newsletter"],
      // nested object → tree view shows expand/collapse
      settings: {
        theme: "dark",
        lang: "zh",
        notifications: { email: true, sms: false },
      },
    },
    {
      id: 3,
      username: "charlie",
      email: "charlie@example.com",
      password_hash: "hashed_password_3",
      created_at: "2024-01-17 14:20:00",
      updated_at: "2024-01-17 14:20:00",
      // empty object → {}
      metadata: {},
      // empty array → []
      tags: [],
      settings: { theme: "light", lang: "en" },
    },
    {
      id: 4,
      username: "diana",
      email: "diana@example.com",
      password_hash: "hashed_password_4",
      created_at: "2024-01-18 09:15:00",
      updated_at: "2024-01-18 09:15:00",
      // object containing a nested array
      metadata: {
        role: "moderator",
        permissions: ["read", "write", "delete"],
        score: 88,
      },
      tags: ["moderator", "trusted"],
      settings: null,
    },
    {
      id: 5,
      username: "eve",
      email: "eve@example.com",
      password_hash: "hashed_password_5",
      created_at: "2024-01-19 16:50:00",
      updated_at: "2024-01-19 16:50:00",
      // array of objects → table view renders as multi-column table
      metadata: [
        { key: "plan", value: "pro" },
        { key: "trial", value: false },
      ],
      tags: ["pro"],
      // object with 4 keys → tree/table view
      settings: {
        theme: "system",
        lang: "ja",
        timezone: "Asia/Tokyo",
        fontSize: 14,
      },
    },
    {
      id: 6,
      username: "frank",
      email: "frank@example.com",
      password_hash: "hashed_password_6",
      created_at: "2024-01-20 08:00:00",
      updated_at: "2024-01-20 08:00:00",
      // deeply nested 3-level object → tree view shows recursive expand
      metadata: {
        profile: {
          address: { city: "Shanghai", country: "CN", zip: "200000" },
          contact: { phone: "138-0000-0001", wechat: "frank_wx" },
        },
        billing: { plan: "enterprise", seats: 50, currency: "CNY" },
      },
      // large array (10 items) → [10 items]
      tags: ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"],
      settings: { theme: "dark", lang: "en", beta: false },
    },
    {
      id: 7,
      username: "grace",
      email: "grace@example.com",
      password_hash: "hashed_password_7",
      created_at: "2024-01-21 09:30:00",
      updated_at: "2024-01-21 09:30:00",
      // array of objects with uniform shape → table view renders nicely
      metadata: [
        { name: "cpu", value: "85%", status: "warn" },
        { name: "memory", value: "42%", status: "ok" },
        { name: "disk", value: "91%", status: "critical" },
        { name: "network", value: "12%", status: "ok" },
      ],
      // mixed-type array → table view falls back to index/value layout
      tags: ["ops", 42, true, null, { env: "prod" }],
      settings: null,
    },
    {
      id: 8,
      username: "henry",
      email: "henry@example.com",
      password_hash: "hashed_password_8",
      created_at: "2024-01-22 11:00:00",
      updated_at: "2024-01-22 11:00:00",
      // object with null/boolean values inside
      metadata: { verified: true, banned: false, reason: null, score: 0 },
      tags: ["new"],
      // deeply nested settings
      settings: {
        ui: { sidebar: "collapsed", density: "compact", animations: true },
        editor: { fontSize: 13, tabSize: 2, wordWrap: true, minimap: false },
        shortcuts: { save: "Ctrl+S", run: "F5", format: "Shift+Alt+F" },
      },
    },
    {
      id: 9,
      username: "iris",
      email: "iris@example.com",
      password_hash: "hashed_password_9",
      created_at: "2024-01-23 14:45:00",
      updated_at: "2024-01-23 14:45:00",
      // object with only 1 key → inline JSON {"role":"guest"}
      metadata: { role: "guest" },
      // array with exactly 2 items → inline JSON
      tags: ["read-only", "trial"],
      settings: { theme: "light", lang: "en", timezone: "UTC" },
    },
    {
      id: 10,
      username: "jack",
      email: "jack@example.com",
      password_hash: "hashed_password_10",
      created_at: "2024-01-24 16:20:00",
      updated_at: "2024-01-24 16:20:00",
      // all three complex fields are null → verify null rendering unchanged
      metadata: null,
      tags: null,
      settings: null,
    },
  ],
  total: 10,
  page: 1,
  limit: 10,
  executionTimeMs: 25,
};

// Dedicated dataset for querying "SELECT * FROM json_test" in mock mode.
// Covers every complex-type edge case in a single focused table.
export const mockComplexTypeData: QueryResult = {
  rowCount: 8,
  timeTakenMs: 12,
  success: true,
  columns: [
    { name: "id", type: "integer" },
    { name: "label", type: "text" },
    { name: "payload", type: "jsonb" },
    { name: "notes", type: "text" },
  ],
  data: [
    {
      id: 1,
      label: "flat object (2 keys)",
      payload: { name: "alice", age: 30 },
      notes: "inline JSON in cell",
    },
    {
      id: 2,
      label: "flat object (4+ keys)",
      payload: { id: 42, role: "admin", active: true, score: 99 },
      notes: "abbreviated as {id, role, ... +2}",
    },
    {
      id: 3,
      label: "nested object (3 levels)",
      payload: {
        user: {
          profile: { city: "Beijing", country: "CN" },
          prefs: { lang: "zh", theme: "dark" },
        },
        meta: { version: 2, flags: ["a", "b"] },
      },
      notes: "tree view shows recursive expand/collapse",
    },
    {
      id: 4,
      label: "array of primitives (10 items)",
      payload: [10, 20, 30, 40, 50, 60, 70, 80, 90, 100],
      notes: "[10 items] in cell",
    },
    {
      id: 5,
      label: "array of objects (uniform shape)",
      payload: [
        { metric: "cpu", value: 72, unit: "%" },
        { metric: "mem", value: 48, unit: "%" },
        { metric: "disk", value: 91, unit: "%" },
      ],
      notes: "table view renders as multi-column table",
    },
    {
      id: 6,
      label: "mixed-type array",
      payload: ["text", 42, true, null, { nested: "obj" }, [1, 2]],
      notes: "table view falls back to index/value layout",
    },
    {
      id: 7,
      label: "empty containers",
      payload: {},
      notes: "verify {} and [] display correctly",
    },
    {
      id: 8,
      label: "null value",
      payload: null,
      notes: "should display NULL (italic), no expand icon",
    },
  ],
};

// Dedicated dataset for querying "SELECT * FROM pg_arrays" in mock mode.
// Simulates what PostgreSQL array columns look like after the backend fix.
export const mockArrayTypeData: QueryResult = {
  rowCount: 4,
  timeTakenMs: 8,
  success: true,
  columns: [
    { name: "id", type: "integer" },
    { name: "tags", type: "text[]" },
    { name: "scores", type: "int4[]" },
    { name: "flags", type: "bool[]" },
    { name: "readings", type: "float8[]" },
    { name: "metadata_list", type: "jsonb[]" },
  ],
  data: [
    {
      id: 1,
      tags: ["postgres", "arrays", "jsonb"],
      scores: [95, 87, 72],
      flags: [true, false, true],
      readings: [3.14, 2.72, 1.41],
      metadata_list: [
        { source: "web", valid: true },
        { source: "app", valid: false },
      ],
    },
    {
      id: 2,
      tags: ["empty-arrays-test"],
      scores: [],
      flags: [],
      readings: [],
      metadata_list: [],
    },
    {
      id: 3,
      tags: ["null-elements", null, "after-null"],
      scores: [1, null, 3],
      flags: [null, true],
      readings: [null, 9.99],
      metadata_list: [null, { key: "value" }],
    },
    {
      id: 4,
      tags: null,
      scores: null,
      flags: null,
      readings: null,
      metadata_list: null,
    },
  ],
};

export const mockDatabases = [
  "postgres",
  "template1",
  "template0",
  "testdb",
  "myapp_dev",
];

export const mockSavedQueries: SavedQuery[] = [
  {
    id: 1,
    name: "Get all users",
    query: "SELECT * FROM users",
    description: "Fetch all users from the database",
    connectionId: 1,
    database: "testdb",
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
  {
    id: 2,
    name: "Active posts",
    query: "SELECT * FROM posts WHERE status = 'active'",
    description: null,
    connectionId: 1,
    database: "testdb",
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
];

export const mockQueryResult: QueryResult = {
  data: mockTableData.data,
  rowCount: 10,
  columns: [
    { name: "id", type: "integer" },
    { name: "username", type: "varchar" },
    { name: "email", type: "varchar" },
    { name: "password_hash", type: "varchar" },
    { name: "created_at", type: "timestamp" },
    { name: "updated_at", type: "timestamp" },
    { name: "metadata", type: "jsonb" },
    { name: "tags", type: "text[]" },
    { name: "settings", type: "jsonb" },
  ],
  timeTakenMs: 45,
  success: true,
};

export const mockMultipleResultSets: QueryResult = {
  data: [],
  rowCount: 0,
  columns: [],
  timeTakenMs: 120,
  success: true,
  resultSets: [
    {
      data: mockTableData.data.slice(0, 3),
      rowCount: 3,
      columns: [
        { name: "id", type: "integer" },
        { name: "username", type: "varchar" },
        { name: "email", type: "varchar" },
      ],
      index: 0,
      statement: "SELECT id, username, email FROM users LIMIT 3",
    },
    {
      data: mockTableData.data.slice(3, 6),
      rowCount: 3,
      columns: [
        { name: "id", type: "integer" },
        { name: "username", type: "varchar" },
        { name: "created_at", type: "timestamp" },
      ],
      index: 1,
      statement: "SELECT id, username, created_at FROM users LIMIT 3 OFFSET 3",
    },
    {
      data: mockTableData.data.slice(6, 8),
      rowCount: 2,
      columns: [
        { name: "id", type: "integer" },
        { name: "email", type: "varchar" },
      ],
      index: 2,
      statement: "SELECT id, email FROM users LIMIT 2 OFFSET 6",
    },
  ],
};

let mockSqlExecutionLogId = 1;
const mockSqlExecutionLogs: SqlExecutionLog[] = [];

function appendSqlExecutionLog(params: {
  sql: string;
  source?: string;
  connectionId?: number;
  database?: string;
  success: boolean;
  error?: string;
}) {
  mockSqlExecutionLogs.unshift({
    id: mockSqlExecutionLogId++,
    sql: params.sql,
    source: params.source || "unknown",
    connectionId: params.connectionId ?? null,
    database: params.database ?? null,
    success: params.success,
    error: params.error ?? null,
    executedAt: new Date().toISOString(),
  });

  if (mockSqlExecutionLogs.length > 100) {
    mockSqlExecutionLogs.length = 100;
  }
}

const mockRedisCommandLogs: RedisCommandLog[] = [];

export async function mockListRedisCommandLogs(
  limit?: number,
): Promise<RedisCommandLog[]> {
  const safeLimit = Math.min(Math.max(limit ?? 100, 1), 100);
  return mockRedisCommandLogs.slice(0, safeLimit);
}

const mockAiProviders: AIProviderConfig[] = [
  {
    id: 1,
    name: "OpenAI",
    providerType: "openai",
    baseUrl: "https://api.openai.com/v1",
    model: "gpt-4.1-mini",
    hasApiKey: true,
    isDefault: false,
    enabled: true,
    extraJson: null,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
  {
    id: 2,
    name: "OpenAI Compat",
    providerType: "openai_compat",
    baseUrl: "http://localhost:11434/v1",
    model: "qwen2.5-coder:14b",
    hasApiKey: true,
    isDefault: true,
    enabled: true,
    extraJson: JSON.stringify({ note: "mock provider" }),
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
  {
    id: 3,
    name: "Disabled Provider",
    providerType: "openai",
    baseUrl: "https://example.invalid/v1",
    model: "gpt-4.1",
    hasApiKey: true,
    isDefault: false,
    enabled: false,
    extraJson: null,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
];

const mockAiConversations: AIConversation[] = [
  {
    id: 1,
    title: "Generate: Order List SQL",
    scenario: "sql_generate",
    connectionId: 1,
    database: "testdb",
    createdAt: new Date(Date.now() - 86400000).toISOString(),
    updatedAt: new Date(Date.now() - 86400000).toISOString(),
  },
  {
    id: 2,
    title: "Optimize: Slow Query Log",
    scenario: "sql_optimize",
    connectionId: 1,
    database: "testdb",
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
  {
    id: 3,
    title: "Explain: JOIN Statement",
    scenario: "sql_explain",
    connectionId: 1,
    database: "testdb",
    createdAt: new Date(Date.now() - 3 * 3600 * 1000).toISOString(),
    updatedAt: new Date(Date.now() - 3 * 3600 * 1000).toISOString(),
  },
  {
    id: 4,
    title: "Test: Markdown Rendering",
    scenario: "general_chat",
    connectionId: 1,
    database: "testdb",
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
];

const mockAiMessages: Record<number, AIConversationDetail["messages"]> = {
  1: [
    {
      id: 1,
      conversationId: 1,
      role: "user",
      content:
        "List order count and total amount for each user in the last 7 days, ordered by total amount descending.",
      createdAt: new Date(Date.now() - 86400000).toISOString(),
    },
    {
      id: 2,
      conversationId: 1,
      role: "assistant",
      content:
        "SELECT u.id,\n       u.username,\n       COUNT(o.id) AS order_count,\n       COALESCE(SUM(o.total_amount), 0) AS total_amount\nFROM public.users u\nLEFT JOIN public.orders o\n  ON o.user_id = u.id\n AND o.created_at >= NOW() - INTERVAL '7 days'\nGROUP BY u.id, u.username\nORDER BY total_amount DESC;",
      model: "mock-model",
      createdAt: new Date(Date.now() - 86400000 + 1000).toISOString(),
    },
  ],
  2: [
    {
      id: 3,
      conversationId: 2,
      role: "user",
      content:
        "Optimize this query: SELECT * FROM audit_logs WHERE created_at > NOW() - INTERVAL '30 days' AND action = 'login'",
      createdAt: new Date().toISOString(),
    },
    {
      id: 4,
      conversationId: 2,
      role: "assistant",
      content:
        "SELECT id, user_id, action, created_at, ip\nFROM public.audit_logs\nWHERE action = 'login'\n  AND created_at > NOW() - INTERVAL '30 days'\nORDER BY created_at DESC;",
      model: "mock-model",
      createdAt: new Date(Date.now() + 2000).toISOString(),
    },
  ],
  3: [
    {
      id: 5,
      conversationId: 3,
      role: "user",
      content:
        "Explain what this SQL does: SELECT p.id, p.title FROM posts p JOIN users u ON u.id = p.user_id WHERE u.email LIKE '%@example.com' ORDER BY p.id DESC LIMIT 20",
      createdAt: new Date(Date.now() - 3 * 3600 * 1000).toISOString(),
    },
    {
      id: 6,
      conversationId: 3,
      role: "assistant",
      content:
        "The intent of this SQL is:\n1) Select posts (p.id, p.title) from posts table.\n2) Join users table via p.user_id = u.id, filtering author emails ending with @example.com.\n3) Sort results by post id descending, taking the latest 20.\n\nIf posts table is large, ensure indexes on posts(user_id) and users(email) (or use appropriate pattern matching strategy).",
      model: "mock-model",
      createdAt: new Date(Date.now() - 3 * 3600 * 1000 + 1000).toISOString(),
    },
  ],
  4: [
    {
      id: 7,
      conversationId: 4,
      role: "user",
      content:
        "Please show various Markdown formats, including code blocks, blockquotes, emphasis, etc.",
      createdAt: new Date().toISOString(),
    },
    {
      id: 8,
      conversationId: 4,
      role: "assistant",
      content:
        "Okay, here is a showcase of various Markdown formats:\\n\\n### 1. Code Blocks\\n\\n**SQL Query:**\\n```sql\\nSELECT u.id,\\n       u.username,\\n       COUNT(o.id) AS order_count,\\n       COALESCE(SUM(o.total_amount), 0) AS total_amount\\nFROM public.users u\\nLEFT JOIN public.orders o\\n  ON o.user_id = u.id\\n AND o.created_at >= NOW() - INTERVAL '7 days'\\nGROUP BY u.id, u.username\\nORDER BY total_amount DESC;\\n```\\n\\n**JavaScript:**\\n```javascript\\nfunction hello(name) {\\n  console.log('Hello, World!');\\n}\\n```\\n\\n### 2. Blockquotes\\n\\n> This is a blockquote.\\n> It can contain multiple lines.\\n>\\n> > It can even be nested.\\n\\n### 3. Emphasis\\n\\n*   **Bold Text**\\n*   *Italic Text*\\n*   ***Bold Italic***\\n*   ~~Strikethrough~~\\n\\n### 4. Lists\\n\\n**Unordered List:**\\n- Item A\\n- Item B\\n  - Subitem B.1\\n  - Subitem B.2\\n\\n**Ordered List:**\\n1. Step 1\\n2. Step 2\\n3. Step 3\\n\\n### 5. Tables\\n\\n| Name | Age | Occupation |\\n| :--- | :---: | ---: |\\n| Alice | 25 | Engineer |\\n| Bob | 30 | Designer |\\n| Charlie | 28 | Product Manager |\\n\\n### 6. Links & Inline Code\\n\\nThis is a [link](https://example.com), and this is inline code `const x = 1`.",
      model: "mock-model",
      createdAt: new Date(Date.now() + 1000).toISOString(),
    },
  ],
};

const mockDDL = `CREATE TABLE public.users (
  id integer NOT NULL,
  username character varying(255) NOT NULL,
  email character varying(255) NOT NULL,
  password_hash character varying(255) NOT NULL,
  created_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP,
  updated_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP,
  CONSTRAINT users_pkey PRIMARY KEY (id)
);

CREATE INDEX users_email_idx ON public.users USING btree (email);
CREATE INDEX users_username_idx ON public.users USING btree (username);`;

// ==================== Mock Handler Functions ====================

/**
 * Mock query execution
 */
export async function mockExecuteQuery(
  id: number,
  query: string,
  database?: string,
  source?: string,
): Promise<QueryResult> {
  // Simulate network latency
  await new Promise((resolve) => setTimeout(resolve, 100));

  const lower = query.toLowerCase();
  const failed = lower.includes("invalid") || lower.includes("error");
  if (failed) {
    const error = "Mock query execution failed";
    appendSqlExecutionLog({
      sql: query,
      source: source || "unknown",
      connectionId: id,
      database,
      success: false,
      error,
    });
    throw new Error(error);
  }

  // Check if query contains multiple statements (separated by semicolons)
  const statements = query
    .split(";")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
  const hasMultipleStatements = statements.length > 1;

  // Return different data based on query type
  if (lower.includes("select")) {
    // Check for multiple result sets request
    if (hasMultipleStatements || lower.includes("multiple")) {
      appendSqlExecutionLog({
        sql: query,
        source: source || "unknown",
        connectionId: id,
        database,
        success: true,
      });
      return mockMultipleResultSets;
    }

    // Dedicated array-type dataset: SELECT * FROM pg_arrays
    const isArrayQuery = lower.includes("pg_arrays") || lower.includes("array");
    // Dedicated complex-type dataset: SELECT * FROM json_test
    const isComplexQuery =
      !isArrayQuery &&
      (lower.includes("json_test") ||
        lower.includes("json") ||
        lower.includes("jsonb") ||
        lower.includes("complex"));
    const result = {
      ...(isArrayQuery
        ? mockArrayTypeData
        : isComplexQuery
          ? mockComplexTypeData
          : mockQueryResult),
      timeTakenMs: Math.floor(Math.random() * 100) + 20,
    };
    appendSqlExecutionLog({
      sql: query,
      source: source || "unknown",
      connectionId: id,
      database,
      success: true,
    });
    return result;
  }

  const result = {
    data: [],
    rowCount: 0,
    columns: [],
    timeTakenMs: Math.floor(Math.random() * 50) + 10,
    success: true,
  };
  appendSqlExecutionLog({
    sql: query,
    source: source || "unknown",
    connectionId: id,
    database,
    success: true,
  });
  return result;
}

/**
 * Mock query cancellation
 */
export async function mockCancelQuery(
  _uuid: string,
  _queryId: string,
): Promise<boolean> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return true;
}

/**
 * Mock query execution by connection info
 */
export async function mockExecuteByConn(
  form: ConnectionForm,
  sql: string,
): Promise<QueryResult> {
  await new Promise((resolve) => setTimeout(resolve, 100));
  appendSqlExecutionLog({
    sql,
    source: "execute_by_conn",
    database: form.database,
    success: true,
  });
  return mockQueryResult;
}

export async function mockListSqlExecutionLogs(
  limit = 100,
): Promise<SqlExecutionLog[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  const safeLimit = Math.max(1, Math.min(100, limit));
  return mockSqlExecutionLogs.slice(0, safeLimit);
}

/**
 * Mock list tables
 */
export async function mockListTables(
  _id: number,
  _database?: string,
  _schema?: string,
): Promise<{ schema: string; name: string; type: string }[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockTables;
}

/**
 * Mock get table structure
 */
export async function mockGetTableStructure(
  _id: number,
  _schema: string,
  _table: string,
): Promise<{ columns: { name: string; type: string; nullable: boolean }[] }> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockTableStructure;
}

/**
 * Mock get table DDL
 */
export async function mockGetTableDDL(
  _id: number,
  _database: string | undefined,
  _schema: string,
  _table: string,
): Promise<string> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockDDL;
}

export async function mockListEvents(
  _connectionId: string,
  _database: string,
): Promise<{ schema: string; name: string; status: string; eventType: string; executeAt: string | null; intervalValue: string | null; lastExecuted: string | null; definition: string | null }[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return [];
}

export async function mockListSequences(
  _connectionId: string,
  _database: string,
): Promise<{ schema: string; name: string; dataType: string; startValue: string | null; increment: string | null }[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return [];
}

export async function mockListTypes(
  _connectionId: string,
  _database: string,
): Promise<{ schema: string; name: string; category: string }[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return [];
}

export async function mockListSynonyms(
  _connectionId: string,
  _database: string,
): Promise<{ schema: string; name: string; baseObjectType: string }[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return [];
}

export async function mockListPackages(
  _connectionId: string,
  _database: string,
): Promise<{ schema: string; name: string; objectType: string }[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return [];
}

export async function mockListRoutines(
  _id: number,
  _database?: string,
  _schema?: string,
): Promise<{ schema: string; name: string; type: "procedure" | "function" }[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  const routines: {
    schema: string;
    name: string;
    type: "procedure" | "function";
  }[] = [
    { schema: "dbo", name: "sync_user_stats", type: "procedure" },
    { schema: "dbo", name: "format_user_name", type: "function" },
  ];
  return routines.filter((routine) => !_schema || routine.schema === _schema);
}

export async function mockGetRoutineDDL(
  _id: number,
  _database: string | undefined,
  schema: string,
  name: string,
  routineType: "procedure" | "function",
): Promise<string> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  if (routineType === "procedure") {
    return `CREATE PROCEDURE [${schema}].[${name}]
AS
BEGIN
    SELECT 1 AS ok;
END;`;
  }

  return `CREATE FUNCTION [${schema}].[${name}]()
RETURNS INT
AS
BEGIN
    RETURN 1;
END;`;
}

/**
 * Mock get table metadata
 */
const mockJsonTestTableMetadata: TableMetadata = {
  columns: [
    { name: "id", type: "integer", nullable: false, primaryKey: true },
    { name: "label", type: "text", nullable: false, primaryKey: false },
    { name: "payload", type: "jsonb", nullable: true, primaryKey: false },
    { name: "notes", type: "text", nullable: true, primaryKey: false },
  ],
  indexes: [],
  foreignKeys: [],
  specialTypeSummaries: [],
};

const mockArrayTestTableMetadata: TableMetadata = {
  columns: [
    { name: "id", type: "integer", nullable: false, primaryKey: true },
    { name: "tags", type: "text[]", nullable: true, primaryKey: false },
    { name: "scores", type: "int4[]", nullable: true, primaryKey: false },
    { name: "flags", type: "bool[]", nullable: true, primaryKey: false },
    { name: "readings", type: "float8[]", nullable: true, primaryKey: false },
    {
      name: "metadata_list",
      type: "jsonb[]",
      nullable: true,
      primaryKey: false,
    },
  ],
  indexes: [],
  foreignKeys: [],
  specialTypeSummaries: [],
};

const mockSpecialTypeTableMetadata: TableMetadata = {
  columns: [
    { name: "id", type: "integer", nullable: false, primaryKey: true },
    { name: "user_bitmap", type: "BITMAP", nullable: true, primaryKey: false },
    { name: "region_geo", type: "GEOMETRY", nullable: true, primaryKey: false },
    { name: "uv_hll", type: "HLL", nullable: true, primaryKey: false },
  ],
  indexes: [],
  foreignKeys: [],
  specialTypeSummaries: [
    {
      columnName: "user_bitmap",
      category: "bitmap",
      typeName: "BITMAP",
      declaredLength: null,
      memoryUsageBytes: null,
      memoryUsageDisplay: null,
      rawType: "BITMAP",
      notes: "Memory usage is not exposed by the current metadata driver.",
    },
    {
      columnName: "region_geo",
      category: "geo",
      typeName: "GEOMETRY",
      declaredLength: null,
      memoryUsageBytes: null,
      memoryUsageDisplay: null,
      rawType: "GEOMETRY",
      notes: "Memory usage is not exposed by the current metadata driver.",
    },
    {
      columnName: "uv_hll",
      category: "hyperloglog",
      typeName: "HLL",
      declaredLength: null,
      memoryUsageBytes: null,
      memoryUsageDisplay: null,
      rawType: "HLL",
      notes: "Memory usage is not exposed by the current metadata driver.",
    },
  ],
};

export async function mockGetTableMetadata(
  _id: number,
  _database: string | undefined,
  _schema: string,
  _table: string,
): Promise<TableMetadata> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  if (_table === "json_test") return mockJsonTestTableMetadata;
  if (_table === "pg_arrays") return mockArrayTestTableMetadata;
  if (_table === "special_types") return mockSpecialTypeTableMetadata;
  return mockTableMetadata;
}

export async function mockGetSchemaForeignKeys(
  _id: number,
  _database?: string,
  _schema?: string,
): Promise<SchemaForeignKey[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockSchemaForeignKeys;
}

/**
 * Mock list tables by connection info
 */
export async function mockListTablesByConn(
  _form: ConnectionForm,
): Promise<{ schema: string; name: string; type: string }[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockTables;
}

/**
 * Mock list databases
 */
export async function mockListDatabases(
  _form: ConnectionForm,
): Promise<string[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockDatabases;
}

/**
 * Mock list databases by ID
 */
export async function mockListDatabasesById(_id: number): Promise<string[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockDatabases;
}

/**
 * Mock get MySQL charsets
 */
export async function mockGetMysqlCharsets(_id: number): Promise<string[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return [
    "armscii8",
    "ascii",
    "big5",
    "binary",
    "cp1250",
    "cp1251",
    "cp1256",
    "cp1257",
    "cp850",
    "cp852",
    "cp866",
    "cp932",
    "dec8",
    "eucjpms",
    "euckr",
    "gb18030",
    "gb2312",
    "gbk",
    "geostd8",
    "greek",
    "hebrew",
    "hp8",
    "keybcs2",
    "koi8r",
    "koi8u",
    "latin1",
    "latin2",
    "latin5",
    "latin7",
    "macce",
    "macroman",
    "sjis",
    "swe7",
    "tis620",
    "ucs2",
    "ujis",
    "utf16",
    "utf16le",
    "utf32",
    "utf8",
    "utf8mb4",
  ];
}

/**
 * Mock get MySQL collations
 */
export async function mockGetMysqlCollations(
  _id: number,
  charset?: string,
): Promise<string[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  const all: Record<string, string[]> = {
    utf8mb4: [
      "utf8mb4_0900_ai_ci",
      "utf8mb4_0900_as_ci",
      "utf8mb4_0900_as_cs",
      "utf8mb4_bin",
      "utf8mb4_general_ci",
      "utf8mb4_unicode_ci",
      "utf8mb4_unicode_520_ci",
    ],
    utf8: ["utf8_bin", "utf8_general_ci", "utf8_unicode_ci"],
    latin1: ["latin1_bin", "latin1_general_ci", "latin1_swedish_ci"],
    ascii: ["ascii_bin", "ascii_general_ci"],
    binary: ["binary"],
  };
  if (charset && all[charset]) return all[charset];
  return Object.values(all).flat().sort();
}

/**
 * Mock get schema overview
 */
export async function mockGetSchemaOverview(
  _id: number,
  _database?: string,
  _schema?: string,
): Promise<SchemaOverview> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockSchemaOverview;
}

/**
 * Mock get table data
 */
export async function mockGetTableData(params: {
  id: number;
  schema: string;
  table: string;
  page: number;
  limit: number;
  filter?: string;
  sortColumn?: string;
  sortDirection?: "asc" | "desc";
  orderBy?: string;
}): Promise<{
  data: any[];
  total: number;
  page: number;
  limit: number;
  executionTimeMs: number;
}> {
  await new Promise((resolve) => setTimeout(resolve, 100));

  const { page = 1, limit = 10, table } = params;
  const start = (page - 1) * limit;
  const end = start + limit;

  const source =
    table === "json_test"
      ? { data: mockComplexTypeData.data, total: mockComplexTypeData.rowCount }
      : table === "pg_arrays"
        ? { data: mockArrayTypeData.data, total: mockArrayTypeData.rowCount }
        : mockTableData;

  return {
    data: source.data.slice(start, end),
    total: source.total,
    page,
    limit,
    executionTimeMs: Math.floor(Math.random() * 50) + 20,
  };
}

/**
 * Mock get table data by connection info
 */
export async function mockGetTableDataByConn(
  _form: ConnectionForm,
  _schema: string,
  _table: string,
  page: number,
  limit: number,
): Promise<{
  data: any[];
  total: number;
  page: number;
  limit: number;
  executionTimeMs: number;
}> {
  await new Promise((resolve) => setTimeout(resolve, 100));

  const start = (page - 1) * limit;
  const end = start + limit;

  return {
    data: mockTableData.data.slice(start, end),
    total: mockTableData.total,
    page,
    limit,
    executionTimeMs: Math.floor(Math.random() * 50) + 20,
  };
}

/**
 * Mock get connections list
 */
export async function mockGetConnections(): Promise<any[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockConnections;
}

/**
 * Mock create connection
 */
export async function mockCreateConnection(form: ConnectionForm): Promise<any> {
  await new Promise((resolve) => setTimeout(resolve, 100));

  const newConnection = {
    id: mockConnections.length + 1,
    uuid: `mock-${mockConnections.length + 1}`,
    name: form.name || "New Connection",
    dbType: form.driver,
    host: form.host ?? "",
    port: form.port ?? 0,
    database: form.database ?? "",
    username: form.username ?? "",
    ssl: form.ssl ?? false,
    filePath: form.filePath ?? null,
    sshEnabled: form.sshEnabled ?? false,
    sshHost: form.sshHost ?? null,
    sshPort: form.sshPort ?? null,
    sshUsername: form.sshUsername ?? null,
    sshPassword: form.sshPassword ?? null,
    sshKeyPath: form.sshKeyPath ?? null,
    mode: form.mode ?? null,
    seedNodes: form.seedNodes ?? null,
    sentinels: form.sentinels ?? null,
    connectTimeoutMs: form.connectTimeoutMs ?? null,
    serviceName: form.serviceName ?? null,
    sentinelPassword: form.sentinelPassword ?? null,
    authMode: form.authMode ?? null,
    apiKeyId: form.apiKeyId ?? null,
    apiKeySecret: form.apiKeySecret ?? null,
    apiKeyEncoded: form.apiKeyEncoded ?? null,
    cloudId: form.cloudId ?? null,
    authSource: form.authSource ?? null,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  };

  mockConnections.push(newConnection);
  return newConnection;
}

/**
 * Mock update connection
 */
export async function mockUpdateConnection(
  id: number,
  form: ConnectionForm,
): Promise<any> {
  await new Promise((resolve) => setTimeout(resolve, 100));

  const index = mockConnections.findIndex((c) => c.id === id);
  if (index === -1) {
    throw new Error(`Connection with id ${id} not found`);
  }

  const existing = mockConnections[index];
  const nextPassword =
    form.password !== undefined && form.password !== ""
      ? form.password
      : existing.password;
  const nextApiKeySecret =
    form.apiKeySecret !== undefined && form.apiKeySecret !== ""
      ? form.apiKeySecret
      : existing.apiKeySecret;
  const nextApiKeyEncoded =
    form.apiKeyEncoded !== undefined && form.apiKeyEncoded !== ""
      ? form.apiKeyEncoded
      : existing.apiKeyEncoded;

  const updatedConnection = {
    ...existing,
    name: form.name || existing.name,
    dbType: form.driver || existing.dbType,
    host: form.host ?? existing.host,
    port: form.port ?? existing.port,
    database: form.database ?? existing.database,
    username: form.username ?? existing.username,
    password: nextPassword,
    ssl: form.ssl ?? existing.ssl ?? false,
    filePath: form.filePath ?? existing.filePath ?? null,
    sshEnabled: form.sshEnabled ?? existing.sshEnabled ?? false,
    sshHost: form.sshHost ?? existing.sshHost ?? null,
    sshPort: form.sshPort ?? existing.sshPort ?? null,
    sshUsername: form.sshUsername ?? existing.sshUsername ?? null,
    sshPassword: form.sshPassword ?? existing.sshPassword ?? null,
    sshKeyPath: form.sshKeyPath ?? existing.sshKeyPath ?? null,
    mode: form.mode ?? existing.mode ?? null,
    seedNodes: form.seedNodes ?? existing.seedNodes ?? null,
    sentinels: form.sentinels ?? existing.sentinels ?? null,
    connectTimeoutMs:
      form.connectTimeoutMs ?? existing.connectTimeoutMs ?? null,
    serviceName: form.serviceName ?? existing.serviceName ?? null,
    sentinelPassword:
      form.sentinelPassword !== undefined && form.sentinelPassword !== ""
        ? form.sentinelPassword
        : (existing.sentinelPassword ?? null),
    authMode: form.authMode ?? existing.authMode ?? null,
    apiKeyId: form.apiKeyId ?? existing.apiKeyId ?? null,
    apiKeySecret: nextApiKeySecret,
    apiKeyEncoded: nextApiKeyEncoded,
    cloudId: form.cloudId ?? existing.cloudId ?? null,
    authSource: form.authSource ?? existing.authSource ?? null,
    updatedAt: new Date().toISOString(),
  };

  mockConnections[index] = updatedConnection;
  return updatedConnection;
}

/**
 * Mock delete connection
 */
export async function mockDeleteConnection(id: number): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  const index = mockConnections.findIndex((c) => c.id === id);
  if (index === -1) {
    throw new Error(`Connection with id ${id} not found`);
  }
  mockConnections.splice(index, 1);
}

export async function mockCreateDatabaseById(
  _id: number,
  _payload: { name: string },
): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 80));
}

/**
 * Mock test connection
 */
export async function mockTestConnectionEphemeral(
  _form: ConnectionForm,
): Promise<TestConnectionResult> {
  await new Promise((resolve) => setTimeout(resolve, 200));

  return {
    success: true,
    message: "Connection test successful",
    latencyMs: Math.floor(Math.random() * 100) + 50,
  };
}

/**
 * Mock get saved queries
 */
export async function mockGetSavedQueries(): Promise<SavedQuery[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return [...mockSavedQueries];
}

/**
 * Mock save query
 */
export async function mockSaveQuery(data: {
  name: string;
  query: string;
  description?: string;
  connectionId?: number;
  database?: string;
}): Promise<SavedQuery> {
  await new Promise((resolve) => setTimeout(resolve, 100));

  const newQuery: SavedQuery = {
    id:
      mockSavedQueries.length > 0
        ? Math.max(...mockSavedQueries.map((q) => q.id)) + 1
        : 1,
    name: data.name,
    query: data.query,
    description: data.description || null,
    connectionId: data.connectionId || null,
    database: data.database || null,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  };

  mockSavedQueries.push(newQuery);
  return newQuery;
}

/**
 * Mock update saved query
 */
export async function mockUpdateSavedQuery(
  id: number,
  data: {
    name: string;
    query: string;
    description?: string;
    connectionId?: number;
    database?: string;
  },
): Promise<SavedQuery> {
  await new Promise((resolve) => setTimeout(resolve, 100));

  const index = mockSavedQueries.findIndex((q) => q.id === id);
  if (index === -1) {
    throw new Error(`Saved query with id ${id} not found`);
  }

  const updatedQuery = {
    ...mockSavedQueries[index],
    ...data,
    updatedAt: new Date().toISOString(),
  };

  mockSavedQueries[index] = updatedQuery;
  return updatedQuery;
}

/**
 * Mock delete saved query
 */
export async function mockDeleteSavedQuery(id: number): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  const index = mockSavedQueries.findIndex((q) => q.id === id);
  if (index !== -1) {
    mockSavedQueries.splice(index, 1);
  }
}

/**
 * Mock export table data
 */
export async function mockExportTableData(_params: any): Promise<ExportResult> {
  await new Promise((resolve) => setTimeout(resolve, 120));
  return {
    filePath: `/tmp/dbpaw-table-export-${Date.now()}.csv`,
    rowCount: mockTableData.total,
  };
}

export async function mockExportDatabaseSql(
  params: any,
): Promise<ExportResult> {
  await new Promise((resolve) => setTimeout(resolve, 120));
  const suffix =
    params?.format === "sql_ddl"
      ? "ddl"
      : params?.format === "sql_dml"
        ? "dml"
        : "full";
  return {
    filePath:
      params?.filePath ||
      `/tmp/dbpaw-database-export-${suffix}-${Date.now()}.sql`,
    rowCount: mockTableData.total,
  };
}

/**
 * Mock export query result
 */
export async function mockExportQueryResult(
  _params: any,
): Promise<ExportResult> {
  await new Promise((resolve) => setTimeout(resolve, 120));
  return {
    filePath: `/tmp/dbpaw-query-export-${Date.now()}.csv`,
    rowCount: mockQueryResult.rowCount,
  };
}

export async function mockImportSqlFile(
  _params: any,
): Promise<ImportSqlResult> {
  await new Promise((resolve) => setTimeout(resolve, 160));
  return {
    filePath: _params?.filePath || `/tmp/dbpaw-import-${Date.now()}.sql`,
    totalStatements: 3,
    successStatements: 3,
    failedAt: undefined,
    error: undefined,
    timeTakenMs: 120,
    rolledBack: false,
  };
}

/**
 * Invoke corresponding mock handler function by command name
 */
export async function invokeMock<T>(cmd: string, args?: any): Promise<T> {
  console.log(`[Mock] ${cmd}`, args);

  switch (cmd) {
    // Query commands
    case "execute_query":
      return mockExecuteQuery(
        args.id,
        args.query,
        args.database,
        args.source,
      ) as Promise<T>;

    case "cancel_query":
      return mockCancelQuery(args.uuid, args.queryId) as Promise<T>;

    case "execute_by_conn":
      return mockExecuteByConn(args.form, args.sql) as Promise<T>;

    case "list_sql_execution_logs":
      return mockListSqlExecutionLogs(args?.limit) as Promise<T>;

    case "list_redis_command_logs":
      return mockListRedisCommandLogs(args?.limit) as Promise<T>;

    // Metadata commands
    case "list_tables":
      return mockListTables(args.id, args.database, args.schema) as Promise<T>;

    case "list_routines":
      return mockListRoutines(
        args.id,
        args.database,
        args.schema,
      ) as Promise<T>;

    case "list_events":
      return mockListEvents(args.connectionId, args.database) as Promise<T>;

    case "list_sequences":
      return mockListSequences(args.connectionId, args.database) as Promise<T>;

    case "list_types":
      return mockListTypes(args.connectionId, args.database) as Promise<T>;

    case "list_synonyms":
      return mockListSynonyms(args.connectionId, args.database) as Promise<T>;

    case "list_packages":
      return mockListPackages(args.connectionId, args.database) as Promise<T>;

    case "get_table_structure":
      return mockGetTableStructure(
        args.id,
        args.schema,
        args.table,
      ) as Promise<T>;

    case "get_table_ddl":
      return mockGetTableDDL(
        args.id,
        args.database,
        args.schema,
        args.table,
      ) as Promise<T>;

    case "get_routine_ddl":
      return mockGetRoutineDDL(
        args.id,
        args.database,
        args.schema,
        args.name,
        args.routineType,
      ) as Promise<T>;

    case "get_table_metadata":
      return mockGetTableMetadata(
        args.id,
        args.database,
        args.schema,
        args.table,
      ) as Promise<T>;

    case "list_tables_by_conn":
      return mockListTablesByConn(args.form) as Promise<T>;

    case "list_databases":
      return mockListDatabases(args.form) as Promise<T>;

    case "list_databases_by_id":
      return mockListDatabasesById(args.id) as Promise<T>;

    case "get_schema_overview":
      return mockGetSchemaOverview(
        args.id,
        args.database,
        args.schema,
      ) as Promise<T>;

    case "get_schema_foreign_keys":
      return mockGetSchemaForeignKeys(args.id, args.database, args.schema) as Promise<T>;

    // Table data commands
    case "get_table_data":
      return mockGetTableData(args) as Promise<T>;

    case "get_table_data_by_conn":
      return mockGetTableDataByConn(
        args.form,
        args.schema,
        args.table,
        args.page,
        args.limit,
      ) as Promise<T>;

    // Connection commands
    case "get_connections":
      return mockGetConnections() as Promise<T>;

    case "create_connection":
      return mockCreateConnection(args.form) as Promise<T>;

    case "update_connection":
      return mockUpdateConnection(args.id, args.form) as Promise<T>;

    case "delete_connection":
      return mockDeleteConnection(args.id) as Promise<T>;

    case "import_connections":
      return {
        imported: [],
        skipped: 0,
      } as T;

    case "create_database_by_id":
      return mockCreateDatabaseById(args.id, args.payload) as Promise<T>;

    case "get_mysql_charsets_by_id":
      return mockGetMysqlCharsets(args.id) as Promise<T>;

    case "get_mysql_collations_by_id":
      return mockGetMysqlCollations(args.id, args.charset) as Promise<T>;

    case "test_connection_ephemeral":
      return mockTestConnectionEphemeral(args.form) as Promise<T>;

    // Saved Queries commands
    case "get_saved_queries":
      return mockGetSavedQueries() as Promise<T>;

    case "save_query":
      return mockSaveQuery(args) as Promise<T>;

    case "update_saved_query":
      return mockUpdateSavedQuery(args.id, args) as Promise<T>;

    case "delete_saved_query":
      return mockDeleteSavedQuery(args.id) as Promise<T>;

    // Transfer commands
    case "export_table_data":
      return mockExportTableData(args) as Promise<T>;

    case "export_database_sql":
      return mockExportDatabaseSql(args) as Promise<T>;

    case "export_query_result":
      return mockExportQueryResult(args) as Promise<T>;

    case "import_sql_file":
      return mockImportSqlFile(args) as Promise<T>;

    case "elasticsearch_test_connection":
      return Promise.resolve({
        clusterName: "mock-cluster",
        clusterUuid: "mock-uuid",
        version: "8.13.0",
        tagline: "You Know, for Search",
      }) as Promise<T>;

    case "elasticsearch_test_connection_ephemeral":
      return Promise.resolve({
        success: true,
        message: "Connected to Elasticsearch 8.13.0",
        latencyMs: 12,
      }) as Promise<T>;

    case "elasticsearch_list_indices":
      return Promise.resolve([
        {
          name: "products",
          health: "green",
          status: "open",
          uuid: "mock-uuid-1",
          primaryShards: "1",
          replicaShards: "1",
          docsCount: 128,
          storeSize: "45kb",
          isSystem: false,
        },
        {
          name: "orders",
          health: "green",
          status: "open",
          uuid: "mock-uuid-2",
          primaryShards: "1",
          replicaShards: "1",
          docsCount: 512,
          storeSize: "120kb",
          isSystem: false,
        },
        {
          name: ".kibana",
          health: "green",
          status: "open",
          uuid: "mock-uuid-3",
          primaryShards: "1",
          replicaShards: "0",
          docsCount: 8,
          storeSize: "12kb",
          isSystem: true,
        },
      ]) as Promise<T>;

    case "elasticsearch_get_index_mapping": {
      const idx = String(args.index || "products");
      return Promise.resolve({
        [idx]: {
          mappings: {
            properties: {
              id: { type: "keyword" },
              name: { type: "text" },
              price: { type: "float" },
              category: { type: "keyword" },
              created_at: { type: "date" },
            },
          },
        },
      }) as Promise<T>;
    }

    case "elasticsearch_create_index":
      return Promise.resolve({
        index: String(args.index || "new-index"),
        acknowledged: true,
        shardsAcknowledged: true,
        status: 200,
      }) as Promise<T>;

    case "elasticsearch_delete_index":
      return Promise.resolve({
        index: String(args.index || "products"),
        acknowledged: true,
        shardsAcknowledged: null,
        status: 200,
      }) as Promise<T>;

    case "elasticsearch_refresh_index":
    case "elasticsearch_open_index":
    case "elasticsearch_close_index":
      return Promise.resolve({
        index: String(args.index || "products"),
        acknowledged: true,
        shardsAcknowledged: true,
        status: 200,
      }) as Promise<T>;

    case "elasticsearch_search_documents": {
      const hits = Array.from({ length: 3 }, (_, i) => ({
        index: String(args.index || "products"),
        id: `doc-${i + 1}`,
        score: 1.0 - i * 0.1,
        source: {
          id: `doc-${i + 1}`,
          name: `Mock Product ${i + 1}`,
          price: 19.99 + i * 10,
          category: "electronics",
          created_at: new Date().toISOString(),
        },
        fields: null,
      }));
      return Promise.resolve({
        hits,
        total: 3,
        tookMs: 5,
        aggregations: {
          categories: {
            buckets: [{ key: "electronics", doc_count: 3 }],
          },
        },
      }) as Promise<T>;
    }

    case "elasticsearch_get_document":
      return Promise.resolve({
        index: String(args.index || "products"),
        id: String(args.documentId || "doc-1"),
        found: true,
        source: {
          id: String(args.documentId || "doc-1"),
          name: "Mock Product",
          price: 29.99,
          category: "electronics",
          created_at: new Date().toISOString(),
        },
        fields: null,
      }) as Promise<T>;

    case "elasticsearch_upsert_document":
      return Promise.resolve({
        index: String(args.index || "products"),
        id: args.documentId || `auto-${Date.now()}`,
        result: args.documentId ? "updated" : "created",
        status: args.documentId ? 200 : 201,
      }) as Promise<T>;

    case "elasticsearch_delete_document":
      return Promise.resolve({
        index: String(args.index || "products"),
        id: String(args.documentId || "doc-1"),
        result: "deleted",
        status: 200,
      }) as Promise<T>;

    case "elasticsearch_execute_raw":
      return Promise.resolve({
        status: 200,
        body: '{"count":3,"_shards":{"total":1,"successful":1,"skipped":0,"failed":0}}',
        json: {
          count: 3,
          _shards: { total: 1, successful: 1, skipped: 0, failed: 0 },
        },
        tookMs: 3,
      }) as Promise<T>;

    case "ai_list_providers":
      return Promise.resolve([...mockAiProviders]) as Promise<T>;

    case "ai_create_provider": {
      const requestedType = String(args.config.providerType || "openai");

      const now = new Date().toISOString();
      const isDefault = args.config.isDefault ?? true;
      if (isDefault) {
        mockAiProviders.forEach((p) => (p.isDefault = false));
      }

      const idx = mockAiProviders.findIndex(
        (p) => p.providerType === requestedType,
      );
      if (idx >= 0) {
        mockAiProviders[idx] = {
          ...mockAiProviders[idx],
          ...args.config,
          providerType: requestedType,
          enabled: args.config.enabled ?? true,
          isDefault,
          updatedAt: now,
        };
        return Promise.resolve(mockAiProviders[idx]) as Promise<T>;
      }

      const id = mockAiProviders.length
        ? Math.max(...mockAiProviders.map((p) => p.id)) + 1
        : 1;
      const created: AIProviderConfig = {
        id,
        providerType: requestedType,
        isDefault,
        enabled: args.config.enabled ?? true,
        extraJson: args.config.extraJson ?? null,
        createdAt: now,
        updatedAt: now,
        ...args.config,
      };
      mockAiProviders.push(created);
      return Promise.resolve(created) as Promise<T>;
    }

    case "ai_update_provider": {
      const idx = mockAiProviders.findIndex((p) => p.id === args.id);
      if (idx < 0) throw new Error("Provider not found");
      const requestedType = String(
        args.config.providerType || mockAiProviders[idx].providerType,
      );
      const conflict = mockAiProviders.find(
        (p) => p.providerType === requestedType && p.id !== args.id,
      );
      if (conflict) {
        throw new Error("UNIQUE constraint failed: ai_providers.provider_type");
      }
      if (args.config.isDefault) {
        mockAiProviders.forEach((p) => (p.isDefault = false));
      }
      mockAiProviders[idx] = {
        ...mockAiProviders[idx],
        ...args.config,
        providerType: requestedType,
        updatedAt: new Date().toISOString(),
      };
      return Promise.resolve(mockAiProviders[idx]) as Promise<T>;
    }

    case "ai_delete_provider": {
      const idx = mockAiProviders.findIndex((p) => p.id === args.id);
      if (idx >= 0) mockAiProviders.splice(idx, 1);
      return Promise.resolve(undefined) as Promise<T>;
    }

    case "ai_set_default_provider": {
      mockAiProviders.forEach((p) => (p.isDefault = p.id === args.id));
      return Promise.resolve(undefined) as Promise<T>;
    }

    case "ai_list_conversations":
      return Promise.resolve([...mockAiConversations]) as Promise<T>;

    case "ai_get_conversation": {
      const c = mockAiConversations.find((x) => x.id === args.conversationId);
      if (!c) throw new Error("Conversation not found");
      return Promise.resolve({
        conversation: c,
        messages: mockAiMessages[c.id] || [],
      }) as Promise<T>;
    }

    case "list_system_fonts":
      return Promise.resolve([
        "Arial",
        "Helvetica",
        "Times New Roman",
        "Courier New",
        "Georgia",
        "Verdana",
        "Trebuchet MS",
        "Arial Black",
        "Impact",
        "Lucida Console",
        "Monaco",
        "Menlo",
        "SF Pro Text",
        "SF Mono",
        "PingFang SC",
        "Microsoft YaHei",
        "SimSun",
        "SimHei",
      ]) as Promise<T>;

    case "ai_delete_conversation": {
      const idx = mockAiConversations.findIndex(
        (x) => x.id === args.conversationId,
      );
      if (idx >= 0) mockAiConversations.splice(idx, 1);
      delete mockAiMessages[args.conversationId];
      return Promise.resolve(undefined) as Promise<T>;
    }

    case "ai_chat_start":
    case "ai_chat_continue": {
      const input = args.request.input as string;
      const selectedTables =
        (args.request.selectedTables as
          | Array<{ schema: string; name: string }>
          | undefined) || [];
      let conversationId = args.request.conversationId as number | undefined;
      if (!conversationId) {
        conversationId = mockAiConversations.length
          ? Math.max(...mockAiConversations.map((x) => x.id)) + 1
          : 1;
        mockAiConversations.unshift({
          id: conversationId,
          title: args.request.title || input.slice(0, 20),
          scenario: args.request.scenario || "sql_generate",
          connectionId: args.request.connectionId || null,
          database: args.request.database || null,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        });
      }
      const msgs = mockAiMessages[conversationId] || [];
      const now = new Date().toISOString();
      msgs.push({
        id: msgs.length + 1,
        conversationId,
        role: "user",
        content: input,
        createdAt: now,
      } as any);
      msgs.push({
        id: msgs.length + 1,
        conversationId,
        role: "assistant",
        content: (() => {
          const scenario = String(args.request.scenario || "sql_generate");
          const first = selectedTables[0];
          const from = first ? `${first.schema}.${first.name}` : "public.users";

          // Check for markdown test request
          if (
            input.toLowerCase().includes("markdown") ||
            input.includes("format")
          ) {
            return "Okay, here is a showcase of various Markdown formats:\n\n### 1. Code Blocks\n\n**SQL Query:**\n```sql\nSELECT u.id,\n       u.username,\n       COUNT(o.id) AS order_count,\n       COALESCE(SUM(o.total_amount), 0) AS total_amount\nFROM public.users u\nLEFT JOIN public.orders o\n  ON o.user_id = u.id\n AND o.created_at >= NOW() - INTERVAL '7 days'\nGROUP BY u.id, u.username\nORDER BY total_amount DESC;\n```\n\n**JavaScript:**\n```javascript\nfunction hello(name) {\n  console.log('Hello, World!');\n}\n```\n\n### 2. Blockquotes\n\n> This is a blockquote.\n> It can contain multiple lines.\n>\n> > It can even be nested.\n\n### 3. Emphasis\n\n*   **Bold Text**\n*   *Italic Text*\n*   ***Bold Italic***\n*   ~~Strikethrough~~\n\n### 4. Lists\n\n**Unordered List:**\n- Item A\n- Item B\n  - Subitem B.1\n  - Subitem B.2\n\n**Ordered List:**\n1. Step 1\n2. Step 2\n3. Step 3\n\n### 5. Tables\n\n| Name | Age | Occupation |\n| :--- | :---: | ---: |\n| Alice | 25 | Engineer |\n| Bob | 30 | Designer |\n| Charlie | 28 | Product Manager |\n\n### 6. Links & Inline Code\n\nThis is a [link](https://example.com), and this is inline code `const x = 1`.";
          }

          if (scenario === "sql_optimize") {
            return `SELECT *\nFROM ${from}\nWHERE 1=1\nLIMIT 100;`;
          }
          if (scenario === "sql_explain") {
            return `This is a mock explanation: The SQL mainly reads data from ${from}.`;
          }
          if (selectedTables.length > 0) {
            const names = selectedTables
              .map((t) => `${t.schema}.${t.name}`)
              .join(", ");
            return `SELECT *\nFROM ${from}\n-- selected tables: ${names}\nLIMIT 50;`;
          }
          return `SELECT *\nFROM ${from}\nLIMIT 50;`;
        })(),
        model: "mock-model",
        createdAt: now,
      } as any);
      mockAiMessages[conversationId] = msgs;
      const idx = mockAiConversations.findIndex((x) => x.id === conversationId);
      if (idx >= 0) {
        mockAiConversations[idx] = {
          ...mockAiConversations[idx],
          updatedAt: now,
        };
      }
      return Promise.resolve({
        conversationId,
        userMessageId: msgs[msgs.length - 2].id,
        assistantMessageId: msgs[msgs.length - 1].id,
      }) as Promise<T>;
    }

    default:
      console.warn(`[Mock] Unknown command: ${cmd}`);
      throw new Error(`Mock: Unknown command '${cmd}'`);
  }
}
