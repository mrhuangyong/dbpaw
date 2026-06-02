import { useState, useEffect, useCallback } from "react";
import { api, SyncConfig, SyncProviderType, SyncStatus } from "@/services/api";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Cloud, Upload, Download, RefreshCw, CloudOff } from "lucide-react";
import { useTranslation } from "react-i18next";

export function SyncSettings() {
  const { t } = useTranslation();
  const [providerType, setProviderType] = useState<SyncProviderType>("S3");
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [loading, setLoading] = useState(false);

  // S3 fields
  const [endpoint, setEndpoint] = useState("");
  const [region, setRegion] = useState("us-east-1");
  const [bucket, setBucket] = useState("");
  const [accessKeyId, setAccessKeyId] = useState("");
  const [secretAccessKey, setSecretAccessKey] = useState("");
  const [pathPrefix, setPathPrefix] = useState("dbpaw/");

  // WebDAV fields
  const [serverUrl, setServerUrl] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");

  // Sync password
  const [syncPassword, setSyncPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");

  const loadStatus = useCallback(async () => {
    try {
      const s = await api.sync.getStatus();
      setStatus(s);
    } catch (e) {
      console.error("Failed to load sync status:", e);
    }
  }, []);

  useEffect(() => {
    loadStatus();
  }, [loadStatus]);

  const buildConfig = (): SyncConfig => {
    if (providerType === "S3") {
      return {
        providerType: "S3",
        endpoint,
        region,
        bucket,
        accessKeyId,
        secretAccessKey,
        pathPrefix,
      };
    }
    return {
      providerType: "WebDAV",
      serverUrl,
      username,
      password,
    };
  };

  const handleTestConnection = async () => {
    setLoading(true);
    try {
      await api.sync.testConnection(buildConfig());
      toast.success(
        t("settings.sync.testSuccess", {
          defaultValue: "Connection successful",
        }),
      );
    } catch (e) {
      toast.error(
        t("settings.sync.testFailed", { defaultValue: "Connection failed" }),
        {
          description: e instanceof Error ? e.message : String(e),
        },
      );
    } finally {
      setLoading(false);
    }
  };

  const handleConfigure = async () => {
    if (!syncPassword || syncPassword.length < 6) {
      toast.error(
        t("settings.sync.passwordTooShort", {
          defaultValue: "Password must be at least 6 characters",
        }),
      );
      return;
    }
    if (syncPassword !== confirmPassword) {
      toast.error(
        t("settings.sync.passwordMismatch", {
          defaultValue: "Passwords do not match",
        }),
      );
      return;
    }
    setLoading(true);
    try {
      await api.sync.configure(buildConfig(), syncPassword);
      toast.success(
        t("settings.sync.configured", {
          defaultValue: "Sync configured and enabled",
        }),
      );
      loadStatus();
    } catch (e) {
      toast.error(
        t("settings.sync.configureFailed", {
          defaultValue: "Failed to configure sync",
        }),
        {
          description: e instanceof Error ? e.message : String(e),
        },
      );
    } finally {
      setLoading(false);
    }
  };

  const handleSyncNow = async () => {
    if (!syncPassword) {
      toast.error(
        t("settings.sync.enterPassword", {
          defaultValue: "Enter your sync password",
        }),
      );
      return;
    }
    setLoading(true);
    try {
      const result = await api.sync.syncNow(syncPassword);
      toast.success(
        t("settings.sync.synced", {
          defaultValue: `Sync: ${result.action}`,
        }),
      );
      loadStatus();
    } catch (e) {
      toast.error(
        t("settings.sync.syncFailed", { defaultValue: "Sync failed" }),
        {
          description: e instanceof Error ? e.message : String(e),
        },
      );
    } finally {
      setLoading(false);
    }
  };

  const handleForcePush = async () => {
    if (!syncPassword) {
      toast.error(
        t("settings.sync.enterPassword", {
          defaultValue: "Enter your sync password",
        }),
      );
      return;
    }
    setLoading(true);
    try {
      await api.sync.forcePush(syncPassword);
      toast.success(
        t("settings.sync.forcePushed", {
          defaultValue: "Force pushed to remote",
        }),
      );
      loadStatus();
    } catch (e) {
      toast.error(
        t("settings.sync.forcePushFailed", {
          defaultValue: "Force push failed",
        }),
        {
          description: e instanceof Error ? e.message : String(e),
        },
      );
    } finally {
      setLoading(false);
    }
  };

  const handleForcePull = async () => {
    if (!syncPassword) {
      toast.error(
        t("settings.sync.enterPassword", {
          defaultValue: "Enter your sync password",
        }),
      );
      return;
    }
    setLoading(true);
    try {
      await api.sync.forcePull(syncPassword);
      toast.success(
        t("settings.sync.forcePulled", {
          defaultValue: "Force pulled from remote",
        }),
      );
      loadStatus();
    } catch (e) {
      toast.error(
        t("settings.sync.forcePullFailed", {
          defaultValue: "Force pull failed",
        }),
        {
          description: e instanceof Error ? e.message : String(e),
        },
      );
    } finally {
      setLoading(false);
    }
  };

  const handleDisable = async () => {
    setLoading(true);
    try {
      await api.sync.disable();
      toast.success(
        t("settings.sync.disabled", { defaultValue: "Sync disabled" }),
      );
      loadStatus();
    } catch (e) {
      toast.error(
        t("settings.sync.disableFailed", {
          defaultValue: "Failed to disable sync",
        }),
        {
          description: e instanceof Error ? e.message : String(e),
        },
      );
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-4">
      <h3 className="text-lg font-medium flex items-center gap-2">
        <Cloud className="w-5 h-5" />{" "}
        {t("settings.sync.title", { defaultValue: "Config Sync" })}
      </h3>

      {/* Provider Configuration */}
      <div className="space-y-2 border rounded-md p-3">
        <Label className="text-base">
          {t("settings.sync.provider", {
            defaultValue: "Sync Provider",
          })}
        </Label>
        <Select
          value={providerType}
          onValueChange={(v) => setProviderType(v as SyncProviderType)}
        >
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="S3">S3 (AWS / MinIO / OSS)</SelectItem>
            <SelectItem value="WebDAV">WebDAV</SelectItem>
          </SelectContent>
        </Select>

        {providerType === "S3" ? (
          <div className="space-y-2">
            <Input
              placeholder="Endpoint (e.g., https://s3.amazonaws.com)"
              value={endpoint}
              onChange={(e) => setEndpoint(e.target.value)}
            />
            <Input
              placeholder="Region (e.g., us-east-1)"
              value={region}
              onChange={(e) => setRegion(e.target.value)}
            />
            <Input
              placeholder="Bucket"
              value={bucket}
              onChange={(e) => setBucket(e.target.value)}
            />
            <Input
              placeholder="Access Key ID"
              value={accessKeyId}
              onChange={(e) => setAccessKeyId(e.target.value)}
            />
            <Input
              placeholder="Secret Access Key"
              type="password"
              value={secretAccessKey}
              onChange={(e) => setSecretAccessKey(e.target.value)}
            />
            <Input
              placeholder="Path Prefix (default: dbpaw/)"
              value={pathPrefix}
              onChange={(e) => setPathPrefix(e.target.value)}
            />
          </div>
        ) : (
          <div className="space-y-2">
            <Input
              placeholder="Server URL (e.g., https://dav.example.com/dbpaw/)"
              value={serverUrl}
              onChange={(e) => setServerUrl(e.target.value)}
            />
            <Input
              placeholder="Username"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
            />
            <Input
              placeholder="Password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </div>
        )}

        <Separator className="my-2" />

        <Label className="text-base">
          {t("settings.sync.syncPassword", {
            defaultValue: "Sync Password",
          })}
        </Label>
        <Input
          placeholder="Sync password (min 6 chars)"
          type="password"
          value={syncPassword}
          onChange={(e) => setSyncPassword(e.target.value)}
        />
        <Input
          placeholder="Confirm password"
          type="password"
          value={confirmPassword}
          onChange={(e) => setConfirmPassword(e.target.value)}
        />

        <div className="flex gap-2 mt-2">
          <Button
            variant="outline"
            onClick={handleTestConnection}
            disabled={loading}
          >
            {t("settings.sync.testConnection", {
              defaultValue: "Test Connection",
            })}
          </Button>
          <Button onClick={handleConfigure} disabled={loading}>
            {t("settings.sync.saveAndEnable", {
              defaultValue: "Save & Enable",
            })}
          </Button>
          {status?.enabled && (
            <Button
              variant="outline"
              onClick={handleDisable}
              disabled={loading}
            >
              <CloudOff className="w-4 h-4 mr-1" />
              {t("settings.sync.disable", { defaultValue: "Disable" })}
            </Button>
          )}
        </div>
      </div>

      {/* Sync Status */}
      {status && (
        <div className="rounded-md border p-3 text-xs text-muted-foreground">
          <div className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground/90 mb-1">
            {t("settings.sync.status", { defaultValue: "Sync Status" })}
          </div>
          {status.deviceId && (
            <div>
              Device ID: {status.deviceId.slice(0, 8)}...
            </div>
          )}
          {status.lastSyncAt && (
            <div>
              {t("settings.sync.lastSync", { defaultValue: "Last sync" })}:{" "}
              {new Date(status.lastSyncAt).toLocaleString()}
              {status.lastSyncResult === "success"
                ? " ✓"
                : ` ✗ ${status.lastSyncResult}`}
            </div>
          )}
          {status.enabled && (
            <div className="mt-2 flex gap-2">
              <Button
                size="sm"
                variant="outline"
                onClick={handleSyncNow}
                disabled={loading}
              >
                <RefreshCw className="w-3.5 h-3.5 mr-1" />
                {t("settings.sync.syncNow", { defaultValue: "Sync Now" })}
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={handleForcePush}
                disabled={loading}
              >
                <Upload className="w-3.5 h-3.5 mr-1" />
                {t("settings.sync.forcePush", {
                  defaultValue: "Force Push",
                })}
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={handleForcePull}
                disabled={loading}
              >
                <Download className="w-3.5 h-3.5 mr-1" />
                {t("settings.sync.forcePull", {
                  defaultValue: "Force Pull",
                })}
              </Button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
