import { useState, useCallback, useRef, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Upload } from "lucide-react";

interface ConnectionContextMenuProps {
  onNewConnection: () => void;
  onImportConnection: () => void;
  children: (props: {
    onContextMenu: (e: React.MouseEvent) => void;
  }) => React.ReactNode;
}

export function ConnectionContextMenu({
  onNewConnection,
  onImportConnection,
  children,
}: ConnectionContextMenuProps) {
  const { t } = useTranslation();
  const [menu, setMenu] = useState<{ visible: boolean; x: number; y: number }>({
    visible: false,
    x: 0,
    y: 0,
  });
  const menuRef = useRef<HTMLDivElement>(null);

  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setMenu({ visible: true, x: e.clientX, y: e.clientY });
  }, []);

  const handleClose = useCallback(() => {
    setMenu((prev) => ({ ...prev, visible: false }));
  }, []);

  useEffect(() => {
    if (!menu.visible) return;
    const handleClick = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        handleClose();
      }
    };
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape") handleClose();
    };
    document.addEventListener("mousedown", handleClick);
    document.addEventListener("keydown", handleEscape);
    return () => {
      document.removeEventListener("mousedown", handleClick);
      document.removeEventListener("keydown", handleEscape);
    };
  }, [menu.visible, handleClose]);

  return (
    <>
      {children({ onContextMenu: handleContextMenu })}
      {menu.visible && (
        <div
          ref={menuRef}
          className="fixed z-50 min-w-[140px] bg-popover border border-border rounded-md shadow-lg py-1"
          style={{ left: menu.x, top: menu.y }}
        >
          <button
            className="w-full px-3 py-1.5 text-left text-sm flex items-center gap-2 hover:bg-accent hover:text-accent-foreground"
            onClick={() => {
              handleClose();
              onNewConnection();
            }}
          >
            <Plus className="w-4 h-4" />
            {t("connection.menu.newConnection")}
          </button>
          <button
            className="w-full px-3 py-1.5 text-left text-sm flex items-center gap-2 hover:bg-accent hover:text-accent-foreground"
            onClick={() => {
              handleClose();
              onImportConnection();
            }}
          >
            <Upload className="w-4 h-4" />
            {t("connection.menu.importConnections")}
          </button>
        </div>
      )}
    </>
  );
}
