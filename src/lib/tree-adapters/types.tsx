import type { ReactNode } from "react";
import type { Driver, DriverKind } from "../driver-registry";

// ============ Context Types ============

export interface TreeContext {
  connectionId: string;
  connectionName: string;
  connectionType: Driver;
  driverKind: DriverKind;
}

export interface DatabaseContext extends TreeContext {
  databaseName: string;
  databaseMeta?: Record<string, unknown>;
}

export interface LeafContext extends DatabaseContext {
  leafName: string;
  leafSchema?: string;
  leafMeta?: Record<string, unknown>;
}

// ============ Menu Types ============

export interface TreeMenuItem {
  key: string;
  label: string;
  icon?: ReactNode;
  destructive?: boolean;
  onClick: () => void;
}

// ============ Callbacks ============

export interface TreeCallbacks {
  // SQL 类通用
  onTableSelect?: (ctx: LeafContext) => void;
  onCreateQuery?: (ctx: DatabaseContext) => void;
  onExportTable?: (ctx: LeafContext) => void;
  onAlterTable?: (ctx: LeafContext) => void;

  // Redis 专用
  onCreateKey?: (ctx: DatabaseContext) => void;
  onOpenBrowser?: (ctx: DatabaseContext) => void;
  onOpenConsole?: (ctx: DatabaseContext) => void;
  onOpenServerInfo?: (ctx: DatabaseContext) => void;
  onKeySelect?: (ctx: LeafContext) => void;

  // Elasticsearch 专用
  onCreateIndex?: (ctx: DatabaseContext) => void;
  onOpenIndex?: (ctx: LeafContext) => void;
  onIndexAction?: (
    ctx: LeafContext,
    action: "refresh" | "open" | "close" | "delete",
  ) => void;

  // MongoDB 专用
  onCreateCollection?: (ctx: DatabaseContext) => void;
  onCollectionSelect?: (ctx: LeafContext) => void;
  onCollectionAction?: (ctx: LeafContext, action: "drop" | "rename") => void;
}

// ============ Tree Config ============

export interface TreeConfig {
  // 节点能力
  supportsSavedQueries: boolean;
  databaseExpandable: boolean;
  supportsSchemaNode: boolean;

  // 叶子节点
  leafNodeType: "table" | "key" | "index" | "collection";
  leafNodeIcon: () => ReactNode;

  // 数据库节点
  databaseNodeIcon: () => ReactNode;

  // 虚拟数据库（如 ES 的 "Indices"）
  virtualDatabases?: string[];

  // 数据库标签自定义
  getDatabaseLabel?: (
    name: string,
    meta?: Record<string, unknown>,
  ) => string | null;

  // 数据库节点 Actions（如 "+" 按钮）
  getDatabaseActions?: (ctx: DatabaseContext) => ReactNode | undefined;

  // 数据库节点双击行为
  onDatabaseDoubleClick?: (ctx: DatabaseContext) => void;

  // 数据库节点 Footer
  renderDatabaseFooter?: (ctx: DatabaseContext & { level: number }) => ReactNode;

  // 数据库右键菜单
  getDatabaseContextMenuItems?: (ctx: DatabaseContext) => TreeMenuItem[];

  // 叶子节点右键菜单
  getLeafContextMenuItems?: (ctx: LeafContext) => TreeMenuItem[];

  // 叶子节点激活行为
  onLeafActivate?: (ctx: LeafContext) => void;
}
