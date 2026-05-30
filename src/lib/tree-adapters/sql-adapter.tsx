import { Table, Database, FileCode, Download, RefreshCw, Eye, Cog, Clock, Hash, Type } from "lucide-react";
import type {
  TreeConfig,
  TreeCallbacks,
  TreeMenuItem,
  DatabaseContext,
  LeafContext,
  DatabaseGroupConfig,
} from "./types";

const mysqlGroups: DatabaseGroupConfig[] = [
  { id: "tables",     label: "connection.tree.tables",     icon: <Table className="w-4 h-4" />,  leafIcon: <Table className="w-4 h-4" />,  source: "tables" },
  { id: "views",      label: "connection.tree.views",      icon: <Eye className="w-4 h-4" />,    leafIcon: <Eye className="w-4 h-4" />,    source: "tables",  sourceFilter: "view" },
  { id: "functions",  label: "connection.tree.functions",  icon: <Cog className="w-4 h-4" />,    leafIcon: <Cog className="w-4 h-4" />,    source: "routines", sourceFilter: "function" },
  { id: "procedures", label: "connection.tree.procedures", icon: <Cog className="w-4 h-4" />,    leafIcon: <Cog className="w-4 h-4" />,    source: "routines", sourceFilter: "procedure" },
  { id: "events",     label: "connection.tree.events",     icon: <Clock className="w-4 h-4" />,  leafIcon: <Clock className="w-4 h-4" />,  source: "events" },
];

const postgresGroups: DatabaseGroupConfig[] = [
  { id: "tables",     label: "connection.tree.tables",     icon: <Table className="w-4 h-4" />,  leafIcon: <Table className="w-4 h-4" />,  source: "tables" },
  { id: "views",      label: "connection.tree.views",      icon: <Eye className="w-4 h-4" />,    leafIcon: <Eye className="w-4 h-4" />,    source: "tables",  sourceFilter: "VIEW" },
  { id: "functions",  label: "connection.tree.functions",  icon: <Cog className="w-4 h-4" />,    leafIcon: <Cog className="w-4 h-4" />,    source: "routines", sourceFilter: "function" },
  { id: "procedures", label: "connection.tree.procedures", icon: <Cog className="w-4 h-4" />,    leafIcon: <Cog className="w-4 h-4" />,    source: "routines", sourceFilter: "procedure" },
  { id: "sequences",  label: "connection.tree.sequences",  icon: <Hash className="w-4 h-4" />,   leafIcon: <Hash className="w-4 h-4" />,   source: "sequences" },
  { id: "types",      label: "connection.tree.types",      icon: <Type className="w-4 h-4" />,   leafIcon: <Type className="w-4 h-4" />,   source: "types" },
];

const sqliteGroups: DatabaseGroupConfig[] = [
  { id: "tables",     label: "connection.tree.tables",     icon: <Table className="w-4 h-4" />,  leafIcon: <Table className="w-4 h-4" />,  source: "tables" },
  { id: "views",      label: "connection.tree.views",      icon: <Eye className="w-4 h-4" />,    leafIcon: <Eye className="w-4 h-4" />,    source: "tables",  sourceFilter: "view" },
];

const clickhouseGroups: DatabaseGroupConfig[] = [
  { id: "tables",            label: "connection.tree.tables",            icon: <Table className="w-4 h-4" />,  leafIcon: <Table className="w-4 h-4" />,  source: "tables" },
  { id: "views",             label: "connection.tree.views",             icon: <Eye className="w-4 h-4" />,    leafIcon: <Eye className="w-4 h-4" />,    source: "tables", sourceFilter: "View" },
  { id: "materializedViews", label: "connection.tree.materializedViews", icon: <Eye className="w-4 h-4" />,    leafIcon: <Eye className="w-4 h-4" />,    source: "tables", sourceFilter: "MaterializedView" },
];

const defaultSqlGroups: DatabaseGroupConfig[] = [
  { id: "tables",     label: "connection.tree.tables",     icon: <Table className="w-4 h-4" />,  leafIcon: <Table className="w-4 h-4" />,  source: "tables" },
  { id: "views",      label: "connection.tree.views",      icon: <Eye className="w-4 h-4" />,    leafIcon: <Eye className="w-4 h-4" />,    source: "tables",  sourceFilter: "view" },
  { id: "functions",  label: "connection.tree.functions",  icon: <Cog className="w-4 h-4" />,    leafIcon: <Cog className="w-4 h-4" />,    source: "routines", sourceFilter: "function" },
  { id: "procedures", label: "connection.tree.procedures", icon: <Cog className="w-4 h-4" />,    leafIcon: <Cog className="w-4 h-4" />,    source: "routines", sourceFilter: "procedure" },
];

export function createSqlTreeConfig(
  callbacks: TreeCallbacks = {},
  overrides?: Partial<TreeConfig>,
  driverId?: string,
): TreeConfig {
  const groups = driverId === "mysql" || driverId === "mariadb" || driverId === "tidb" || driverId === "starrocks" || driverId === "doris"
    ? mysqlGroups
    : driverId === "postgres"
      ? postgresGroups
      : driverId === "sqlite" || driverId === "duckdb"
        ? sqliteGroups
        : driverId === "clickhouse"
          ? clickhouseGroups
          : defaultSqlGroups;
  return {
    supportsSavedQueries: true,
    databaseExpandable: true,
    supportsSchemaNode: false,
    leafNodeType: "table",
    leafNodeIcon: () => <Table className="w-4 h-4" />,
    databaseNodeIcon: () => <Database className="w-4 h-4" />,
    databaseGroups: groups,
    getDatabaseContextMenuItems: (ctx) =>
      getSqlDatabaseContextMenuItems(ctx, callbacks),
    getLeafContextMenuItems: (ctx) =>
      getSqlLeafContextMenuItems(ctx, callbacks),
    ...overrides,
  };
}

export function getSqlLeafContextMenuItems(
  ctx: LeafContext,
  callbacks: {
    onCreateQuery?: (ctx: DatabaseContext) => void;
    onRefresh?: (ctx: DatabaseContext) => void;
    onOpenERDiagram?: (ctx: DatabaseContext) => void;
    onExportTable?: (ctx: LeafContext) => void;
    onAlterTable?: (ctx: LeafContext) => void;
  },
): TreeMenuItem[] {
  const items: TreeMenuItem[] = [];

  if (callbacks.onCreateQuery) {
    items.push({
      key: "new-query",
      label: "New Query",
      icon: <FileCode className="mr-2 h-4 w-4" />,
      onClick: () =>
        callbacks.onCreateQuery!({
          connectionId: ctx.connectionId,
          connectionName: ctx.connectionName,
          connectionType: ctx.connectionType,
          driverKind: ctx.driverKind,
          databaseName: ctx.databaseName,
        }),
    });
  }

  if (callbacks.onRefresh) {
    items.push({
      key: "refresh",
      label: "Refresh",
      icon: <RefreshCw className="mr-2 h-4 w-4" />,
      onClick: () =>
        callbacks.onRefresh!({
          connectionId: ctx.connectionId,
          connectionName: ctx.connectionName,
          connectionType: ctx.connectionType,
          driverKind: ctx.driverKind,
          databaseName: ctx.databaseName,
        }),
    });
  }

  if (callbacks.onOpenERDiagram) {
    items.push({
      key: "er-diagram",
      label: "ER Diagram",
      icon: <Table className="mr-2 h-4 w-4" />,
      onClick: () =>
        callbacks.onOpenERDiagram!({
          connectionId: ctx.connectionId,
          connectionName: ctx.connectionName,
          connectionType: ctx.connectionType,
          driverKind: ctx.driverKind,
          databaseName: ctx.databaseName,
        }),
    });
  }

  if (callbacks.onExportTable) {
    items.push({
      key: "export-table",
      label: "Export Table",
      icon: <Download className="mr-2 h-4 w-4" />,
      onClick: () => callbacks.onExportTable!(ctx),
    });
  }

  if (callbacks.onAlterTable) {
    items.push({
      key: "alter-table",
      label: "Alter Table",
      icon: <Table className="mr-2 h-4 w-4" />,
      onClick: () => callbacks.onAlterTable!(ctx),
    });
  }

  return items;
}

export function getSqlDatabaseContextMenuItems(
  ctx: DatabaseContext,
  callbacks: {
    onCreateQuery?: (ctx: DatabaseContext) => void;
    onOpenERDiagram?: (ctx: DatabaseContext) => void;
  },
): TreeMenuItem[] {
  const items: TreeMenuItem[] = [];

  if (callbacks.onCreateQuery) {
    items.push({
      key: "new-query",
      label: "New Query",
      icon: <FileCode className="mr-2 h-4 w-4" />,
      onClick: () => callbacks.onCreateQuery!(ctx),
    });
  }

  if (callbacks.onOpenERDiagram) {
    items.push({
      key: "er-diagram",
      label: "ER Diagram",
      icon: <Table className="mr-2 h-4 w-4" />,
      onClick: () => callbacks.onOpenERDiagram!(ctx),
    });
  }

  return items;
}
