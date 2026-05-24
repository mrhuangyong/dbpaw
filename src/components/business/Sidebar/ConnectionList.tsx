import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type FormEvent,
  type ReactNode,
} from "react";
import { save, open } from "@tauri-apps/plugin-dialog";
import { readTextFile } from "@tauri-apps/plugin-fs";
import {
  Database,
  Table2 as TableIcon,
  Key,
  Copy,
  Edit3,
  Plus,
  RefreshCw,
  Play,
  Loader2,
  Trash2,
  FileCode,
  Search,
  Download,
  FolderOpen,
  Upload,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { api, getImportDriverCapability, isTauri } from "@/services/api";
import type {
  ConnectionForm,
  CreateDatabasePayload,
  Driver,
  RedisConnectionMode,
  RoutineType,
  SavedQuery,
  SavedConnection,
} from "@/services/api";
import {
  getConnectionIcon,
  isMysqlFamilyDriver,
  supportsSSLCA,
  supportsCreateDatabase,
  supportsRoutines,
  supportsSchemaBrowsing,
  getTreeConfig,
} from "@/lib/driver-registry";
import type { TreeCallbacks } from "@/lib/tree-adapters/types.tsx";
import { toast } from "sonner";
import { TreeNode } from "./connection-list/TreeNode";
import { ConnectionDialog } from "./connection-list/ConnectionDialog";
import {
  getExportDefaultName,
  getExportFilter,
  mergeConnections,
  renderConnectionStatusIndicator,
  sanitizeConnectionErrorMessage,
} from "./connection-list/helpers";
import { useTranslation } from "react-i18next";
import {
  buildConnectionFormDefaults,
  normalizeConnectionFormInput,
} from "@/lib/connection-form/rules";
import { validateConnectionFormInput } from "@/lib/connection-form/validate";
import { isRedisClusterDatabaseList } from "@/components/business/Redis/redis-utils";
import { CreateElasticsearchIndexDialog } from "@/components/business/Elasticsearch/CreateElasticsearchIndexDialog";
import {
  elasticsearchIndexActionSuccessMessage,
  executeElasticsearchIndexAction,
  type ElasticsearchIndexAction,
} from "@/components/business/Elasticsearch/elasticsearch-index-management";

interface Column {
  name: string;
  type: string;
  isPrimaryKey?: boolean;
  nullable?: boolean;
}

interface TableInfo {
  name: string;
  schema: string;
  columns: Column[];
  isSystem?: boolean;
  indexStatus?: string | null;
}

interface RoutineInfo {
  name: string;
  schema: string;
  type: RoutineType;
}

interface SchemaInfo {
  name: string;
  tables: TableInfo[];
  procedures: RoutineInfo[];
  functions: RoutineInfo[];
}

interface DatabaseInfo {
  name: string;
  schemas: SchemaInfo[];
  tables: TableInfo[];
  routines: RoutineInfo[];
  redisCursor?: string;
  redisIsPartial?: boolean;
  redisRequiresPattern?: boolean;
  redisKeyCount?: number;
}

type DatabaseExportFormat = "sql_dml" | "sql_ddl" | "sql_full";
type TableExportFormat = "csv" | "json" | "sql_dml" | "sql_ddl" | "sql_full";

interface Connection {
  id: string;
  name: string;
  type: Driver;
  host: string;
  port: string;
  database?: string;
  username: string;
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
  authMode?: "none" | "basic" | "api_key";
  apiKeyId?: string;
  apiKeySecret?: string;
  apiKeyEncoded?: string;
  cloudId?: string;
  authSource?: string;
  databases: DatabaseInfo[];
  isConnected: boolean;
  connectState: "idle" | "connecting" | "success" | "error";
  connectError?: string;
}

function groupSqlObjectsBySchema(
  tables: TableInfo[],
  routines: RoutineInfo[],
): SchemaInfo[] {
  const groupedTables = tables.reduce<Record<string, TableInfo[]>>(
    (acc, table) => {
      const schemaName = (table.schema || "").trim() || "public";
      const current = acc[schemaName] || [];
      current.push(table);
      acc[schemaName] = current;
      return acc;
    },
    {},
  );
  const groupedRoutines = routines.reduce<Record<string, RoutineInfo[]>>(
    (acc, routine) => {
      const schemaName = (routine.schema || "").trim() || "dbo";
      const current = acc[schemaName] || [];
      current.push(routine);
      acc[schemaName] = current;
      return acc;
    },
    {},
  );
  const schemaNames = Array.from(
    new Set([...Object.keys(groupedTables), ...Object.keys(groupedRoutines)]),
  ).sort((a, b) => a.localeCompare(b));

  return schemaNames.map((name) => {
    const schemaTables = groupedTables[name] || [];
    const schemaRoutines = groupedRoutines[name] || [];
    return {
      name,
      tables: [...schemaTables].sort((a, b) => a.name.localeCompare(b.name)),
      procedures: schemaRoutines
        .filter((routine) => routine.type === "procedure")
        .sort((a, b) => a.name.localeCompare(b.name)),
      functions: schemaRoutines
        .filter((routine) => routine.type === "function")
        .sort((a, b) => a.name.localeCompare(b.name)),
    };
  });
}

interface CreateDatabaseForm {
  name: string;
  ifNotExists: boolean;
  charset: string;
  collation: string;
  encoding: string;
  lcCollate: string;
  lcCtype: string;
}

type SelectedTableNode = {
  key: string;
  connectionId: number;
  database: string;
  table: string;
  schema: string;
};

interface DatasourceTreeAdapter {
  supportsSchemaNode: boolean;
  isDatabaseExpandable: boolean;
  listDatabases: () => Promise<string[]>;
  loadDatabaseChildren: (databaseName: string) => Promise<TableInfo[]>;
  shouldSkipTableColumns: boolean;
  getItemIcon: () => ReactNode;
  onItemActivate: (database: DatabaseInfo, table: TableInfo) => void;
  getDatabaseRowActions: (database: DatabaseInfo) => ReactNode | undefined;
  onDatabaseDoubleClick?: (database: DatabaseInfo) => void;
  renderDatabaseFooter: (database: DatabaseInfo, level: number) => ReactNode;
  renderTableContextMenu: (
    database: DatabaseInfo,
    table: TableInfo,
  ) => ReactNode;
  renderDatabaseContextMenu?: (databaseName: string) => ReactNode;
}

const defaultCreateDatabaseForm: CreateDatabaseForm = {
  name: "",
  ifNotExists: true,
  charset: "",
  collation: "",
  encoding: "",
  lcCollate: "",
  lcCtype: "",
};

const createDbNoneOption = "__none__";
const postgresEncodingOptions = [
  "UTF8",
  "SQL_ASCII",
  "BIG5",
  "EUC_CN",
  "EUC_JP",
  "EUC_JIS_2004",
  "EUC_KR",
  "EUC_TW",
  "GB18030",
  "GBK",
  "ISO_8859_5",
  "ISO_8859_6",
  "ISO_8859_7",
  "ISO_8859_8",
  "JOHAB",
  "KOI8R",
  "KOI8U",
  "LATIN1",
  "LATIN2",
  "LATIN3",
  "LATIN4",
  "LATIN5",
  "LATIN6",
  "LATIN7",
  "LATIN8",
  "LATIN9",
  "LATIN10",
  "MULE_INTERNAL",
  "SHIFT_JIS_2004",
  "SJIS",
  "UHC",
  "WIN866",
  "WIN874",
  "WIN1250",
  "WIN1251",
  "WIN1252",
  "WIN1253",
  "WIN1254",
  "WIN1255",
  "WIN1256",
  "WIN1257",
  "WIN1258",
];
const postgresLocaleOptions = [
  "en_US.UTF-8",
  "C",
  "C.UTF-8",
  "zh_CN.UTF-8",
  "ja_JP.UTF-8",
];
const mssqlCollationOptions = [
  "SQL_Latin1_General_CP1_CI_AS",
  "SQL_Latin1_General_CP1_CS_AS",
  "SQL_Latin1_General_CP1_CI_AI",
  "SQL_Latin1_General_CP1_CS_AI",
  "Latin1_General_CI_AS",
  "Latin1_General_CS_AS",
  "Latin1_General_BIN",
  "Latin1_General_BIN2",
  "Latin1_General_100_CI_AS",
  "Latin1_General_100_CS_AS",
  "Latin1_General_100_CI_AI",
  "Latin1_General_100_BIN2",
  "Latin1_General_100_CI_AS_SC",
  "Latin1_General_100_CS_AS_SC",
  "Latin1_General_100_CI_AI_SC",
  "Latin1_General_100_BIN2_UTF8",
  "Latin1_General_100_CI_AS_SC_UTF8",
  "Latin1_General_100_CI_AI_SC_UTF8",
  "SQL_Latin1_General_CP850_CI_AS",
  "Modern_Spanish_CI_AS",
  "Modern_Spanish_100_CI_AS",
  "French_CI_AS",
  "French_100_CI_AS",
  "German_PhoneBook_CI_AS",
  "German_PhoneBook_100_CI_AS",
  "Turkish_CI_AS",
  "Turkish_100_CI_AS",
  "Cyrillic_General_CI_AS",
  "Cyrillic_General_100_CI_AS",
  "Chinese_PRC_CI_AS",
  "Chinese_PRC_CS_AS",
  "Chinese_PRC_100_CI_AS",
  "Chinese_PRC_100_CS_AS",
  "Chinese_PRC_100_BIN2",
  "Chinese_PRC_100_CI_AS_SC",
  "Chinese_PRC_100_CI_AS_SC_UTF8",
  "Chinese_Simplified_Pinyin_100_CI_AS",
  "Chinese_Simplified_Pinyin_100_CS_AS",
  "Chinese_Traditional_Stroke_Order_100_CI_AS",
  "Japanese_CI_AS",
  "Japanese_CS_AS",
  "Japanese_BIN2",
  "Japanese_XJIS_100_CI_AS",
  "Japanese_XJIS_100_CS_AS",
  "Japanese_XJIS_100_BIN2",
  "Japanese_XJIS_140_CI_AS",
  "Japanese_XJIS_140_CI_AS_KS_WS",
  "Japanese_Bushu_Kakusu_100_CI_AS",
  "Japanese_Bushu_Kakusu_140_CI_AS",
  "Korean_Wansung_CI_AS",
  "Korean_Wansung_100_CI_AS",
  "Korean_Wansung_140_CI_AS",
  "Korean_Unicode_CI_AS",
  "Korean_Unicode_100_CI_AS",
  "Korean_Unicode_140_CI_AS",
];

const defaultConnectionDriver: Driver = "postgres";

const buildFormFromConnection = (
  connection: Pick<
    Connection,
    | "type"
    | "name"
    | "host"
    | "port"
    | "database"
    | "username"
    | "ssl"
    | "sslMode"
    | "sslCaCert"
    | "filePath"
    | "sshEnabled"
    | "sshHost"
    | "sshPort"
    | "sshUsername"
    | "sshKeyPath"
    | "mode"
    | "seedNodes"
    | "sentinels"
    | "connectTimeoutMs"
    | "serviceName"
    | "sentinelPassword"
    | "authMode"
    | "apiKeyId"
    | "apiKeySecret"
    | "apiKeyEncoded"
    | "cloudId"
    | "authSource"
  >,
  overrides: Partial<ConnectionForm> = {},
): ConnectionForm =>
  buildConnectionFormDefaults(connection.type, {
    name: connection.name,
    host: connection.host || "",
    port: Number(connection.port) || undefined,
    database: connection.database || "",
    schema: connection.type === "postgres" ? "public" : "",
    username: connection.username || "",
    password: "",
    ssl: connection.ssl || false,
    sslMode: connection.sslMode || "require",
    sslCaCert: connection.sslCaCert || "",
    filePath: connection.filePath || "",
    sshEnabled: connection.sshEnabled || false,
    sshHost: connection.sshHost || "",
    sshPort: connection.sshPort || undefined,
    sshUsername: connection.sshUsername || "",
    sshPassword: "",
    sshKeyPath: connection.sshKeyPath || "",
    mode: connection.mode,
    seedNodes: connection.seedNodes || [],
    sentinels: connection.sentinels || [],
    connectTimeoutMs: connection.connectTimeoutMs,
    serviceName: connection.serviceName || "",
    sentinelPassword: "",
    authMode: connection.authMode || "none",
    apiKeyId: connection.apiKeyId || "",
    apiKeySecret: "",
    apiKeyEncoded: "",
    cloudId: connection.cloudId || "",
    authSource: connection.authSource || "",
    ...overrides,
  });

const mapSavedConnection = (
  c: SavedConnection,
  fallbackName: string,
): Connection => ({
  id: String(c.id),
  name: c.name || fallbackName,
  type: (c.dbType as Driver) || "postgres",
  host: c.host || "",
  port: String(c.port || ""),
  database: c.database || "",
  username: c.username || "",
  ssl: c.ssl || false,
  sslMode: c.sslMode || "require",
  sslCaCert: c.sslCaCert || "",
  filePath: c.filePath || "",
  sshEnabled: c.sshEnabled || false,
  sshHost: c.sshHost || "",
  sshPort: c.sshPort || 22,
  sshUsername: c.sshUsername || "root",
  sshPassword: c.sshPassword || "",
  sshKeyPath: c.sshKeyPath || "",
  mode: c.mode || undefined,
  seedNodes: c.seedNodes || [],
  sentinels: c.sentinels || [],
  connectTimeoutMs: c.connectTimeoutMs || undefined,
  serviceName: c.serviceName || undefined,
  sentinelPassword: c.sentinelPassword || "",
  authMode: c.authMode || "none",
  apiKeyId: c.apiKeyId || "",
  apiKeySecret: c.apiKeySecret || "",
  apiKeyEncoded: c.apiKeyEncoded || "",
  cloudId: c.cloudId || "",
  authSource: c.authSource || "",
  isConnected: false,
  connectState: "idle",
  connectError: undefined,
  databases: [],
});

interface ConnectionListProps {
  onTableSelect?: (
    connection: string,
    database: string,
    table: string,
    connectionId: number,
    driver: string,
    schema?: string,
  ) => void;
  onConnect?: (form: ConnectionForm) => void;
  onCreateQuery?: (
    connectionId: number,
    databaseName: string,
    driver: string,
  ) => void;
  onRoutineSelect?: (
    connection: string,
    database: string,
    schema: string,
    name: string,
    routineType: RoutineType,
    connectionId: number,
    driver: string,
  ) => void;
  onExportTable?: (
    ctx: {
      connectionId: number;
      database: string;
      schema: string;
      table: string;
      driver: string;
    },
    format: "csv" | "json" | "sql_dml" | "sql_ddl" | "sql_full",
    filePath: string,
  ) => void;
  onExportDatabase?: (ctx: {
    connectionId: number;
    database: string;
    driver: string;
    format: DatabaseExportFormat;
    filePath: string;
  }) => void;
  onCreateTable?: (
    connectionId: number,
    database: string,
    schema: string,
    driver: string,
  ) => void;
  onAlterTable?: (
    connectionId: number,
    database: string,
    schema: string,
    table: string,
    driver: string,
  ) => void;
  activeTableTarget?: {
    connectionId: number;
    database: string;
    table: string;
    schema?: string;
  };
  sidebarRevealRequest?: {
    id: number;
    connectionId: number;
    database: string;
    table: string;
    schema?: string;
  };
  onSelectSavedQuery?: (query: SavedQuery) => void;
  lastUpdated?: number;
  showSavedQueriesInTree?: boolean;
  redisRefreshRequest?: RedisRefreshRequest;
  treeCallbacks?: TreeCallbacks;
}

export interface RedisRefreshRequest {
  id: number;
  connectionId: number;
  database: string;
}

export function ConnectionList({
  onTableSelect,
  onConnect,
  onCreateQuery,
  onRoutineSelect,
  onExportTable,
  onExportDatabase,
  onCreateTable,
  onAlterTable,
  activeTableTarget,
  sidebarRevealRequest,
  onSelectSavedQuery,
  lastUpdated,
  showSavedQueriesInTree = false,
  redisRefreshRequest,
  treeCallbacks,
}: ConnectionListProps) {
  const { t } = useTranslation();
  const tableNodeRefs = useRef<Record<string, HTMLDivElement | null>>({});
  const handledRevealRequestIdRef = useRef<number | null>(null);
  const handledRedisRefreshIdRef = useRef<number | null>(null);
  const [connections, setConnections] = useState<Connection[]>([]);
  const [expandedConnections, setExpandedConnections] = useState<Set<string>>(
    new Set(),
  );
  const [expandedDatabases, setExpandedDatabases] = useState<Set<string>>(
    new Set(),
  );
  // These refs are updated every render so effects can read latest values without
  // listing them as deps (avoids re-firing on every connection state update).
  const connectionsRef = useRef(connections);
  connectionsRef.current = connections;
  const expandedDatabasesRef = useRef(expandedDatabases);
  expandedDatabasesRef.current = expandedDatabases;
  const [expandedDatabaseGroups, setExpandedDatabaseGroups] = useState<
    Set<string>
  >(new Set());
  const [expandedQueryGroups, setExpandedQueryGroups] = useState<Set<string>>(
    new Set(),
  );
  const [expandedSchemas, setExpandedSchemas] = useState<Set<string>>(
    new Set(),
  );
  const [expandedRoutineGroups, setExpandedRoutineGroups] = useState<
    Set<string>
  >(new Set());
  const [expandedTableGroups, setExpandedTableGroups] = useState<Set<string>>(
    new Set(),
  );
  const [expandedTables, setExpandedTables] = useState<Set<string>>(new Set());
  const [selectedTableNode, setSelectedTableNode] =
    useState<SelectedTableNode | null>(null);
  const selectedTableKey = selectedTableNode?.key ?? null;
  const [autoScrollRequest, setAutoScrollRequest] = useState<{
    key: string;
    id: number;
  } | null>(null);
  const [contextMenu, setContextMenu] = useState<{
    visible: boolean;
    x: number;
    y: number;
    connectionId: string | null;
    databaseName?: string | null;
    schemaName?: string | null;
    type: "connection" | "database" | "schema";
  }>({ visible: false, x: 0, y: 0, connectionId: null, type: "connection" });
  const [isDialogOpen, setIsDialogOpen] = useState(false);
  const [dialogMode, setDialogMode] = useState<"create" | "edit">("create");
  const [createStep, setCreateStep] = useState<"type" | "details">("type");
  const [editingConnectionId, setEditingConnectionId] = useState<string | null>(
    null,
  );
  const [loadingDatabaseKeys, setLoadingDatabaseKeys] = useState<Set<string>>(
    new Set(),
  );
  const [loadingTableKeys, setLoadingTableKeys] = useState<Set<string>>(
    new Set(),
  );
  const loadingSpinner = (
    <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />
  );
  const [isTesting, setIsTesting] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);
  const [isSavingEdit, setIsSavingEdit] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [isCreatingDatabase, setIsCreatingDatabase] = useState(false);
  const [isImportingSql, setIsImportingSql] = useState(false);
  const [deleteTargetConnectionId, setDeleteTargetConnectionId] = useState<
    string | null
  >(null);
  const [createDbConnectionId, setCreateDbConnectionId] = useState<
    string | null
  >(null);
  const [isCreateDbDialogOpen, setIsCreateDbDialogOpen] = useState(false);
  const [showCreateDbAdvanced, setShowCreateDbAdvanced] = useState(false);
  const [createDbValidationMsg, setCreateDbValidationMsg] = useState<
    string | null
  >(null);
  const [createDbForm, setCreateDbForm] = useState<CreateDatabaseForm>(
    defaultCreateDatabaseForm,
  );
  const [showElasticsearchSystemIndices, setShowElasticsearchSystemIndices] =
    useState(false);
  const [showMongoSystemCollections, setShowMongoSystemCollections] =
    useState(false);
  const [createEsIndexConnectionId, setCreateEsIndexConnectionId] = useState<
    string | null
  >(null);
  const [isCreateEsIndexDialogOpen, setIsCreateEsIndexDialogOpen] =
    useState(false);
  const [mysqlCharsets, setMysqlCharsets] = useState<string[]>([]);
  const [mysqlCollations, setMysqlCollations] = useState<string[]>([]);
  const [loadingMysqlOptions, setLoadingMysqlOptions] = useState(false);
  const [isLoadingConnections, setIsLoadingConnections] = useState(false);
  const [isLoadingQueries, setIsLoadingQueries] = useState(false);
  const [testMsg, setTestMsg] = useState<{
    ok: boolean;
    text: string;
    latency?: number;
  } | null>(null);
  const [validationMsg, setValidationMsg] = useState<string | null>(null);
  const [form, setForm] = useState<ConnectionForm>(
    buildConnectionFormDefaults(defaultConnectionDriver),
  );
  const [searchTerm, setSearchTerm] = useState("");
  const [savedQueriesByConnection, setSavedQueriesByConnection] = useState<
    Record<string, SavedQuery[]>
  >({});
  const [pendingImport, setPendingImport] = useState<{
    connectionId: string;
    databaseName: string;
    driver: Driver;
    filePath: string;
  } | null>(null);
  const [isImportConfirmOpen, setIsImportConfirmOpen] = useState(false);
  const [pendingDatabaseExport, setPendingDatabaseExport] = useState<{
    connectionId: string;
    databaseName: string;
    driver: Driver;
    format: DatabaseExportFormat;
  } | null>(null);
  const [isDatabaseExportDialogOpen, setIsDatabaseExportDialogOpen] =
    useState(false);
  const [isExportingDatabaseSql, setIsExportingDatabaseSql] = useState(false);
  const [pendingTableExport, setPendingTableExport] = useState<{
    connection: Connection;
    database: DatabaseInfo;
    table: TableInfo;
  } | null>(null);
  const [isTableExportDialogOpen, setIsTableExportDialogOpen] = useState(false);
  const [isExportingTable, setIsExportingTable] = useState(false);
  const [tableExportFormat, setTableExportFormat] =
    useState<TableExportFormat>("csv");

  const supportsCreateDatabaseForDriver = (driver: Driver) =>
    supportsCreateDatabase(driver);
  const supportsSchemaNodeForDriver = (driver: Driver) =>
    supportsSchemaBrowsing(driver);
  const getSchemaNodeKey = (databaseKey: string, schema: string) =>
    `${databaseKey}::${schema}`;
  const getRoutineGroupNodeKey = (
    databaseKey: string,
    schema: string,
    routineType: RoutineType,
  ) => `${databaseKey}::${schema}::${routineType}`;
  const getTableGroupNodeKey = (databaseKey: string, schema: string) =>
    `${databaseKey}::${schema}::tables`;
  const getTableNodeKey = (
    connectionId: string,
    databaseName: string,
    schemaName: string,
    tableName: string,
  ) => `${connectionId}-${databaseName}-${schemaName}-${tableName}`;

  const createDbTargetConnection = useMemo(
    () => connections.find((conn) => conn.id === createDbConnectionId) || null,
    [connections, createDbConnectionId],
  );
  const createDbTargetDriver = createDbTargetConnection?.type;
  const isMySqlFamilyCreateDb = createDbTargetDriver
    ? isMysqlFamilyDriver(createDbTargetDriver as any)
    : false;
  const isPostgresCreateDb = createDbTargetDriver === "postgres";
  const isMssqlCreateDb = createDbTargetDriver === "mssql";

  useEffect(() => {
    if (
      !isCreateDbDialogOpen ||
      !isMySqlFamilyCreateDb ||
      !createDbConnectionId
    )
      return;
    setLoadingMysqlOptions(true);
    api.connections
      .getMysqlCharsets(Number(createDbConnectionId))
      .then(setMysqlCharsets)
      .catch(() => setMysqlCharsets(["utf8mb4", "utf8", "latin1"]))
      .finally(() => setLoadingMysqlOptions(false));
  }, [isCreateDbDialogOpen, isMySqlFamilyCreateDb, createDbConnectionId]);

  useEffect(() => {
    if (
      !isCreateDbDialogOpen ||
      !isMySqlFamilyCreateDb ||
      !createDbConnectionId
    )
      return;
    api.connections
      .getMysqlCollations(
        Number(createDbConnectionId),
        createDbForm.charset || undefined,
      )
      .then(setMysqlCollations)
      .catch(() => setMysqlCollations([]));
  }, [
    isCreateDbDialogOpen,
    isMySqlFamilyCreateDb,
    createDbConnectionId,
    createDbForm.charset,
  ]);

  const getConnectionStatusLabel = (connection: Connection) => {
    if (connection.connectState === "success") {
      return t("connection.status.connected");
    }
    if (connection.connectState === "error") {
      if (connection.connectError) {
        return t("connection.status.failedWithReason", {
          error: connection.connectError,
        });
      }
      return t("connection.status.failed");
    }
    if (connection.connectState === "connecting") {
      return t("connection.status.connecting");
    }
    return t("connection.status.idle");
  };

  const filteredConnections = useMemo(() => {
    if (!searchTerm) return connections;
    const lowerTerm = searchTerm.toLowerCase();
    return connections
      .map((conn) => {
        const filteredDbs = conn.databases
          .map((db) => {
            const filteredSchemas = db.schemas
              .map((schema) => {
                const filteredTables = schema.tables.filter((t) =>
                  t.name.toLowerCase().includes(lowerTerm),
                );
                const filteredProcedures = schema.procedures.filter((routine) =>
                  routine.name.toLowerCase().includes(lowerTerm),
                );
                const filteredFunctions = schema.functions.filter((routine) =>
                  routine.name.toLowerCase().includes(lowerTerm),
                );
                if (
                  filteredTables.length > 0 ||
                  filteredProcedures.length > 0 ||
                  filteredFunctions.length > 0
                ) {
                  return {
                    ...schema,
                    tables: filteredTables,
                    procedures: filteredProcedures,
                    functions: filteredFunctions,
                  };
                }
                return null;
              })
              .filter(Boolean) as SchemaInfo[];
            const filteredTables = db.tables.filter((t) =>
              t.name.toLowerCase().includes(lowerTerm),
            );
            if (filteredSchemas.length > 0 || filteredTables.length > 0) {
              return {
                ...db,
                schemas: filteredSchemas,
                tables: filteredTables,
              };
            }
            return null;
          })
          .filter(Boolean) as DatabaseInfo[];

        const hasMatchingQuery =
          showSavedQueriesInTree &&
          (savedQueriesByConnection[conn.id] || []).some((query) =>
            query.name.toLowerCase().includes(lowerTerm),
          );

        if (filteredDbs.length > 0 || hasMatchingQuery) {
          return { ...conn, databases: filteredDbs };
        }
        return null;
      })
      .filter(Boolean) as Connection[];
  }, [
    connections,
    savedQueriesByConnection,
    searchTerm,
    showSavedQueriesInTree,
  ]);

  useEffect(() => {
    if (searchTerm) {
      setExpandedConnections((prev) => {
        const next = new Set(prev);
        filteredConnections.forEach((conn) => {
          next.add(conn.id);
        });
        return next;
      });
      setExpandedDatabases((prev) => {
        const next = new Set(prev);
        filteredConnections.forEach((conn) => {
          conn.databases.forEach((db) => {
            next.add(`${conn.id}-${db.name}`);
          });
        });
        return next;
      });
      setExpandedSchemas((prev) => {
        const next = new Set(prev);
        filteredConnections.forEach((conn) => {
          conn.databases.forEach((db) => {
            const databaseKey = `${conn.id}-${db.name}`;
            db.schemas.forEach((schema) => {
              next.add(getSchemaNodeKey(databaseKey, schema.name));
            });
          });
        });
        return next;
      });
      setExpandedRoutineGroups((prev) => {
        const next = new Set(prev);
        filteredConnections.forEach((conn) => {
          conn.databases.forEach((db) => {
            const databaseKey = `${conn.id}-${db.name}`;
            db.schemas.forEach((schema) => {
              if (schema.procedures.length > 0) {
                next.add(
                  getRoutineGroupNodeKey(databaseKey, schema.name, "procedure"),
                );
              }
              if (schema.functions.length > 0) {
                next.add(
                  getRoutineGroupNodeKey(databaseKey, schema.name, "function"),
                );
              }
            });
          });
        });
        return next;
      });
      setExpandedTableGroups((prev) => {
        const next = new Set(prev);
        filteredConnections.forEach((conn) => {
          const supportsSchemaNode = supportsSchemaNodeForDriver(conn.type);
          conn.databases.forEach((db) => {
            const databaseKey = `${conn.id}-${db.name}`;
            if (supportsSchemaNode) {
              db.schemas.forEach((schema) => {
                if (schema.tables.length > 0) {
                  next.add(getTableGroupNodeKey(databaseKey, schema.name));
                }
              });
            } else {
              if (db.tables.length > 0) {
                next.add(getTableGroupNodeKey(databaseKey, db.name));
              }
            }
          });
        });
        return next;
      });
      if (showSavedQueriesInTree) {
        setExpandedDatabaseGroups((prev) => {
          const next = new Set(prev);
          filteredConnections.forEach((conn) => {
            next.add(`${conn.id}::databases`);
          });
          return next;
        });
        setExpandedQueryGroups((prev) => {
          const next = new Set(prev);
          filteredConnections.forEach((conn) => {
            next.add(`${conn.id}::queries`);
          });
          return next;
        });
      }
    }
  }, [searchTerm, filteredConnections, showSavedQueriesInTree]);

  const normalizedForm = useMemo(
    () => normalizeConnectionFormInput(form),
    [form],
  );
  const validationIssues = useMemo(
    () =>
      validateConnectionFormInput(
        normalizedForm,
        dialogMode === "edit" ? "edit" : "create",
      ),
    [normalizedForm, dialogMode],
  );
  const requiredOk = useMemo(() => {
    return validationIssues.length === 0;
  }, [validationIssues]);

  const validateSslSettings = () => {
    if (!form.ssl || !supportsSSLCA(form.driver)) {
      return null;
    }
    if (form.sslMode === "verify_ca" && !(form.sslCaCert || "").trim()) {
      return t("connection.dialog.sslValidation.caRequired");
    }
    return null;
  };

  const getFirstValidationMessage = () => {
    if (validationIssues.length === 0) {
      return null;
    }
    const issue = validationIssues[0];
    return t(issue.key);
  };

  const pickSingleFile = async (params: {
    title: string;
    filters?: { name: string; extensions: string[] }[];
  }) => {
    if (!isTauri()) {
      toast.info(t("connection.toast.fileBrowserDesktopOnly"));
      return null;
    }
    try {
      const selected = await open({
        title: params.title,
        multiple: false,
        filters: params.filters,
      });
      if (selected && typeof selected === "string") {
        return selected;
      }
      return null;
    } catch (e) {
      toast.error(t("connection.toast.openFileDialogFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
      return null;
    }
  };

  const handlePickSslCaCertFile = async () => {
    const selectedPath = await pickSingleFile({
      title: t("connection.dialog.sslCaFileDialogTitle"),
      filters: [
        {
          name: t("connection.dialog.fileFilterCert"),
          extensions: ["pem", "crt", "cer"],
        },
        { name: t("connection.dialog.fileFilterAll"), extensions: ["*"] },
      ],
    });
    if (!selectedPath) return;
    try {
      const content = await readTextFile(selectedPath);
      setForm((f) => ({ ...f, sslCaCert: content }));
    } catch (e) {
      toast.error(t("connection.toast.readFileFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
    }
  };

  const handlePickSshKeyFile = async () => {
    const selectedPath = await pickSingleFile({
      title: t("connection.dialog.sshKeyFileDialogTitle"),
      // SSH private keys are often extensionless (for example ~/.ssh/id_rsa),
      // so filtering by extension can hide valid keys in the native picker.
    });
    if (!selectedPath) return;
    setForm((f) => ({ ...f, sshKeyPath: selectedPath }));
  };

  useEffect(() => {
    fetchConnections();
  }, []);

  useEffect(() => {
    if (!showSavedQueriesInTree) return;
    void fetchSavedQueriesByConnection();
  }, [showSavedQueriesInTree, lastUpdated]);

  const fetchConnections = async () => {
    setIsLoadingConnections(true);
    try {
      const conns = await api.connections.list();
      const mapped = conns.map((c) =>
        mapSavedConnection(c, t("common.unknown")),
      );
      setConnections((prev) => mergeConnections(mapped, prev));
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      console.error("listConnections failed", message);
      toast.error(t("connection.toast.loadConnectionsFailed"), {
        description: message,
      });
    } finally {
      setIsLoadingConnections(false);
    }
  };

  const handlePickDatabaseFile = async (driver: Driver) => {
    const selected = await pickSingleFile({
      title:
        driver === "duckdb"
          ? t("connection.dialog.fileDialogTitleDuckdb")
          : t("connection.dialog.fileDialogTitle"),
      filters: [
        {
          name:
            driver === "duckdb"
              ? t("connection.dialog.fileFilterDuckdb")
              : t("connection.dialog.fileFilterSqlite"),
          extensions:
            driver === "duckdb"
              ? ["duckdb", "db"]
              : ["sqlite", "db", "sqlite3", "db3"],
        },
        {
          name: t("connection.dialog.fileFilterAll"),
          extensions: ["*"],
        },
      ],
    });
    if (!selected) return;
    setForm((current) => ({ ...current, filePath: selected }));
  };

  const fetchSavedQueriesByConnection = async () => {
    setIsLoadingQueries(true);
    try {
      const queries = await api.queries.list();
      const grouped: Record<string, SavedQuery[]> = {};
      queries.forEach((query) => {
        if (!query.connectionId) return;
        const key = String(query.connectionId);
        if (!grouped[key]) grouped[key] = [];
        grouped[key].push(query);
      });
      Object.values(grouped).forEach((items) =>
        items.sort((a, b) => a.name.localeCompare(b.name)),
      );
      setSavedQueriesByConnection(grouped);
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      console.error("Failed to fetch saved queries for tree", message);
      toast.error(t("connection.toast.loadQueriesFailed"), {
        description: message,
      });
    } finally {
      setIsLoadingQueries(false);
    }
  };

  const toggleConnection = (id: string) => {
    const connection = connections.find((conn) => conn.id === id);
    if (!connection) return;
    if (connection.connectState !== "success") return;

    const newExpanded = new Set(expandedConnections);
    if (newExpanded.has(id)) {
      newExpanded.delete(id);
    } else {
      newExpanded.add(id);
    }
    setExpandedConnections(newExpanded);
  };

  const fetchAndSetDatabases = async (
    connectionId: string,
  ): Promise<boolean> => {
    try {
      const current = connections.find((conn) => conn.id === connectionId);
      if (!current) return false;

      // For Redis, fetch full database info including key counts
      let databases: DatabaseInfo[];
      if (current.type === "redis") {
        const redisDbs = await api.redis.listDatabases(Number(current.id));
        databases = redisDbs.map((db) => ({
          name: db.name,
          schemas: [],
          tables: [],
          routines: [],
          redisKeyCount: db.keyCount,
        }));
      } else {
        const dbNames = await getDatasourceTreeAdapter(current).listDatabases();
        databases = dbNames.map((name) => ({
          name,
          schemas: [],
          tables: [],
          routines: [],
        }));
      }

      setConnections((prev) =>
        prev.map((conn) => {
          if (conn.id !== connectionId) return conn;
          return {
            ...conn,
            isConnected: true,
            connectState: "success",
            connectError: undefined,
            databases,
          };
        }),
      );
      return true;
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      const sanitizedMessage = sanitizeConnectionErrorMessage(message);
      console.error("listDatabasesById failed", message);
      setConnections((prev) =>
        prev.map((conn) => {
          if (conn.id !== connectionId) return conn;
          return {
            ...conn,
            isConnected: false,
            connectState: "error",
            connectError: sanitizedMessage || message,
            databases: [],
          };
        }),
      );
      toast.error(t("connection.toast.loadDatabasesFailed"), {
        description: sanitizedMessage || message,
      });
      return false;
    }
  };

  const connectConnection = async (
    connectionId: string,
    options?: { resetTree?: boolean },
  ) => {
    const target = connections.find((conn) => conn.id === connectionId);
    if (!target || target.connectState === "connecting") return;

    if (options?.resetTree) {
      setExpandedConnections((prev) => {
        const next = new Set(prev);
        next.delete(connectionId);
        return next;
      });
      setExpandedDatabases((prev) => {
        const next = new Set(
          [...prev].filter((key) => !key.startsWith(`${connectionId}-`)),
        );
        return next;
      });
      setExpandedSchemas((prev) => {
        const next = new Set(
          [...prev].filter((key) => !key.startsWith(`${connectionId}-`)),
        );
        return next;
      });
      setExpandedTables((prev) => {
        const next = new Set(
          [...prev].filter((key) => !key.startsWith(`${connectionId}-`)),
        );
        return next;
      });
    }

    setConnections((prev) =>
      prev.map((conn) => {
        if (conn.id !== connectionId) return conn;
        return {
          ...conn,
          isConnected: false,
          connectState: "connecting",
          connectError: undefined,
          databases: options?.resetTree ? [] : conn.databases,
        };
      }),
    );

    const ok = await fetchAndSetDatabases(connectionId);
    if (ok) {
      setExpandedConnections((prev) => {
        const next = new Set(prev);
        next.add(connectionId);
        return next;
      });
      return;
    }

    setExpandedConnections((prev) => {
      const next = new Set(prev);
      next.delete(connectionId);
      return next;
    });
  };

  const redisKeyToTableInfo = (key: {
    key: string;
    keyType: string;
    ttl: number;
  }): TableInfo => ({
    name: key.key,
    schema: key.keyType,
    columns: [
      {
        name:
          key.ttl > 0
            ? `ttl ${key.ttl}s`
            : key.ttl === -1
              ? "persist"
              : "expired",
        type: key.keyType,
      },
    ],
  });

  const loadRedisKeysPage = useCallback(
    async (
      connectionId: string,
      databaseName: string,
      cursor: string,
      append: boolean,
    ): Promise<TableInfo[]> => {
      const targetConnection = connectionsRef.current.find(
        (conn) => conn.id === connectionId,
      );
      const isRedisCluster =
        targetConnection &&
        !getDatasourceTreeAdapter(targetConnection).isDatabaseExpandable &&
        isRedisClusterDatabaseList(targetConnection.databases);
      if (isRedisCluster && !searchTerm.trim()) {
        setConnections((prev) =>
          prev.map((conn) => {
            if (conn.id !== connectionId) return conn;
            return {
              ...conn,
              databases: conn.databases.map((db) => {
                if (db.name !== databaseName) return db;
                return {
                  ...db,
                  tables: [],
                  redisCursor: "0",
                  redisIsPartial: false,
                  redisRequiresPattern: true,
                };
              }),
            };
          }),
        );
        return [];
      }
      const pattern = searchTerm.trim() ? `*${searchTerm.trim()}*` : "*";
      const response = await api.redis.scanKeys({
        id: Number(connectionId),
        database: databaseName,
        cursor,
        pattern,
        limit: 200,
      });
      const newKeys = response.keys.map(redisKeyToTableInfo);
      setConnections((prev) =>
        prev.map((conn) => {
          if (conn.id !== connectionId) return conn;
          return {
            ...conn,
            databases: conn.databases.map((db) => {
              if (db.name !== databaseName) return db;
              return {
                ...db,
                tables: append ? [...db.tables, ...newKeys] : newKeys,
                redisCursor: response.cursor,
                redisIsPartial: response.isPartial,
                redisRequiresPattern: false,
              };
            }),
          };
        }),
      );
      return newKeys;
    },
    [searchTerm],
  );

  useEffect(() => {
    connectionsRef.current.forEach((conn) => {
      if (getDatasourceTreeAdapter(conn).isDatabaseExpandable) return;
      conn.databases.forEach((db) => {
        const dbKey = `${conn.id}-${db.name}`;
        if (!expandedDatabasesRef.current.has(dbKey) || db.tables.length === 0)
          return;
        void loadRedisKeysPage(conn.id, db.name, "0", false);
      });
    });
  }, [searchTerm, loadRedisKeysPage]);

  const fetchSqlTablesAsTableInfo = async (
    connectionId: string,
    databaseName: string,
  ): Promise<TableInfo[]> => {
    const tables = await api.metadata.listTables(
      Number(connectionId),
      databaseName,
    );
    return tables.map((table) => ({
      name: table.name,
      schema: table.schema,
      columns: [],
    }));
  };

  const fetchSqlRoutinesAsRoutineInfo = async (
    connectionId: string,
    databaseName: string,
    driver: Driver,
  ): Promise<RoutineInfo[]> => {
    if (!supportsRoutines(driver)) return [];
    try {
      const routines = await api.metadata.listRoutines(
        Number(connectionId),
        databaseName,
      );
      return routines.map((routine) => ({
        name: routine.name,
        schema: routine.schema,
        type: routine.type,
      }));
    } catch (e) {
      console.warn(
        "listRoutines failed",
        e instanceof Error ? e.message : String(e),
      );
      return [];
    }
  };

  const openCreateElasticsearchIndexDialog = (
    connectionId: string,
    _databaseName = "Indices",
  ) => {
    const connection = connections.find((conn) => conn.id === connectionId);
    if (!connection || connection.type !== "elasticsearch") return;
    setCreateEsIndexConnectionId(connectionId);
    setIsCreateEsIndexDialogOpen(true);
  };

  const handleElasticsearchIndexAction = async (
    connectionId: string,
    databaseName: string,
    index: string,
    action: ElasticsearchIndexAction,
  ) => {
    if (action === "delete" && !window.confirm(`Delete index "${index}"?`)) {
      return;
    }

    try {
      await executeElasticsearchIndexAction(
        Number(connectionId),
        index,
        action,
      );
      toast.success(elasticsearchIndexActionSuccessMessage(action, index));
      await handleRefreshDatabaseTables(connectionId, databaseName);
    } catch (e) {
      toast.error(`Failed to ${action} Elasticsearch index`, {
        description: e instanceof Error ? e.message : String(e),
      });
    }
  };

  const getDatasourceTreeAdapter = (
    connection: Connection,
  ): DatasourceTreeAdapter => {
    const config = getTreeConfig(connection.type, treeCallbacks);
    const driverKind =
      connection.type === "redis"
        ? "kv"
        : connection.type === "elasticsearch"
          ? "search"
          : connection.type === "mongodb"
            ? "document"
            : "sql";

    // Build context for callbacks
    const buildContext = () => ({
      connectionId: connection.id,
      connectionName: connection.name,
      connectionType: connection.type,
      driverKind: driverKind as any,
    });

    // Wrap treeCallbacks with ConnectionList internal functions
    const enhancedCallbacks = {
      ...treeCallbacks,
      onCreateIndex: (ctx: any) => {
        openCreateElasticsearchIndexDialog(ctx.connectionId, ctx.databaseName);
        treeCallbacks?.onCreateIndex?.(ctx);
      },
      onIndexAction: async (
        ctx: any,
        action: "refresh" | "open" | "close" | "delete",
      ) => {
        await handleElasticsearchIndexAction(
          ctx.connectionId,
          ctx.databaseName,
          ctx.leafName,
          action,
        );
        treeCallbacks?.onIndexAction?.(ctx, action);
      },
    };

    const configWithEnhancedCallbacks = getTreeConfig(
      connection.type,
      enhancedCallbacks,
    );

    return {
      supportsSchemaNode: config.supportsSchemaNode,
      isDatabaseExpandable: config.databaseExpandable,
      listDatabases: async () => {
        if (config.virtualDatabases) {
          return config.virtualDatabases;
        }
        if (connection.type === "redis") {
          return (await api.redis.listDatabases(Number(connection.id))).map(
            (db) => db.name,
          );
        }
        if (connection.type === "mongodb") {
          return (await api.mongodb.listDatabases(Number(connection.id))).map(
            (db) => db.name,
          );
        }
        return api.metadata.listDatabasesById(Number(connection.id));
      },
      loadDatabaseChildren: async (databaseName: string) => {
        if (connection.type === "redis") {
          await loadRedisKeysPage(connection.id, databaseName, "0", false);
          return [];
        }
        if (connection.type === "elasticsearch") {
          const indices = await api.elasticsearch.listIndices(
            Number(connection.id),
          );
          return indices
            .filter(
              (index) =>
                showElasticsearchSystemIndices ||
                !index.isSystem ||
                searchTerm.trim().startsWith("."),
            )
            .map((index) => ({
              name: index.name,
              schema: "Indices",
              columns: [],
              isSystem: index.isSystem,
              indexStatus: index.status,
            }));
        }
        if (connection.type === "mongodb") {
          const collections = await api.mongodb.listCollections(
            Number(connection.id),
            databaseName,
          );
          return collections
            .filter(
              (col) =>
                showMongoSystemCollections ||
                !col.name.startsWith("system.") ||
                searchTerm.trim().startsWith("system"),
            )
            .map((col) => ({
              name: col.name,
              schema: databaseName,
              columns: [],
              isSystem: col.name.startsWith("system."),
            }));
        }
        return fetchSqlTablesAsTableInfo(connection.id, databaseName);
      },
      shouldSkipTableColumns: driverKind !== "sql",
      getItemIcon: config.leafNodeIcon,
      onItemActivate: (database, table) => {
        const ctx = {
          ...buildContext(),
          databaseName: database.name,
          leafName: table.name,
          leafSchema: table.schema,
          leafMeta: {
            isSystem: table.isSystem,
            indexStatus: table.indexStatus,
          },
        };

        if (configWithEnhancedCallbacks.onLeafActivate) {
          configWithEnhancedCallbacks.onLeafActivate(ctx);
          return;
        }

        // Default SQL behavior
        if (driverKind === "sql") {
          onTableSelect?.(
            connection.name,
            database.name,
            table.name,
            Number(connection.id),
            connection.type,
            table.schema,
          );
        }
      },
      getDatabaseRowActions: (database) => {
        if (!configWithEnhancedCallbacks.getDatabaseActions) return undefined;
        const ctx = {
          ...buildContext(),
          databaseName: database.name,
          databaseMeta: {
            redisKeyCount: database.redisKeyCount,
            redisCursor: database.redisCursor,
            redisIsPartial: database.redisIsPartial,
            redisRequiresPattern: database.redisRequiresPattern,
          },
        };
        return configWithEnhancedCallbacks.getDatabaseActions(ctx);
      },
      onDatabaseDoubleClick: configWithEnhancedCallbacks.onDatabaseDoubleClick
        ? (database) => {
            const ctx = {
              ...buildContext(),
              databaseName: database.name,
              databaseMeta: {
                redisKeyCount: database.redisKeyCount,
              },
            };
            configWithEnhancedCallbacks.onDatabaseDoubleClick!(ctx);
          }
        : undefined,
      renderDatabaseFooter: (database, level) => {
        // Redis footer with pagination
        if (connection.type === "redis") {
          const indent = `${(level + 1) * 12 + 8}px`;
          if (database.redisRequiresPattern) {
            return (
              <span
                key="redis-pattern-required"
                className="block px-3 py-1 text-xs text-muted-foreground"
                style={{ paddingLeft: indent }}
              >
                Enter a search pattern to browse cluster keys safely
              </span>
            );
          }
          if (!database.redisIsPartial) return null;
          return database.redisCursor !== "0" ? (
            <button
              key="redis-load-more"
              className="block w-full px-3 py-1 text-left text-xs text-muted-foreground hover:text-foreground"
              style={{ paddingLeft: indent }}
              onClick={() =>
                void loadRedisKeysPage(
                  connection.id,
                  database.name,
                  database.redisCursor!,
                  true,
                )
              }
            >
              Load more…
            </button>
          ) : (
            <span
              key="redis-capped"
              className="block px-3 py-1 text-xs text-muted-foreground"
              style={{ paddingLeft: indent }}
            >
              Results capped — use a pattern to narrow down
            </span>
          );
        }

        // Elasticsearch footer with system indices toggle
        if (connection.type === "elasticsearch") {
          return (
            <label
              key="elasticsearch-system-indices"
              className="flex items-center gap-2 px-3 py-1 text-xs text-muted-foreground"
              style={{ paddingLeft: `${(level + 1) * 12 + 8}px` }}
              onClick={(e) => e.stopPropagation()}
            >
              <Checkbox
                checked={showElasticsearchSystemIndices}
                onCheckedChange={(checked) =>
                  setShowElasticsearchSystemIndices(checked === true)
                }
              />
              Show system indices
            </label>
          );
        }

        // MongoDB footer with system collections toggle
        if (connection.type === "mongodb") {
          return (
            <label
              key="mongodb-system-collections"
              className="flex items-center gap-2 px-3 py-1 text-xs text-muted-foreground"
              style={{ paddingLeft: `${(level + 1) * 12 + 8}px` }}
              onClick={(e) => e.stopPropagation()}
            >
              <Checkbox
                checked={showMongoSystemCollections}
                onCheckedChange={(checked) =>
                  setShowMongoSystemCollections(checked === true)
                }
              />
              Show system collections
            </label>
          );
        }

        return null;
      },
      renderTableContextMenu: (database, table) => {
        // SQL class databases - use internal functions for export
        if (driverKind === "sql") {
          return (
            <>
              <ContextMenuItem
                onClick={() =>
                  handleCreateQueryFromContext(connection.id, database.name)
                }
              >
                <FileCode className="mr-2 h-4 w-4" />
                {t("connection.menu.newQuery")}
              </ContextMenuItem>
              <ContextMenuItem
                onClick={() =>
                  handleTableExportDialog(connection, database, table)
                }
              >
                <Download className="mr-2 h-4 w-4" />
                {t("connection.menu.exportTable")}
              </ContextMenuItem>
              {onAlterTable ? (
                <ContextMenuItem
                  onClick={() =>
                    onAlterTable(
                      Number(connection.id),
                      database.name,
                      table.schema ?? "",
                      table.name,
                      connection.type,
                    )
                  }
                >
                  <TableIcon className="mr-2 h-4 w-4" />
                  {t("connection.menu.alterTable")}
                </ContextMenuItem>
              ) : null}
            </>
          );
        }

        // Non-SQL databases - use treeConfig
        if (!configWithEnhancedCallbacks.getLeafContextMenuItems) return null;
        const ctx = {
          ...buildContext(),
          databaseName: database.name,
          leafName: table.name,
          leafSchema: table.schema,
          leafMeta: {
            isSystem: table.isSystem,
            indexStatus: table.indexStatus,
          },
        };
        const items = configWithEnhancedCallbacks.getLeafContextMenuItems(ctx);
        if (items.length === 0) return null;
        return (
          <>
            {items.map((item) => (
              <ContextMenuItem
                key={item.key}
                className={item.destructive ? "text-destructive" : ""}
                onClick={item.onClick}
              >
                {item.icon}
                {item.label}
              </ContextMenuItem>
            ))}
          </>
        );
      },
      renderDatabaseContextMenu:
        configWithEnhancedCallbacks.getDatabaseContextMenuItems
          ? (databaseName) => {
              const ctx = {
                ...buildContext(),
                databaseName,
              };
              const items =
                configWithEnhancedCallbacks.getDatabaseContextMenuItems!(ctx);
              return (
                <>
                  {items.map((item) => (
                    <button
                      key={item.key}
                      className="flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-accent"
                      onClick={() => {
                        item.onClick();
                        setContextMenu((prev) => ({ ...prev, visible: false }));
                      }}
                    >
                      {item.icon}
                      {item.label}
                    </button>
                  ))}
                </>
              );
            }
          : undefined,
    };
  };

  const fetchAndSetTables = async (
    connectionId: string,
    databaseName: string,
    options?: { force?: boolean },
  ): Promise<TableInfo[]> => {
    try {
      const targetConnection = connections.find(
        (conn) => conn.id === connectionId,
      );
      if (!targetConnection) {
        return [];
      }
      const datasourceAdapter = getDatasourceTreeAdapter(targetConnection);
      if (!datasourceAdapter.isDatabaseExpandable) {
        await datasourceAdapter.loadDatabaseChildren(databaseName);
        return [];
      }
      const [nextTables, nextRoutines] = await Promise.all([
        datasourceAdapter.loadDatabaseChildren(databaseName),
        fetchSqlRoutinesAsRoutineInfo(
          connectionId,
          databaseName,
          targetConnection.type,
        ),
      ]);
      setConnections((prev) =>
        prev.map((conn) => {
          if (conn.id !== connectionId) return conn;
          const supportsSchemaNode = datasourceAdapter.supportsSchemaNode;
          return {
            ...conn,
            databases: conn.databases.map((db) => {
              if (db.name !== databaseName) return db;
              if (
                !options?.force &&
                (supportsSchemaNode
                  ? db.schemas.length > 0
                  : db.tables.length > 0)
              ) {
                return db;
              }
              if (!supportsSchemaNode) {
                return {
                  ...db,
                  schemas: [],
                  tables: nextTables,
                  routines: nextRoutines,
                };
              }
              return {
                ...db,
                schemas: groupSqlObjectsBySchema(nextTables, nextRoutines),
                tables: [],
              };
            }),
          };
        }),
      );
      return nextTables;
    } catch (e) {
      console.error(
        "listTables failed",
        e instanceof Error ? e.message : String(e),
      );
      return [];
    }
  };

  // Sync UI state (expansion, selection) and load data if needed.
  useEffect(() => {
    if (!activeTableTarget) {
      setSelectedTableNode(null);
      return;
    }

    const connectionId = String(activeTableTarget.connectionId);
    const databaseName = activeTableTarget.database;
    const tableName = activeTableTarget.table;
    const schemaName = activeTableTarget.schema || "";
    const dbKey = `${connectionId}-${databaseName}`;
    let cancelled = false;

    setExpandedConnections((prev) => {
      const next = new Set(prev);
      next.add(connectionId);
      return next;
    });
    setExpandedDatabases((prev) => {
      const next = new Set(prev);
      next.add(dbKey);
      return next;
    });

    const ensureDatabaseTablesLoaded = async () => {
      const targetConnection = connections.find(
        (conn) => conn.id === connectionId,
      );
      const targetDatabase = targetConnection?.databases.find(
        (db) => db.name === databaseName,
      );
      if (!targetDatabase) return;

      const supportsSchemaNode = supportsSchemaNodeForDriver(
        targetConnection?.type || "postgres",
      );
      const hasLoadedTables = supportsSchemaNode
        ? targetDatabase.schemas.length > 0
        : targetDatabase.tables.length > 0;
      let availableTables = supportsSchemaNode
        ? targetDatabase.schemas.flatMap((schema) => schema.tables)
        : targetDatabase.tables;
      if (!hasLoadedTables) {
        availableTables = await fetchAndSetTables(connectionId, databaseName);
      }
      if (cancelled) return;
      const resolvedSchema =
        schemaName ||
        availableTables.find((table) => table.name === tableName)?.schema ||
        "";
      if (supportsSchemaNode && resolvedSchema) {
        setExpandedSchemas((prev) => {
          const next = new Set(prev);
          next.add(getSchemaNodeKey(dbKey, resolvedSchema));
          return next;
        });
      }
      const resolvedTableKey = getTableNodeKey(
        connectionId,
        databaseName,
        resolvedSchema,
        tableName,
      );
      setSelectedTableNode({
        key: resolvedTableKey,
        connectionId: activeTableTarget.connectionId,
        database: databaseName,
        table: tableName,
        schema: resolvedSchema,
      });
    };

    void ensureDatabaseTablesLoaded();
    return () => {
      cancelled = true;
    };
  }, [activeTableTarget, connections]);

  useEffect(() => {
    if (!sidebarRevealRequest || !activeTableTarget || !selectedTableNode)
      return;
    if (handledRevealRequestIdRef.current === sidebarRevealRequest.id) return;
    if (
      sidebarRevealRequest.connectionId !== activeTableTarget.connectionId ||
      sidebarRevealRequest.database !== activeTableTarget.database ||
      sidebarRevealRequest.table !== activeTableTarget.table
    ) {
      return;
    }
    if (
      selectedTableNode.connectionId !== sidebarRevealRequest.connectionId ||
      selectedTableNode.database !== sidebarRevealRequest.database ||
      selectedTableNode.table !== sidebarRevealRequest.table
    ) {
      return;
    }
    if (
      sidebarRevealRequest.schema &&
      sidebarRevealRequest.schema !== selectedTableNode.schema
    ) {
      return;
    }

    handledRevealRequestIdRef.current = sidebarRevealRequest.id;
    setAutoScrollRequest({
      key: selectedTableNode.key,
      id: sidebarRevealRequest.id,
    });
  }, [activeTableTarget, selectedTableNode, sidebarRevealRequest]);

  useEffect(() => {
    if (!redisRefreshRequest) return;
    if (handledRedisRefreshIdRef.current === redisRefreshRequest.id) return;
    handledRedisRefreshIdRef.current = redisRefreshRequest.id;
    const dbKey = `${String(redisRefreshRequest.connectionId)}-${redisRefreshRequest.database}`;
    if (!expandedDatabasesRef.current.has(dbKey)) return;
    void loadRedisKeysPage(
      String(redisRefreshRequest.connectionId),
      redisRefreshRequest.database,
      "0",
      false,
    );
  }, [redisRefreshRequest, loadRedisKeysPage]);

  useEffect(() => {
    if (!autoScrollRequest) return;
    let cancelled = false;
    let retriesLeft = 12;
    let frame1 = 0;
    let frame2 = 0;

    const run = () => {
      frame1 = requestAnimationFrame(() => {
        frame2 = requestAnimationFrame(() => {
          if (cancelled) return;
          const target = tableNodeRefs.current[autoScrollRequest.key];
          if (target) {
            target.scrollIntoView({
              block: "center",
              inline: "nearest",
              behavior: "auto",
            });
            setAutoScrollRequest((prev) =>
              prev?.id === autoScrollRequest.id ? null : prev,
            );
            return;
          }

          retriesLeft -= 1;
          if (retriesLeft > 0) {
            run();
            return;
          }

          setAutoScrollRequest((prev) =>
            prev?.id === autoScrollRequest.id ? null : prev,
          );
        });
      });
    };

    run();

    return () => {
      cancelled = true;
      if (frame1) cancelAnimationFrame(frame1);
      if (frame2) cancelAnimationFrame(frame2);
    };
  }, [autoScrollRequest]);

  const handleRefreshDatabaseTables = async (
    connectionId: string,
    databaseName: string,
  ) => {
    const databaseKey = `${connectionId}-${databaseName}`;
    const tableKeyPrefix = `${databaseKey}-`;
    const schemaKeyPrefix = `${databaseKey}::`;
    setExpandedSchemas((prev) => {
      const next = new Set(
        [...prev].filter((key) => !key.startsWith(schemaKeyPrefix)),
      );
      return next;
    });
    setExpandedRoutineGroups((prev) => {
      const next = new Set(
        [...prev].filter((key) => !key.startsWith(schemaKeyPrefix)),
      );
      return next;
    });
    setExpandedTableGroups((prev) => {
      const next = new Set(
        [...prev].filter((key) => !key.startsWith(schemaKeyPrefix)),
      );
      return next;
    });
    setExpandedTables((prev) => {
      const next = new Set(
        [...prev].filter((key) => !key.startsWith(tableKeyPrefix)),
      );
      return next;
    });

    await fetchAndSetTables(connectionId, databaseName, { force: true });
  };

  useEffect(() => {
    connections
      .filter(
        (connection) =>
          connection.type === "elasticsearch" &&
          connection.connectState === "success" &&
          expandedDatabases.has(`${connection.id}-Indices`),
      )
      .forEach((connection) => {
        void handleRefreshDatabaseTables(connection.id, "Indices");
      });
    // Re-apply the client-side system-index filter for already opened ES trees.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showElasticsearchSystemIndices]);

  useEffect(() => {
    connections
      .filter(
        (connection) =>
          connection.type === "mongodb" &&
          connection.connectState === "success",
      )
      .forEach((connection) => {
        connection.databases.forEach((db) => {
          if (expandedDatabases.has(`${connection.id}-${db.name}`)) {
            void handleRefreshDatabaseTables(connection.id, db.name);
          }
        });
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showMongoSystemCollections]);

  const toggleDatabase = (key: string) => {
    const newExpanded = new Set(expandedDatabases);
    if (newExpanded.has(key)) {
      newExpanded.delete(key);
    } else {
      newExpanded.add(key);
      // When expanding, try to load tables (if not loaded)
      // Key format is "connectionId-dbName"
      const [connId, ...dbNameParts] = key.split("-");
      const dbName = dbNameParts.join("-");
      // Find the corresponding connection and database
      const conn = connections.find((c) => c.id === connId);
      if (conn) {
        const db = conn.databases.find((d) => d.name === dbName);
        if (
          db &&
          (supportsSchemaNodeForDriver(conn.type)
            ? db.schemas.length === 0
            : db.tables.length === 0)
        ) {
          setLoadingDatabaseKeys((prev) => new Set(prev).add(key));
          fetchAndSetTables(connId, dbName).finally(() => {
            setLoadingDatabaseKeys((prev) => {
              const next = new Set(prev);
              next.delete(key);
              return next;
            });
          });
        }
      }
    }
    setExpandedDatabases(newExpanded);
  };

  const toggleQueryGroup = (key: string) => {
    setExpandedQueryGroups((prev) => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  };

  const toggleDatabaseGroup = (key: string) => {
    setExpandedDatabaseGroups((prev) => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  };

  const toggleSchema = (schemaKey: string) => {
    setExpandedSchemas((prev) => {
      const next = new Set(prev);
      if (next.has(schemaKey)) {
        next.delete(schemaKey);
      } else {
        next.add(schemaKey);
      }
      return next;
    });
  };

  const toggleRoutineGroup = (groupKey: string) => {
    setExpandedRoutineGroups((prev) => {
      const next = new Set(prev);
      if (next.has(groupKey)) {
        next.delete(groupKey);
      } else {
        next.add(groupKey);
      }
      return next;
    });
  };

  const toggleTableGroup = (groupKey: string) => {
    setExpandedTableGroups((prev) => {
      const next = new Set(prev);
      if (next.has(groupKey)) {
        next.delete(groupKey);
      } else {
        next.add(groupKey);
      }
      return next;
    });
  };

  const fetchAndSetTableColumns = async (
    connectionId: string,
    databaseName: string,
    schema: string,
    tableName: string,
  ) => {
    try {
      const metadata = await api.metadata.getTableMetadata(
        Number(connectionId),
        databaseName,
        schema,
        tableName,
      );
      setConnections((prev) =>
        prev.map((conn) => {
          if (conn.id !== connectionId) return conn;
          return {
            ...conn,
            databases: conn.databases.map((db) => {
              if (db.name !== databaseName) return db;
              return {
                ...db,
                schemas: db.schemas.map((schemaNode) => ({
                  ...schemaNode,
                  tables: schemaNode.tables.map((t) => {
                    if (t.name !== tableName || t.schema !== schema) return t;
                    if (t.columns.length > 0) return t;
                    return {
                      ...t,
                      columns: metadata.columns.map((c) => ({
                        name: c.name,
                        type: c.type,
                        isPrimaryKey: c.primaryKey,
                        nullable: c.nullable,
                      })),
                    };
                  }),
                })),
                tables: db.tables.map((t) => {
                  if (t.name !== tableName || t.schema !== schema) return t;
                  if (t.columns.length > 0) return t;
                  return {
                    ...t,
                    columns: metadata.columns.map((c) => ({
                      name: c.name,
                      type: c.type,
                      isPrimaryKey: c.primaryKey,
                      nullable: c.nullable,
                    })),
                  };
                }),
              };
            }),
          };
        }),
      );
    } catch (e) {
      console.error(
        "getTableMetadata failed",
        e instanceof Error ? e.message : String(e),
      );
    }
  };

  const toggleTable = (
    tableKey: string,
    connectionId: string,
    databaseName: string,
    table: TableInfo,
  ) => {
    const newExpanded = new Set(expandedTables);
    if (newExpanded.has(tableKey)) {
      newExpanded.delete(tableKey);
    } else {
      newExpanded.add(tableKey);
      const conn = connections.find((c) => c.id === connectionId);
      if (conn && getDatasourceTreeAdapter(conn).shouldSkipTableColumns) {
        setExpandedTables(newExpanded);
        return;
      }
      // Load column info on first expand
      if (table.columns.length === 0) {
        setLoadingTableKeys((prev) => new Set(prev).add(tableKey));
        fetchAndSetTableColumns(
          connectionId,
          databaseName,
          table.schema,
          table.name,
        ).finally(() => {
          setLoadingTableKeys((prev) => {
            const next = new Set(prev);
            next.delete(tableKey);
            return next;
          });
        });
      }
    }
    setExpandedTables(newExpanded);
  };

  const handleTableClick = (
    connection: Connection,
    database: DatabaseInfo,
    table: TableInfo,
  ) => {
    getDatasourceTreeAdapter(connection).onItemActivate(database, table);
  };

  const handleRoutineClick = (
    connection: Connection,
    database: DatabaseInfo,
    routine: RoutineInfo,
  ) => {
    onRoutineSelect?.(
      connection.name,
      database.name,
      routine.schema,
      routine.name,
      routine.type,
      Number(connection.id),
      connection.type,
    );
  };

  const handleCreateQueryFromContext = (
    connectionId: string | null | undefined,
    databaseName?: string | null,
  ) => {
    if (!onCreateQuery || !connectionId) return;
    const connection = connections.find((c) => c.id === connectionId);
    if (!connection) return;

    const explicitDatabaseName = (databaseName || "").trim();
    const fallbackDatabaseName =
      (connection.database || "").trim() ||
      connection.databases.find((db) => db.name.trim().length > 0)?.name ||
      (connection.type === "sqlite" || connection.type === "duckdb"
        ? "main"
        : "");
    const resolvedDatabaseName = explicitDatabaseName || fallbackDatabaseName;

    if (!resolvedDatabaseName) {
      toast.error(t("connection.toast.newQueryNoDatabase"));
      return;
    }

    onCreateQuery(Number(connectionId), resolvedDatabaseName, connection.type);
  };

  const openCreateDatabaseDialog = (connectionId: string) => {
    const connection = connections.find((conn) => conn.id === connectionId);
    if (!connection || !supportsCreateDatabaseForDriver(connection.type)) {
      return;
    }
    setCreateDbConnectionId(connectionId);
    setCreateDbValidationMsg(null);
    setShowCreateDbAdvanced(false);
    setCreateDbForm(defaultCreateDatabaseForm);
    setIsCreateDbDialogOpen(true);
  };

  const clearConnectionTreeCache = (connectionId: string) => {
    setConnections((prev) =>
      prev.map((conn) =>
        conn.id === connectionId ? { ...conn, databases: [] } : conn,
      ),
    );
    setExpandedDatabases(
      (prev) =>
        new Set([...prev].filter((key) => !key.startsWith(`${connectionId}-`))),
    );
    setExpandedSchemas(
      (prev) =>
        new Set([...prev].filter((key) => !key.startsWith(`${connectionId}-`))),
    );
    setExpandedTables(
      (prev) =>
        new Set([...prev].filter((key) => !key.startsWith(`${connectionId}-`))),
    );
  };

  const handleCreateDatabase = async () => {
    const connection = createDbTargetConnection;
    if (!connection || !supportsCreateDatabaseForDriver(connection.type))
      return;

    const name = createDbForm.name.trim();
    if (!name) {
      setCreateDbValidationMsg(
        t("connection.createDbDialog.validation.requiredName"),
      );
      return;
    }

    const payload: CreateDatabasePayload = {
      name,
      ifNotExists: createDbForm.ifNotExists,
    };
    if (isMySqlFamilyCreateDb) {
      if (createDbForm.charset.trim())
        payload.charset = createDbForm.charset.trim();
      if (createDbForm.collation.trim()) {
        payload.collation = createDbForm.collation.trim();
      }
    } else if (isPostgresCreateDb) {
      if (createDbForm.encoding.trim())
        payload.encoding = createDbForm.encoding.trim();
      if (createDbForm.lcCollate.trim()) {
        payload.lcCollate = createDbForm.lcCollate.trim();
      }
      if (createDbForm.lcCtype.trim())
        payload.lcCtype = createDbForm.lcCtype.trim();
    } else if (isMssqlCreateDb) {
      if (createDbForm.collation.trim()) {
        payload.collation = createDbForm.collation.trim();
      }
    }

    setCreateDbValidationMsg(null);
    setIsCreatingDatabase(true);
    try {
      await api.connections.createDatabase(Number(connection.id), payload);
      toast.success(t("connection.toast.createDatabaseSuccess"), {
        description: name,
      });
      setIsCreateDbDialogOpen(false);
      clearConnectionTreeCache(connection.id);
      const loaded = await fetchAndSetDatabases(connection.id);
      if (loaded) {
        setExpandedConnections((prev) => {
          const next = new Set(prev);
          next.add(connection.id);
          return next;
        });
      }
    } catch (e) {
      toast.error(t("connection.toast.createDatabaseFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setIsCreatingDatabase(false);
    }
  };

  const handleTestConnection = async () => {
    try {
      setValidationMsg(null);
      const fieldValidationError = getFirstValidationMessage();
      if (fieldValidationError) {
        setValidationMsg(fieldValidationError);
        return;
      }
      const sslError = validateSslSettings();
      if (sslError) {
        setValidationMsg(sslError);
        return;
      }
      setIsTesting(true);
      setTestMsg(null);
      const res = await api.connections.testEphemeral(normalizedForm);
      setTestMsg({
        ok: res.success,
        text: res.message,
        latency: res.latencyMs,
      });
    } catch (e: any) {
      setTestMsg({ ok: false, text: String(e?.message || e) });
    } finally {
      setIsTesting(false);
    }
  };

  const handleConnect = async () => {
    if (!requiredOk) {
      setValidationMsg(getFirstValidationMessage());
      return;
    }
    setValidationMsg(null);
    const sslError = validateSslSettings();
    if (sslError) {
      setValidationMsg(sslError);
      return;
    }
    setIsConnecting(true);
    try {
      const res = await api.connections.create(normalizedForm);
      setConnections((prev) => [
        mapSavedConnection(res, t("common.unknown")),
        ...prev,
      ]);
      setIsDialogOpen(false);
      setCreateStep("type");
      setForm(buildConnectionFormDefaults(defaultConnectionDriver));
      if (onConnect) onConnect(normalizedForm);
    } catch (e: any) {
      setValidationMsg(String(e?.message || e));
    } finally {
      setIsConnecting(false);
    }
  };

  const handleSaveEdit = async () => {
    if (!editingConnectionId) return;
    if (!requiredOk) {
      setValidationMsg(getFirstValidationMessage());
      return;
    }

    setValidationMsg(null);
    const sslError = validateSslSettings();
    if (sslError) {
      setValidationMsg(sslError);
      return;
    }
    setIsSavingEdit(true);
    try {
      await api.connections.update(Number(editingConnectionId), normalizedForm);
      await fetchConnections();
      setIsDialogOpen(false);
      setDialogMode("create");
      setCreateStep("type");
      setEditingConnectionId(null);
      setForm(buildConnectionFormDefaults(defaultConnectionDriver));
    } catch (e: any) {
      setValidationMsg(String(e?.message || e));
    } finally {
      setIsSavingEdit(false);
    }
  };

  const handleDialogSubmit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (dialogMode === "edit") {
      void handleSaveEdit();
      return;
    }
    void handleConnect();
  };

  const resetConnectionDialogFeedback = () => {
    setValidationMsg(null);
    setTestMsg(null);
  };

  const closeConnectionDialog = () => {
    setIsDialogOpen(false);
    setDialogMode("create");
    setCreateStep("type");
    setEditingConnectionId(null);
    resetConnectionDialogFeedback();
    setForm(buildConnectionFormDefaults(defaultConnectionDriver));
  };

  const openCreateDialog = () => {
    setDialogMode("create");
    setCreateStep("type");
    setEditingConnectionId(null);
    resetConnectionDialogFeedback();
    setForm(buildConnectionFormDefaults(defaultConnectionDriver));
    setIsDialogOpen(true);
  };

  const openEditDialog = (connectionId: string) => {
    const conn = connections.find((c) => c.id === connectionId);
    if (!conn) return;

    setDialogMode("edit");
    setCreateStep("details");
    setEditingConnectionId(connectionId);
    resetConnectionDialogFeedback();
    setForm(buildFormFromConnection(conn));
    setIsDialogOpen(true);
  };

  const handleCreateDriverSelect = (driver: Driver) => {
    setForm((current) =>
      buildConnectionFormDefaults(driver, {
        name: current.name,
      }),
    );
    resetConnectionDialogFeedback();
    setCreateStep("details");
  };

  const handleReconnect = async (connectionId: string) => {
    await connectConnection(connectionId, { resetTree: true });
  };

  const buildDuplicateConnectionName = (sourceName: string) => {
    const baseName = `${sourceName}-${t("connection.menu.copy")}`;
    let candidate = baseName;
    let counter = 2;
    while (connections.some((conn) => conn.name === candidate)) {
      candidate = `${baseName}-${counter}`;
      counter += 1;
    }
    return candidate;
  };

  const handleDuplicateConnection = async (connectionId: string) => {
    const source = connections.find((conn) => conn.id === connectionId);
    if (!source) return;

    const duplicateName = buildDuplicateConnectionName(
      source.name || t("common.unknown"),
    );
    const duplicateForm = buildFormFromConnection(source, {
      name: duplicateName,
    });

    try {
      const res = await api.connections.create(duplicateForm);
      setConnections((prev) => [
        mapSavedConnection(res, t("common.unknown")),
        ...prev,
      ]);
      toast.success(t("connection.toast.duplicateSuccess"));
    } catch (e) {
      toast.error(t("connection.toast.duplicateFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
    }
  };

  const handleDeleteConnection = async (connectionId: string) => {
    setIsDeleting(true);
    try {
      await api.connections.delete(Number(connectionId));
      setConnections((prev) => prev.filter((conn) => conn.id !== connectionId));
      setExpandedConnections((prev) => {
        const next = new Set(prev);
        next.delete(connectionId);
        return next;
      });
      setExpandedDatabases((prev) => {
        const next = new Set(
          [...prev].filter((key) => !key.startsWith(`${connectionId}-`)),
        );
        return next;
      });
      setExpandedSchemas((prev) => {
        const next = new Set(
          [...prev].filter((key) => !key.startsWith(`${connectionId}-`)),
        );
        return next;
      });
      setExpandedTables((prev) => {
        const next = new Set(
          [...prev].filter((key) => !key.startsWith(`${connectionId}-`)),
        );
        return next;
      });
      setDeleteTargetConnectionId(null);
    } catch (e) {
      console.error(
        "deleteConnection failed",
        e instanceof Error ? e.message : String(e),
      );
    } finally {
      setIsDeleting(false);
    }
  };

  const handleTableExportDialog = (
    connection: Connection,
    database: DatabaseInfo,
    table: TableInfo,
  ) => {
    if (!onExportTable) return;
    if (!isTauri()) {
      toast.error(t("connection.toast.exportDesktopOnly"));
      return;
    }
    setPendingTableExport({ connection, database, table });
    setTableExportFormat("csv");
    setIsTableExportDialogOpen(true);
  };

  const handleTableExportConfirm = async () => {
    if (!pendingTableExport || !onExportTable) return;
    const { connection, database, table } = pendingTableExport;
    try {
      setIsExportingTable(true);
      const selected = await save({
        title: t("connection.toast.saveExportFile"),
        defaultPath: getExportDefaultName(table.name, tableExportFormat),
        filters: getExportFilter(tableExportFormat),
      });
      if (!selected) return;
      const filePath = Array.isArray(selected) ? selected[0] : selected;
      if (!filePath) return;
      setIsTableExportDialogOpen(false);
      onExportTable(
        {
          connectionId: Number(connection.id),
          database: database.name,
          schema: table.schema,
          table: table.name,
          driver: connection.type,
        },
        tableExportFormat,
        filePath,
      );
      setPendingTableExport(null);
    } catch (e) {
      toast.error(t("connection.toast.openSaveDialogFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setIsExportingTable(false);
    }
  };

  const handleDatabaseImport = async (
    connectionId: string,
    databaseName: string,
  ) => {
    const connection = connections.find((conn) => conn.id === connectionId);
    if (!connection) return;

    const capability = getImportDriverCapability(connection.type);
    if (capability === "read_only_not_supported") {
      toast.error(t("connection.toast.importReadOnlyDriver"));
      return;
    }

    if (capability !== "supported") {
      toast.error(t("connection.toast.importUnsupportedDriver"));
      return;
    }

    if (!isTauri()) {
      toast.error(t("connection.toast.importDesktopOnly"));
      return;
    }

    const selectedPath = await pickSingleFile({
      title: t("connection.toast.selectImportSqlFile"),
      filters: [{ name: "SQL", extensions: ["sql"] }],
    });
    if (!selectedPath) return;

    setPendingImport({
      connectionId,
      databaseName,
      driver: connection.type,
      filePath: selectedPath,
    });
    setIsImportConfirmOpen(true);
  };

  const handleDatabaseExport = async (
    connection: Connection,
    database: DatabaseInfo,
  ) => {
    if (!onExportDatabase) return;
    if (!isTauri()) {
      toast.error(t("connection.toast.exportDesktopOnly"));
      return;
    }

    setPendingDatabaseExport({
      connectionId: connection.id,
      databaseName: database.name,
      driver: connection.type,
      format: "sql_full",
    });
    setIsDatabaseExportDialogOpen(true);
  };

  const handleConfirmDatabaseExport = async () => {
    if (!pendingDatabaseExport || !onExportDatabase) return;
    if (!isTauri()) {
      toast.error(t("connection.toast.exportDesktopOnly"));
      return;
    }

    setIsExportingDatabaseSql(true);
    try {
      const suffix =
        pendingDatabaseExport.format === "sql_ddl"
          ? "ddl"
          : pendingDatabaseExport.format === "sql_dml"
            ? "dml"
            : "full";
      const selected = await save({
        title: t("connection.toast.saveExportFile"),
        defaultPath: getExportDefaultName(
          `${pendingDatabaseExport.databaseName}_${suffix}`,
          pendingDatabaseExport.format,
        ),
        filters: getExportFilter(pendingDatabaseExport.format),
      });
      if (!selected) return;
      const filePath = Array.isArray(selected) ? selected[0] : selected;
      if (!filePath) return;

      onExportDatabase({
        connectionId: Number(pendingDatabaseExport.connectionId),
        database: pendingDatabaseExport.databaseName,
        driver: pendingDatabaseExport.driver,
        format: pendingDatabaseExport.format,
        filePath,
      });
      setIsDatabaseExportDialogOpen(false);
      setPendingDatabaseExport(null);
    } catch (e) {
      toast.error(t("connection.toast.openSaveDialogFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setIsExportingDatabaseSql(false);
    }
  };

  const handleConfirmImport = async () => {
    if (!pendingImport) return;

    setIsImportingSql(true);
    try {
      const result = await api.transfer.importSqlFile({
        id: Number(pendingImport.connectionId),
        database: pendingImport.databaseName,
        filePath: pendingImport.filePath,
        driver: pendingImport.driver,
      });

      if (result.error || result.failedAt) {
        toast.error(t("connection.toast.importFailed"), {
          description: result.error || t("common.unknown"),
        });
      } else {
        toast.success(
          t("connection.toast.importSuccess", {
            count: result.successStatements,
          }),
          {
            description: pendingImport.filePath,
          },
        );
      }

      await handleRefreshDatabaseTables(
        pendingImport.connectionId,
        pendingImport.databaseName,
      );
    } catch (e) {
      toast.error(t("connection.toast.importFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setIsImportingSql(false);
      setIsImportConfirmOpen(false);
      setPendingImport(null);
    }
  };

  const contextMenuConnection = contextMenu.connectionId
    ? connections.find((conn) => conn.id === contextMenu.connectionId)
    : null;
  const contextMenuDatabaseConnection = contextMenu.connectionId
    ? connections.find((conn) => conn.id === contextMenu.connectionId)
    : null;
  const contextMenuDatabaseAdapter = contextMenuDatabaseConnection
    ? getDatasourceTreeAdapter(contextMenuDatabaseConnection)
    : null;

  return (
    <div className="h-full flex flex-col bg-background border-r border-border">
      <div className="px-2 py-1 border-b border-border flex items-center justify-between h-8">
        <div className="flex items-center gap-2">
          <h2 className="font-semibold text-sm">{t("connection.title")}</h2>
          {isLoadingQueries && (
            <Loader2 className="h-3 w-3 animate-spin text-muted-foreground" />
          )}
        </div>
        <div className="flex gap-1">
          <Button
            variant="ghost"
            size="sm"
            className="h-6 w-6 p-0"
            onClick={fetchConnections}
            loading={isLoadingConnections}
          >
            <RefreshCw className="w-3.5 h-3.5" />
          </Button>
          <ConnectionDialog
            open={isDialogOpen}
            onOpenChange={(open) => {
              if (!open) {
                closeConnectionDialog();
                return;
              }
              setIsDialogOpen(true);
            }}
            trigger={
              <Button
                variant="ghost"
                size="sm"
                className="h-6 w-6 p-0"
                onClick={openCreateDialog}
              >
                <Plus className="w-3.5 h-3.5" />
              </Button>
            }
            dialogMode={dialogMode}
            createStep={createStep}
            form={form}
            setForm={setForm}
            validationMsg={validationMsg}
            testMsg={testMsg}
            requiredOk={requiredOk}
            isTesting={isTesting}
            isConnecting={isConnecting}
            isSavingEdit={isSavingEdit}
            onSubmit={handleDialogSubmit}
            onClose={closeConnectionDialog}
            onTestConnection={handleTestConnection}
            onCreateDriverSelect={handleCreateDriverSelect}
            onBackToType={() => setCreateStep("type")}
            onPickSslCaCertFile={() => void handlePickSslCaCertFile()}
            onPickSshKeyFile={() => void handlePickSshKeyFile()}
            onPickDatabaseFile={(driver) => void handlePickDatabaseFile(driver)}
          />
        </div>
      </div>

      <div className="p-2 border-b border-border">
        <div className="relative">
          <Search className="absolute left-2 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder={t("connection.searchTables")}
            value={searchTerm}
            onChange={(e) => {
              setSearchTerm(e.target.value);
            }}
            className="pl-8"
          />
        </div>
      </div>
      <div
        className="flex-1 overflow-auto"
        onClick={() => setContextMenu((prev) => ({ ...prev, visible: false }))}
      >
        {filteredConnections.map((connection) => {
          const datasourceAdapter = getDatasourceTreeAdapter(connection);
          const queriesForConnection = (
            savedQueriesByConnection[connection.id] || []
          ).filter((query) =>
            query.name.toLowerCase().includes(searchTerm.toLowerCase()),
          );
          const visibleDatabases = connection.databases.filter(
            (database) =>
              !["information_schema", "performance_schema"].includes(
                database.name.toLowerCase(),
              ),
          );

          const renderDatabaseTreeNode = (
            database: DatabaseInfo,
            level: number,
          ) => {
            const dbKey = `${connection.id}-${database.name}`;
            const supportsSchemaNode = datasourceAdapter.supportsSchemaNode;
            const renderTableNode = (table: TableInfo, tableLevel: number) => {
              const tableKey = getTableNodeKey(
                connection.id,
                database.name,
                table.schema,
                table.name,
              );
              return (
                <ContextMenu key={tableKey}>
                  <ContextMenuTrigger asChild>
                    <div
                      ref={(el) => {
                        tableNodeRefs.current[tableKey] = el;
                      }}
                    >
                      <TreeNode
                        level={tableLevel}
                        icon={datasourceAdapter.getItemIcon()}
                        label={table.name}
                        isSelected={selectedTableKey === tableKey}
                        isExpanded={expandedTables.has(tableKey)}
                        toggleOnRowClick={false}
                        onToggle={() => {
                          toggleTable(
                            tableKey,
                            connection.id,
                            database.name,
                            table,
                          );
                        }}
                        onDoubleClick={() => {
                          handleTableClick(connection, database, table);
                        }}
                        statusIndicator={
                          loadingTableKeys.has(tableKey) ||
                          table.isSystem ||
                          table.indexStatus === "close" ? (
                            <span className="inline-flex items-center gap-1">
                              {table.indexStatus === "close" ? (
                                <span className="rounded border px-1 text-[10px] leading-4 text-muted-foreground">
                                  closed
                                </span>
                              ) : null}
                              {table.isSystem ? (
                                <span className="rounded border px-1 text-[10px] leading-4 text-muted-foreground">
                                  system
                                </span>
                              ) : null}
                              {loadingTableKeys.has(tableKey)
                                ? loadingSpinner
                                : null}
                            </span>
                          ) : undefined
                        }
                        actions={
                          <div onClick={(e) => e.stopPropagation()}>
                            <Button
                              variant="ghost"
                              size="sm"
                              className="h-6 w-6 p-0"
                              onClick={() =>
                                handleTableClick(connection, database, table)
                              }
                            >
                              <Play className="w-3 h-3" />
                            </Button>
                          </div>
                        }
                      >
                        {table.columns.map((column) => (
                          <div
                            key={column.name}
                            className="flex items-center gap-1 px-2 py-1 hover:bg-accent text-xs"
                            style={{
                              paddingLeft: `${(tableLevel + 1) * 12 + 8}px`,
                            }}
                          >
                            <span className="w-4" />
                            {column.isPrimaryKey ? (
                              <Key className="w-3 h-3 text-yellow-600 shrink-0" />
                            ) : (
                              <span className="w-3 shrink-0" />
                            )}
                            <span className="flex-1 truncate text-foreground">
                              {column.name}
                            </span>
                            <span className="text-muted-foreground text-xs shrink-0">
                              {column.type}
                            </span>
                          </div>
                        ))}
                      </TreeNode>
                    </div>
                  </ContextMenuTrigger>
                  <ContextMenuContent>
                    {datasourceAdapter.renderTableContextMenu(database, table)}
                  </ContextMenuContent>
                </ContextMenu>
              );
            };

            const renderRoutineNode = (
              routine: RoutineInfo,
              routineLevel: number,
            ) => {
              const routineKey = `${connection.id}-${database.name}-${routine.schema}-${routine.type}-${routine.name}`;
              return (
                <TreeNode
                  key={routineKey}
                  level={routineLevel}
                  icon={<FileCode className="w-4 h-4" />}
                  label={routine.name}
                  hideToggle
                  onDoubleClick={() =>
                    handleRoutineClick(connection, database, routine)
                  }
                  actions={
                    <div onClick={(e) => e.stopPropagation()}>
                      <Button
                        variant="ghost"
                        size="sm"
                        className="h-6 w-6 p-0"
                        onClick={() =>
                          handleRoutineClick(connection, database, routine)
                        }
                      >
                        <FileCode className="w-3 h-3" />
                      </Button>
                    </div>
                  }
                >
                  {null}
                </TreeNode>
              );
            };

            const renderRoutineGroup = (
              schemaNode: SchemaInfo,
              routineType: RoutineType,
              groupLevel: number,
            ) => {
              const routines =
                routineType === "procedure"
                  ? schemaNode.procedures
                  : schemaNode.functions;
              const groupKey = getRoutineGroupNodeKey(
                dbKey,
                schemaNode.name,
                routineType,
              );
              return (
                <TreeNode
                  key={groupKey}
                  level={groupLevel}
                  icon={<FolderOpen className="w-4 h-4" />}
                  label={
                    routineType === "procedure"
                      ? t("connection.tree.procedures")
                      : t("connection.tree.functions")
                  }
                  isExpanded={expandedRoutineGroups.has(groupKey)}
                  onToggle={() => toggleRoutineGroup(groupKey)}
                >
                  {routines.length === 0 ? (
                    <div
                      className="px-2 py-1 text-xs text-muted-foreground"
                      style={{ paddingLeft: `${(groupLevel + 1) * 12 + 8}px` }}
                    >
                      {routineType === "procedure"
                        ? t("connection.tree.noProcedures")
                        : t("connection.tree.noFunctions")}
                    </div>
                  ) : (
                    routines.map((routine) =>
                      renderRoutineNode(routine, groupLevel + 1),
                    )
                  )}
                </TreeNode>
              );
            };

            return (
              <TreeNode
                key={dbKey}
                level={level}
                icon={<Database className="w-4 w-4" />}
                label={
                  <>
                    {(connection.type === "sqlite" ||
                      connection.type === "duckdb") &&
                    database.name === "main"
                      ? t(
                          connection.type === "duckdb"
                            ? "connection.duckdbMainLabel"
                            : "connection.sqliteMainLabel",
                        )
                      : database.name}
                    {connection.type === "redis" &&
                      database.redisKeyCount != null && (
                        <span className="ml-1.5 text-[10px] text-muted-foreground font-normal">
                          · {database.redisKeyCount.toLocaleString()}
                        </span>
                      )}
                  </>
                }
                isExpanded={
                  datasourceAdapter.isDatabaseExpandable
                    ? expandedDatabases.has(dbKey)
                    : false
                }
                onToggle={() => toggleDatabase(dbKey)}
                toggleOnRowClick={datasourceAdapter.isDatabaseExpandable}
                hideToggle={!datasourceAdapter.isDatabaseExpandable}
                statusIndicator={
                  loadingDatabaseKeys.has(dbKey) ? loadingSpinner : undefined
                }
                actions={datasourceAdapter.getDatabaseRowActions(database)}
                onDoubleClick={
                  datasourceAdapter.onDatabaseDoubleClick
                    ? () => datasourceAdapter.onDatabaseDoubleClick?.(database)
                    : undefined
                }
                onContextMenu={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  setContextMenu({
                    visible: true,
                    x: e.clientX,
                    y: e.clientY,
                    connectionId: connection.id,
                    databaseName: database.name,
                    type: "database",
                  });
                }}
              >
                {supportsSchemaNode ? (
                  database.schemas.map((schemaNode) => {
                    const schemaKey = getSchemaNodeKey(dbKey, schemaNode.name);
                    return (
                      <TreeNode
                        key={schemaKey}
                        level={level + 1}
                        icon={<FolderOpen className="w-4 h-4" />}
                        label={schemaNode.name}
                        isExpanded={expandedSchemas.has(schemaKey)}
                        onToggle={() => toggleSchema(schemaKey)}
                        onContextMenu={(e) => {
                          e.preventDefault();
                          e.stopPropagation();
                          setContextMenu({
                            visible: true,
                            x: e.clientX,
                            y: e.clientY,
                            connectionId: connection.id,
                            databaseName: database.name,
                            schemaName: schemaNode.name,
                            type: "schema",
                          });
                        }}
                      >
                        {supportsRoutines(connection.type) ? (
                          <>
                            {(() => {
                              const tableGroupKey = getTableGroupNodeKey(
                                dbKey,
                                schemaNode.name,
                              );
                              return (
                                <TreeNode
                                  key={tableGroupKey}
                                  level={level + 2}
                                  icon={<FolderOpen className="w-4 h-4" />}
                                  label={t("connection.tree.tables")}
                                  isExpanded={expandedTableGroups.has(
                                    tableGroupKey,
                                  )}
                                  onToggle={() =>
                                    toggleTableGroup(tableGroupKey)
                                  }
                                >
                                  {schemaNode.tables.length === 0 ? (
                                    <div
                                      className="px-2 py-1 text-xs text-muted-foreground"
                                      style={{
                                        paddingLeft: `${(level + 3) * 12 + 8}px`,
                                      }}
                                    >
                                      {t("connection.tree.noTables")}
                                    </div>
                                  ) : (
                                    schemaNode.tables.map((table) =>
                                      renderTableNode(table, level + 3),
                                    )
                                  )}
                                </TreeNode>
                              );
                            })()}
                            {renderRoutineGroup(
                              schemaNode,
                              "procedure",
                              level + 2,
                            )}
                            {renderRoutineGroup(
                              schemaNode,
                              "function",
                              level + 2,
                            )}
                          </>
                        ) : (
                          schemaNode.tables.map((table) =>
                            renderTableNode(table, level + 2),
                          )
                        )}
                      </TreeNode>
                    );
                  })
                ) : (
                  <>
                    {(() => {
                      const tableGroupKey = getTableGroupNodeKey(
                        dbKey,
                        database.name,
                      );
                      return (
                        <TreeNode
                          key={tableGroupKey}
                          level={level + 1}
                          icon={<FolderOpen className="w-4 h-4" />}
                          label={t("connection.tree.tables")}
                          isExpanded={expandedTableGroups.has(tableGroupKey)}
                          onToggle={() => toggleTableGroup(tableGroupKey)}
                        >
                          {database.tables.length === 0 ? (
                            <div
                              className="px-2 py-1 text-xs text-muted-foreground"
                              style={{
                                paddingLeft: `${(level + 2) * 12 + 8}px`,
                              }}
                            >
                              {t("connection.tree.noTables")}
                            </div>
                          ) : (
                            database.tables.map((table) =>
                              renderTableNode(table, level + 2),
                            )
                          )}
                        </TreeNode>
                      );
                    })()}
                    {supportsRoutines(connection.type) &&
                      (() => {
                        const virtualSchema: SchemaInfo = {
                          name: database.name,
                          tables: database.tables,
                          procedures: database.routines.filter(
                            (r) => r.type === "procedure",
                          ),
                          functions: database.routines.filter(
                            (r) => r.type === "function",
                          ),
                        };
                        return (
                          <>
                            {renderRoutineGroup(
                              virtualSchema,
                              "procedure",
                              level + 1,
                            )}
                            {renderRoutineGroup(
                              virtualSchema,
                              "function",
                              level + 1,
                            )}
                          </>
                        );
                      })()}
                    {datasourceAdapter.renderDatabaseFooter(database, level)}
                  </>
                )}
              </TreeNode>
            );
          };

          return (
            <TreeNode
              key={connection.id}
              level={0}
              icon={getConnectionIcon(connection.type)}
              label={connection.name}
              isExpanded={expandedConnections.has(connection.id)}
              toggleOnRowClick={connection.connectState === "success"}
              onToggle={() => toggleConnection(connection.id)}
              onDoubleClick={() => {
                void connectConnection(connection.id);
              }}
              onContextMenu={(e) => {
                e.preventDefault();
                e.stopPropagation();
                setContextMenu({
                  visible: true,
                  x: e.clientX,
                  y: e.clientY,
                  connectionId: connection.id,
                  type: "connection",
                });
              }}
              leadingIndicator={
                <span
                  className="inline-flex items-center justify-center shrink-0"
                  role="status"
                  aria-label={getConnectionStatusLabel(connection)}
                  title={getConnectionStatusLabel(connection)}
                >
                  {renderConnectionStatusIndicator(connection)}
                </span>
              }
            >
              <>
                {showSavedQueriesInTree ? (
                  <TreeNode
                    level={1}
                    icon={<FileCode className="w-4 h-4" />}
                    label={t("connection.tree.queries")}
                    isExpanded={expandedQueryGroups.has(
                      `${connection.id}::queries`,
                    )}
                    onToggle={() =>
                      toggleQueryGroup(`${connection.id}::queries`)
                    }
                    forceShowToggle={queriesForConnection.length > 0}
                    canToggle={queriesForConnection.length > 0}
                  >
                    {queriesForConnection.map((query) => (
                      <TreeNode
                        key={`conn-query-${query.id}`}
                        level={2}
                        icon={<FileCode className="w-4 h-4" />}
                        label={query.name}
                        toggleOnRowClick={false}
                        canToggle={false}
                        onDoubleClick={() => onSelectSavedQuery?.(query)}
                        onContextMenu={(e) => {
                          e.preventDefault();
                          e.stopPropagation();
                        }}
                      >
                        {null}
                      </TreeNode>
                    ))}
                  </TreeNode>
                ) : null}

                {connection.connectState === "success" ? (
                  showSavedQueriesInTree ? (
                    <TreeNode
                      level={1}
                      icon={<Database className="w-4 h-4" />}
                      label={t("connection.tree.database")}
                      isExpanded={expandedDatabaseGroups.has(
                        `${connection.id}::databases`,
                      )}
                      onToggle={() =>
                        toggleDatabaseGroup(`${connection.id}::databases`)
                      }
                      forceShowToggle={visibleDatabases.length > 0}
                      canToggle={visibleDatabases.length > 0}
                    >
                      {visibleDatabases.map((database) =>
                        renderDatabaseTreeNode(database, 2),
                      )}
                    </TreeNode>
                  ) : (
                    visibleDatabases.map((database) =>
                      renderDatabaseTreeNode(database, 1),
                    )
                  )
                ) : null}
              </>
            </TreeNode>
          );
        })}
      </div>

      {contextMenu.visible && (
        <div
          className="fixed z-50 min-w-[140px] bg-popover border border-border rounded-md shadow-lg py-1"
          style={{ left: contextMenu.x, top: contextMenu.y }}
        >
          {contextMenu.type === "connection" ? (
            <>
              <button
                className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
                onClick={() => {
                  if (contextMenu.connectionId) {
                    openEditDialog(contextMenu.connectionId);
                  }
                  setContextMenu((prev) => ({ ...prev, visible: false }));
                }}
              >
                <Edit3 className="w-4 h-4" />
                {t("connection.menu.edit")}
              </button>
              <button
                className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
                onClick={async () => {
                  if (contextMenu.connectionId) {
                    await handleDuplicateConnection(contextMenu.connectionId);
                  }
                  setContextMenu((prev) => ({ ...prev, visible: false }));
                }}
              >
                <Copy className="w-4 h-4" />
                {t("connection.menu.copy")}
              </button>
              <button
                className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
                onClick={async () => {
                  if (contextMenu.connectionId) {
                    await handleReconnect(contextMenu.connectionId);
                  }
                  setContextMenu((prev) => ({ ...prev, visible: false }));
                }}
              >
                <RefreshCw className="w-4 h-4" />
                {t("connection.menu.refresh")}
              </button>
              <button
                className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
                onClick={() => {
                  handleCreateQueryFromContext(contextMenu.connectionId);
                  setContextMenu((prev) => ({ ...prev, visible: false }));
                }}
              >
                <FileCode className="w-4 h-4" />
                {t("connection.menu.newQuery")}
              </button>
              {contextMenuConnection &&
              supportsCreateDatabaseForDriver(contextMenuConnection.type) ? (
                <button
                  className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
                  onClick={() => {
                    openCreateDatabaseDialog(contextMenuConnection.id);
                    setContextMenu((prev) => ({ ...prev, visible: false }));
                  }}
                >
                  <Plus className="w-4 h-4" />
                  {t("connection.menu.newDatabase")}
                </button>
              ) : null}
              <div className="h-px bg-border my-1" />
              <button
                className="w-full px-3 py-2 text-left text-sm hover:bg-accent text-destructive flex items-center gap-2"
                onClick={() => {
                  if (contextMenu.connectionId) {
                    setDeleteTargetConnectionId(contextMenu.connectionId);
                  }
                  setContextMenu((prev) => ({ ...prev, visible: false }));
                }}
              >
                <Trash2 className="w-4 h-4" />
                {t("connection.menu.delete")}
              </button>
            </>
          ) : contextMenu.type === "database" ? (
            <>
              <button
                className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
                onClick={async () => {
                  if (contextMenu.connectionId && contextMenu.databaseName) {
                    await handleRefreshDatabaseTables(
                      contextMenu.connectionId,
                      contextMenu.databaseName,
                    );
                  }
                  setContextMenu((prev) => ({ ...prev, visible: false }));
                }}
              >
                <RefreshCw className="w-4 h-4" />
                {t("connection.menu.refreshTables")}
              </button>
              {contextMenuDatabaseAdapter?.renderDatabaseContextMenu &&
              contextMenu.databaseName ? (
                contextMenuDatabaseAdapter.renderDatabaseContextMenu(
                  contextMenu.databaseName,
                )
              ) : (
                <>
                  {contextMenu.connectionId &&
                  contextMenu.databaseName &&
                  contextMenuDatabaseConnection &&
                  getImportDriverCapability(
                    contextMenuDatabaseConnection.type,
                  ) !== "unsupported" ? (
                    <button
                      className="w-full px-3 py-2 text-left text-sm hover:bg-accent disabled:opacity-60 disabled:cursor-not-allowed flex items-center gap-2"
                      disabled={
                        getImportDriverCapability(
                          contextMenuDatabaseConnection.type,
                        ) === "read_only_not_supported"
                      }
                      onClick={async () => {
                        await handleDatabaseImport(
                          contextMenu.connectionId!,
                          contextMenu.databaseName!,
                        );
                        setContextMenu((prev) => ({ ...prev, visible: false }));
                      }}
                    >
                      <Upload className="w-4 h-4" />
                      {getImportDriverCapability(
                        contextMenuDatabaseConnection.type,
                      ) === "read_only_not_supported"
                        ? t("connection.menu.importSqlReadOnly")
                        : t("connection.menu.importSql")}
                    </button>
                  ) : null}
                  <button
                    className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
                    onClick={async () => {
                      if (
                        contextMenu.connectionId &&
                        contextMenu.databaseName
                      ) {
                        const connection = connections.find(
                          (conn) => conn.id === contextMenu.connectionId,
                        );
                        const database = connection?.databases.find(
                          (db) => db.name === contextMenu.databaseName,
                        );
                        if (connection && database) {
                          await handleDatabaseExport(connection, database);
                        }
                      }
                      setContextMenu((prev) => ({ ...prev, visible: false }));
                    }}
                  >
                    <Download className="w-4 h-4" />
                    {t("connection.menu.exportDatabaseSql")}
                  </button>
                  <button
                    className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
                    onClick={() => {
                      handleCreateQueryFromContext(
                        contextMenu.connectionId,
                        contextMenu.databaseName,
                      );
                      setContextMenu((prev) => ({ ...prev, visible: false }));
                    }}
                  >
                    <FileCode className="w-4 h-4" />
                    {t("connection.menu.newQuery")}
                  </button>
                  {contextMenu.connectionId &&
                  contextMenu.databaseName &&
                  contextMenuDatabaseConnection &&
                  onCreateTable ? (
                    <button
                      className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
                      onClick={() => {
                        onCreateTable(
                          Number(contextMenu.connectionId),
                          contextMenu.databaseName!,
                          "",
                          contextMenuDatabaseConnection.type,
                        );
                        setContextMenu((prev) => ({ ...prev, visible: false }));
                      }}
                    >
                      <TableIcon className="w-4 h-4" />
                      {t("connection.menu.newTable")}
                    </button>
                  ) : null}
                </>
              )}
            </>
          ) : contextMenu.type === "schema" ? (
            <>
              <button
                className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
                onClick={async () => {
                  if (contextMenu.connectionId && contextMenu.databaseName) {
                    await handleRefreshDatabaseTables(
                      contextMenu.connectionId,
                      contextMenu.databaseName,
                    );
                  }
                  setContextMenu((prev) => ({ ...prev, visible: false }));
                }}
              >
                <RefreshCw className="w-4 h-4" />
                {t("connection.menu.refreshTables")}
              </button>
              <button
                className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
                onClick={() => {
                  handleCreateQueryFromContext(
                    contextMenu.connectionId,
                    contextMenu.databaseName,
                  );
                  setContextMenu((prev) => ({ ...prev, visible: false }));
                }}
              >
                <FileCode className="w-4 h-4" />
                {t("connection.menu.newQuery")}
              </button>
              {contextMenu.connectionId &&
              contextMenu.databaseName &&
              contextMenuConnection &&
              onCreateTable ? (
                <button
                  className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
                  onClick={() => {
                    onCreateTable(
                      Number(contextMenu.connectionId),
                      contextMenu.databaseName!,
                      contextMenu.schemaName ?? "",
                      contextMenuConnection.type,
                    );
                    setContextMenu((prev) => ({ ...prev, visible: false }));
                  }}
                >
                  <TableIcon className="w-4 h-4" />
                  {t("connection.menu.newTable")}
                </button>
              ) : null}
            </>
          ) : null}
        </div>
      )}
      <CreateElasticsearchIndexDialog
        open={isCreateEsIndexDialogOpen}
        connectionId={
          createEsIndexConnectionId ? Number(createEsIndexConnectionId) : null
        }
        onOpenChange={(open) => {
          setIsCreateEsIndexDialogOpen(open);
          if (!open) setCreateEsIndexConnectionId(null);
        }}
        onCreated={async () => {
          if (createEsIndexConnectionId) {
            await handleRefreshDatabaseTables(
              createEsIndexConnectionId,
              "Indices",
            );
          }
        }}
      />
      <Dialog
        open={isCreateDbDialogOpen}
        onOpenChange={(open) => {
          setIsCreateDbDialogOpen(open);
          if (!open) {
            setCreateDbValidationMsg(null);
            setCreateDbConnectionId(null);
            setShowCreateDbAdvanced(false);
            setCreateDbForm(defaultCreateDatabaseForm);
            setMysqlCharsets([]);
            setMysqlCollations([]);
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("connection.createDbDialog.title")}</DialogTitle>
          </DialogHeader>
          <div className="grid gap-4">
            <div className="grid gap-2">
              <Label htmlFor="create-db-name">
                {t("connection.createDbDialog.fields.name")}{" "}
                <span className="text-red-600">*</span>
              </Label>
              <Input
                id="create-db-name"
                value={createDbForm.name}
                onChange={(e) =>
                  setCreateDbForm((prev) => ({ ...prev, name: e.target.value }))
                }
                placeholder={t("connection.createDbDialog.placeholders.name")}
              />
            </div>
            <div className="flex items-center space-x-2">
              <Checkbox
                id="create-db-if-not-exists"
                checked={createDbForm.ifNotExists}
                onCheckedChange={(checked) =>
                  setCreateDbForm((prev) => ({
                    ...prev,
                    ifNotExists: checked === true,
                  }))
                }
              />
              <Label htmlFor="create-db-if-not-exists">
                {t("connection.createDbDialog.fields.ifNotExists")}
              </Label>
            </div>
            <div>
              <Button
                type="button"
                variant="ghost"
                className="h-8 px-0"
                onClick={() => setShowCreateDbAdvanced((prev) => !prev)}
              >
                {showCreateDbAdvanced
                  ? t("connection.createDbDialog.hideAdvanced")
                  : t("connection.createDbDialog.showAdvanced")}
              </Button>
            </div>
            {showCreateDbAdvanced && (
              <div className="border p-3 rounded-md space-y-3 bg-muted/20">
                {isMySqlFamilyCreateDb && (
                  <>
                    <div className="grid gap-2">
                      <Label htmlFor="create-db-charset">
                        {t("connection.createDbDialog.fields.charset")}
                      </Label>
                      <Select
                        value={createDbForm.charset || createDbNoneOption}
                        disabled={loadingMysqlOptions}
                        onValueChange={(v) =>
                          setCreateDbForm((prev) => ({
                            ...prev,
                            charset: v === createDbNoneOption ? "" : v,
                            collation: "",
                          }))
                        }
                      >
                        <SelectTrigger id="create-db-charset">
                          <SelectValue
                            placeholder={
                              loadingMysqlOptions
                                ? t("common.loading")
                                : t(
                                    "connection.createDbDialog.placeholders.charset",
                                  )
                            }
                          />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value={createDbNoneOption}>
                            {t("connection.createDbDialog.defaultOption")}
                          </SelectItem>
                          {mysqlCharsets.map((opt) => (
                            <SelectItem key={opt} value={opt}>
                              {opt}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </div>
                    <div className="grid gap-2">
                      <Label htmlFor="create-db-collation">
                        {t("connection.createDbDialog.fields.collation")}
                      </Label>
                      <Select
                        value={createDbForm.collation || createDbNoneOption}
                        onValueChange={(v) =>
                          setCreateDbForm((prev) => ({
                            ...prev,
                            collation: v === createDbNoneOption ? "" : v,
                          }))
                        }
                      >
                        <SelectTrigger id="create-db-collation">
                          <SelectValue
                            placeholder={t(
                              "connection.createDbDialog.placeholders.collation",
                            )}
                          />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value={createDbNoneOption}>
                            {t("connection.createDbDialog.defaultOption")}
                          </SelectItem>
                          {mysqlCollations.map((opt) => (
                            <SelectItem key={opt} value={opt}>
                              {opt}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </div>
                  </>
                )}
                {isPostgresCreateDb && (
                  <>
                    <div className="grid gap-2">
                      <Label htmlFor="create-db-encoding">
                        {t("connection.createDbDialog.fields.encoding")}
                      </Label>
                      <Select
                        value={createDbForm.encoding || createDbNoneOption}
                        onValueChange={(v) =>
                          setCreateDbForm((prev) => ({
                            ...prev,
                            encoding: v === createDbNoneOption ? "" : v,
                          }))
                        }
                      >
                        <SelectTrigger id="create-db-encoding">
                          <SelectValue
                            placeholder={t(
                              "connection.createDbDialog.placeholders.encoding",
                            )}
                          />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value={createDbNoneOption}>
                            {t("connection.createDbDialog.defaultOption")}
                          </SelectItem>
                          {postgresEncodingOptions.map((opt) => (
                            <SelectItem key={opt} value={opt}>
                              {opt}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </div>
                    <div className="grid gap-2">
                      <Label htmlFor="create-db-lc-collate">
                        {t("connection.createDbDialog.fields.lcCollate")}
                      </Label>
                      <Select
                        value={createDbForm.lcCollate || createDbNoneOption}
                        onValueChange={(v) =>
                          setCreateDbForm((prev) => ({
                            ...prev,
                            lcCollate: v === createDbNoneOption ? "" : v,
                          }))
                        }
                      >
                        <SelectTrigger id="create-db-lc-collate">
                          <SelectValue
                            placeholder={t(
                              "connection.createDbDialog.placeholders.lcCollate",
                            )}
                          />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value={createDbNoneOption}>
                            {t("connection.createDbDialog.defaultOption")}
                          </SelectItem>
                          {postgresLocaleOptions.map((opt) => (
                            <SelectItem key={opt} value={opt}>
                              {opt}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </div>
                    <div className="grid gap-2">
                      <Label htmlFor="create-db-lc-ctype">
                        {t("connection.createDbDialog.fields.lcCtype")}
                      </Label>
                      <Select
                        value={createDbForm.lcCtype || createDbNoneOption}
                        onValueChange={(v) =>
                          setCreateDbForm((prev) => ({
                            ...prev,
                            lcCtype: v === createDbNoneOption ? "" : v,
                          }))
                        }
                      >
                        <SelectTrigger id="create-db-lc-ctype">
                          <SelectValue
                            placeholder={t(
                              "connection.createDbDialog.placeholders.lcCtype",
                            )}
                          />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value={createDbNoneOption}>
                            {t("connection.createDbDialog.defaultOption")}
                          </SelectItem>
                          {postgresLocaleOptions.map((opt) => (
                            <SelectItem key={opt} value={opt}>
                              {opt}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </div>
                  </>
                )}
                {isMssqlCreateDb && (
                  <div className="grid gap-2">
                    <Label htmlFor="create-db-collation">
                      {t("connection.createDbDialog.fields.collation")}
                    </Label>
                    <Select
                      value={createDbForm.collation || createDbNoneOption}
                      onValueChange={(v) =>
                        setCreateDbForm((prev) => ({
                          ...prev,
                          collation: v === createDbNoneOption ? "" : v,
                        }))
                      }
                    >
                      <SelectTrigger id="create-db-collation">
                        <SelectValue
                          placeholder={t(
                            "connection.createDbDialog.placeholders.collation",
                          )}
                        />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value={createDbNoneOption}>
                          {t("connection.createDbDialog.defaultOption")}
                        </SelectItem>
                        {mssqlCollationOptions.map((opt) => (
                          <SelectItem key={opt} value={opt}>
                            {opt}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                )}
              </div>
            )}
            {createDbValidationMsg && (
              <Alert variant="destructive">
                <AlertTitle>
                  {t("connection.dialog.validationFailed")}
                </AlertTitle>
                <AlertDescription>{createDbValidationMsg}</AlertDescription>
              </Alert>
            )}
            <div className="flex justify-end gap-2">
              <Button
                type="button"
                variant="outline"
                disabled={isCreatingDatabase}
                onClick={() => setIsCreateDbDialogOpen(false)}
              >
                {t("common.cancel")}
              </Button>
              <Button
                type="button"
                disabled={isCreatingDatabase}
                onClick={() => void handleCreateDatabase()}
              >
                {isCreatingDatabase ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    {t("connection.createDbDialog.creating")}
                  </>
                ) : (
                  t("connection.createDbDialog.confirm")
                )}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
      <AlertDialog
        open={!!deleteTargetConnectionId}
        onOpenChange={(open) => {
          if (!open) {
            setDeleteTargetConnectionId(null);
          }
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("connection.deleteDialog.title")}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t("connection.deleteDialog.description")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={isDeleting}>
              {t("common.cancel")}
            </AlertDialogCancel>
            <AlertDialogAction
              disabled={isDeleting || !deleteTargetConnectionId}
              onClick={async (e) => {
                e.preventDefault();
                if (!deleteTargetConnectionId) return;
                await handleDeleteConnection(deleteTargetConnectionId);
              }}
            >
              {isDeleting
                ? t("connection.deleteDialog.deleting")
                : t("common.delete")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
      <AlertDialog
        open={isImportConfirmOpen}
        onOpenChange={(open) => {
          setIsImportConfirmOpen(open);
          if (!open && !isImportingSql) {
            setPendingImport(null);
          }
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("connection.importDialog.title")}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t("connection.importDialog.description", {
                database: pendingImport?.databaseName || "",
              })}
            </AlertDialogDescription>
            <div className="text-xs text-muted-foreground font-mono break-all mt-2">
              {pendingImport?.filePath || ""}
            </div>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={isImportingSql}>
              {t("common.cancel")}
            </AlertDialogCancel>
            <AlertDialogAction
              disabled={isImportingSql || !pendingImport}
              onClick={async (e) => {
                e.preventDefault();
                await handleConfirmImport();
              }}
            >
              {isImportingSql ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  {t("connection.importDialog.importing")}
                </>
              ) : (
                t("connection.importDialog.confirm")
              )}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
      <Dialog
        open={isTableExportDialogOpen}
        onOpenChange={(open) => {
          setIsTableExportDialogOpen(open);
          if (!open && !isExportingTable) {
            setPendingTableExport(null);
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("connection.tableExportDialog.title")}</DialogTitle>
            <DialogDescription>
              {t("connection.tableExportDialog.description", {
                table: pendingTableExport?.table.name || "",
              })}
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-2">
            <RadioGroup
              value={tableExportFormat}
              onValueChange={(value: TableExportFormat) =>
                setTableExportFormat(value)
              }
            >
              <label className="flex items-start gap-3 rounded-md border p-3 cursor-pointer">
                <RadioGroupItem value="csv" id="table-export-csv" />
                <div className="grid gap-1">
                  <Label htmlFor="table-export-csv" className="cursor-pointer">
                    {t("connection.tableExportDialog.formatCsv")}
                  </Label>
                  <p className="text-sm text-muted-foreground">
                    {t("connection.tableExportDialog.formatCsvDesc")}
                  </p>
                </div>
              </label>
              <label className="flex items-start gap-3 rounded-md border p-3 cursor-pointer">
                <RadioGroupItem value="json" id="table-export-json" />
                <div className="grid gap-1">
                  <Label htmlFor="table-export-json" className="cursor-pointer">
                    {t("connection.tableExportDialog.formatJson")}
                  </Label>
                  <p className="text-sm text-muted-foreground">
                    {t("connection.tableExportDialog.formatJsonDesc")}
                  </p>
                </div>
              </label>
              <label className="flex items-start gap-3 rounded-md border p-3 cursor-pointer">
                <RadioGroupItem value="sql_ddl" id="table-export-sql-ddl" />
                <div className="grid gap-1">
                  <Label
                    htmlFor="table-export-sql-ddl"
                    className="cursor-pointer"
                  >
                    {t("connection.tableExportDialog.formatSqlDdl")}
                  </Label>
                  <p className="text-sm text-muted-foreground">
                    {t("connection.tableExportDialog.formatSqlDdlDesc")}
                  </p>
                </div>
              </label>
              <label className="flex items-start gap-3 rounded-md border p-3 cursor-pointer">
                <RadioGroupItem value="sql_dml" id="table-export-sql-dml" />
                <div className="grid gap-1">
                  <Label
                    htmlFor="table-export-sql-dml"
                    className="cursor-pointer"
                  >
                    {t("connection.tableExportDialog.formatSqlDml")}
                  </Label>
                  <p className="text-sm text-muted-foreground">
                    {t("connection.tableExportDialog.formatSqlDmlDesc")}
                  </p>
                </div>
              </label>
              <label className="flex items-start gap-3 rounded-md border p-3 cursor-pointer">
                <RadioGroupItem value="sql_full" id="table-export-sql-full" />
                <div className="grid gap-1">
                  <Label
                    htmlFor="table-export-sql-full"
                    className="cursor-pointer"
                  >
                    {t("connection.tableExportDialog.formatSqlFull")}
                  </Label>
                  <p className="text-sm text-muted-foreground">
                    {t("connection.tableExportDialog.formatSqlFullDesc")}
                  </p>
                </div>
              </label>
            </RadioGroup>
            <div className="flex justify-end gap-2">
              <Button
                type="button"
                variant="outline"
                disabled={isExportingTable}
                onClick={() => setIsTableExportDialogOpen(false)}
              >
                {t("common.cancel")}
              </Button>
              <Button
                type="button"
                disabled={isExportingTable || !pendingTableExport}
                onClick={() => void handleTableExportConfirm()}
              >
                {isExportingTable ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    {t("connection.exportDialog.exporting")}
                  </>
                ) : (
                  t("connection.tableExportDialog.exportButton")
                )}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
      <Dialog
        open={isDatabaseExportDialogOpen}
        onOpenChange={(open) => {
          setIsDatabaseExportDialogOpen(open);
          if (!open && !isExportingDatabaseSql) {
            setPendingDatabaseExport(null);
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("connection.exportDialog.title")}</DialogTitle>
            <DialogDescription>
              {t("connection.exportDialog.description", {
                database: pendingDatabaseExport?.databaseName || "",
              })}
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-2">
            <RadioGroup
              value={pendingDatabaseExport?.format || "sql_full"}
              onValueChange={(value: DatabaseExportFormat) =>
                setPendingDatabaseExport((prev) =>
                  prev ? { ...prev, format: value } : prev,
                )
              }
            >
              <label className="flex items-start gap-3 rounded-md border p-3 cursor-pointer">
                <RadioGroupItem value="sql_ddl" id="database-export-sql-ddl" />
                <div className="grid gap-1">
                  <Label
                    htmlFor="database-export-sql-ddl"
                    className="cursor-pointer"
                  >
                    {t("connection.exportDialog.options.sqlDdl.label")}
                  </Label>
                  <p className="text-sm text-muted-foreground">
                    {t("connection.exportDialog.options.sqlDdl.description")}
                  </p>
                </div>
              </label>
              <label className="flex items-start gap-3 rounded-md border p-3 cursor-pointer">
                <RadioGroupItem value="sql_dml" id="database-export-sql-dml" />
                <div className="grid gap-1">
                  <Label
                    htmlFor="database-export-sql-dml"
                    className="cursor-pointer"
                  >
                    {t("connection.exportDialog.options.sqlDml.label")}
                  </Label>
                  <p className="text-sm text-muted-foreground">
                    {t("connection.exportDialog.options.sqlDml.description")}
                  </p>
                </div>
              </label>
              <label className="flex items-start gap-3 rounded-md border p-3 cursor-pointer">
                <RadioGroupItem
                  value="sql_full"
                  id="database-export-sql-full"
                />
                <div className="grid gap-1">
                  <Label
                    htmlFor="database-export-sql-full"
                    className="cursor-pointer"
                  >
                    {t("connection.exportDialog.options.sqlFull.label")}
                  </Label>
                  <p className="text-sm text-muted-foreground">
                    {t("connection.exportDialog.options.sqlFull.description")}
                  </p>
                </div>
              </label>
            </RadioGroup>
            <div className="flex justify-end gap-2">
              <Button
                type="button"
                variant="outline"
                disabled={isExportingDatabaseSql}
                onClick={() => setIsDatabaseExportDialogOpen(false)}
              >
                {t("common.cancel")}
              </Button>
              <Button
                type="button"
                disabled={isExportingDatabaseSql || !pendingDatabaseExport}
                onClick={() => void handleConfirmDatabaseExport()}
              >
                {isExportingDatabaseSql ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    {t("connection.exportDialog.exporting")}
                  </>
                ) : (
                  t("connection.exportDialog.confirm")
                )}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}
