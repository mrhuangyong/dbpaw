import { Table, Database, FileCode, Download, RefreshCw } from "lucide-react";
import type {
  TreeConfig,
  TreeMenuItem,
  DatabaseContext,
  LeafContext,
} from "./types";

export function createSqlTreeConfig(
  overrides?: Partial<TreeConfig>,
): TreeConfig {
  return {
    supportsSavedQueries: true,
    databaseExpandable: true,
    supportsSchemaNode: false,
    leafNodeType: "table",
    leafNodeIcon: () => <Table className="w-4 h-4" />,
    databaseNodeIcon: () => <Database className="w-4 h-4" />,
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
