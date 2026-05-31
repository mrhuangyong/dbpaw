import { useState } from "react";
import { useTranslation } from "react-i18next";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Loader2, FileUp, FileJson } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { api, isTauri } from "@/services/api";
import { toast } from "sonner";

interface ImportDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onImported: () => void;
}

type Source = "navicat" | "dbeaver";

export function ImportDialog({ open, onOpenChange, onImported }: ImportDialogProps) {
  const { t } = useTranslation();
  const [loading, setLoading] = useState<Source | null>(null);

  const handleImport = async (source: Source) => {
    if (!isTauri()) {
      toast.info(t("connection.toast.importDesktopOnly"));
      return;
    }

    const filters =
      source === "navicat"
        ? [{ name: "Navicat NCX", extensions: ["ncx"] }]
        : [{ name: "DBeaver JSON", extensions: ["json"] }];

    const selected = await openDialog({ multiple: false, filters });
    if (!selected) return;

    const filePath = Array.isArray(selected) ? selected[0] : selected;
    if (!filePath) return;

    setLoading(source);
    try {
      const result = await api.connections.importFromFile(filePath);
      if (result.imported.length > 0) {
        toast.success(
          t("connection.toast.importConnectionsSuccess", {
            count: result.imported.length,
          }),
        );
      }
      if (result.skipped > 0) {
        toast.info(
          t("connection.toast.importConnectionsSkipped", {
            count: result.skipped,
          }),
        );
      }
      if (result.imported.length === 0 && result.skipped === 0) {
        toast.info(t("connection.toast.importConnectionsSuccess", { count: 0 }));
      }
      onImported();
      onOpenChange(false);
    } catch (e) {
      toast.error(t("connection.toast.importConnectionsFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setLoading(null);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("connection.connectionImportDialog.title")}</DialogTitle>
          <DialogDescription>
            {t("connection.connectionImportDialog.description")}
          </DialogDescription>
        </DialogHeader>

        <div className="flex gap-4 justify-center py-4">
          <button
            onClick={() => handleImport("navicat")}
            disabled={loading !== null}
            className="flex flex-col items-center gap-2 p-6 rounded-xl border-2 border-blue-400/50 bg-blue-400/5 hover:bg-blue-400/10 transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed w-40"
          >
            {loading === "navicat" ? (
              <Loader2 className="w-8 h-8 text-blue-400 animate-spin" />
            ) : (
              <FileUp className="w-8 h-8 text-blue-400" />
            )}
            <span className="font-medium text-sm">
              {t("connection.connectionImportDialog.navicat")}
            </span>
            <span className="text-xs text-muted-foreground">
              {t("connection.connectionImportDialog.navicatDescription")}
            </span>
            <span className="text-[10px] text-muted-foreground/60">
              {t("connection.connectionImportDialog.navicatFormat")}
            </span>
          </button>

          <button
            onClick={() => handleImport("dbeaver")}
            disabled={loading !== null}
            className="flex flex-col items-center gap-2 p-6 rounded-xl border-2 border-green-400/50 bg-green-400/5 hover:bg-green-400/10 transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed w-40"
          >
            {loading === "dbeaver" ? (
              <Loader2 className="w-8 h-8 text-green-400 animate-spin" />
            ) : (
              <FileJson className="w-8 h-8 text-green-400" />
            )}
            <span className="font-medium text-sm">
              {t("connection.connectionImportDialog.dbeaver")}
            </span>
            <span className="text-xs text-muted-foreground">
              {t("connection.connectionImportDialog.dbeaverDescription")}
            </span>
            <span className="text-[10px] text-muted-foreground/60">
              {t("connection.connectionImportDialog.dbeaverFormat")}
            </span>
          </button>
        </div>

        <div className="flex justify-center">
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t("connection.connectionImportDialog.cancel")}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
