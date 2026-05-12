import { Database, FileBox, Plus, Trash2, Edit3 } from "lucide-react";
import type { TreeConfig, TreeMenuItem, DatabaseContext, LeafContext } from "./types";

export function createMongodbTreeConfig(
  callbacks: {
    onCreateCollection?: (ctx: DatabaseContext) => void;
    onCollectionSelect?: (ctx: LeafContext) => void;
    onCollectionAction?: (ctx: LeafContext, action: "drop" | "rename") => void;
  },
): TreeConfig {
  return {
    supportsSavedQueries: false,
    databaseExpandable: true,
    supportsSchemaNode: false,
    leafNodeType: "collection",
    leafNodeIcon: () => <FileBox className="w-4 h-4" />,
    databaseNodeIcon: () => <Database className="w-4 h-4" />,

    getDatabaseActions: (ctx) => {
      if (!callbacks.onCreateCollection) return undefined;
      return (
        <div onClick={(e) => e.stopPropagation()}>
          <button
            className="h-6 w-6 p-0 inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors hover:bg-accent hover:text-accent-foreground"
            title="New collection"
            onClick={() => callbacks.onCreateCollection!(ctx)}
          >
            <Plus className="w-3 h-3" />
          </button>
        </div>
      );
    },

    onLeafActivate: callbacks.onCollectionSelect
      ? (ctx) => callbacks.onCollectionSelect!(ctx)
      : undefined,

    getDatabaseContextMenuItems: (ctx) => {
      const items: TreeMenuItem[] = [];

      if (callbacks.onCreateCollection) {
        items.push({
          key: "new-collection",
          label: "New collection",
          icon: <Plus className="h-4 w-4" />,
          onClick: () => callbacks.onCreateCollection!(ctx),
        });
      }

      return items;
    },

    getLeafContextMenuItems: (ctx) => {
      const items: TreeMenuItem[] = [];

      if (callbacks.onCollectionAction) {
        items.push({
          key: "rename",
          label: "Rename collection",
          icon: <Edit3 className="mr-2 h-4 w-4" />,
          onClick: () => callbacks.onCollectionAction!(ctx, "rename"),
        });

        items.push({
          key: "drop",
          label: "Drop collection",
          icon: <Trash2 className="mr-2 h-4 w-4" />,
          destructive: true,
          onClick: () => callbacks.onCollectionAction!(ctx, "drop"),
        });
      }

      return items;
    },
  };
}
