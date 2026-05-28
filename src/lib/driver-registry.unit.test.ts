import { describe, expect, test } from "bun:test";
import {
  DRIVER_REGISTRY,
  getDriverConfig,
  getDefaultPort,
  isFileBasedDriver,
  isMysqlFamilyDriver,
  supportsSSLCA,
  supportsCreateDatabase,
  supportsRoutines,
  supportsSchemaBrowsing,
  getConnectionIcon,
  type Driver,
} from "./driver-registry";

// ─── Registry completeness ────────────────────────────────────────────────────

describe("DRIVER_REGISTRY", () => {
  test("contains all 16 supported drivers", () => {
    const ids = DRIVER_REGISTRY.map((d) => d.id);
    expect(ids).toContain("postgres");
    expect(ids).toContain("mysql");
    expect(ids).toContain("mariadb");
    expect(ids).toContain("tidb");
    expect(ids).toContain("starrocks");
    expect(ids).toContain("doris");
    expect(ids).toContain("sqlite");
    expect(ids).toContain("duckdb");
    expect(ids).toContain("clickhouse");
    expect(ids).toContain("mssql");
    expect(ids).toContain("oracle");
    expect(ids).toContain("db2");
    expect(ids).toContain("redis");
    expect(ids).toContain("elasticsearch");
    expect(ids).toContain("mongodb");
    expect(ids).toContain("cassandra");
    expect(DRIVER_REGISTRY).toHaveLength(16);
  });

  test("has no duplicate IDs", () => {
    const ids = DRIVER_REGISTRY.map((d) => d.id);
    const unique = new Set(ids);
    expect(unique.size).toBe(ids.length);
  });

  test("every entry has a non-empty label", () => {
    for (const d of DRIVER_REGISTRY) {
      expect(d.label.length).toBeGreaterThan(0);
    }
  });

  test("every entry has an icon function", () => {
    for (const d of DRIVER_REGISTRY) {
      expect(typeof d.icon).toBe("function");
    }
  });
});

// ─── Registry invariants ──────────────────────────────────────────────────────

describe("registry invariants", () => {
  test("file-based drivers always have null defaultPort", () => {
    for (const d of DRIVER_REGISTRY) {
      if (d.isFileBased) {
        expect(d.defaultPort).toBeNull();
      }
    }
  });

  test("network drivers always have a positive integer defaultPort", () => {
    for (const d of DRIVER_REGISTRY) {
      if (!d.isFileBased) {
        expect(typeof d.defaultPort).toBe("number");
        expect(d.defaultPort).toBeGreaterThan(0);
      }
    }
  });

  test("mysql-family drivers are not file-based", () => {
    for (const d of DRIVER_REGISTRY) {
      if (d.isMysqlFamily) {
        expect(d.isFileBased).toBe(false);
      }
    }
  });

  test("file-based drivers do not support SSL CA", () => {
    for (const d of DRIVER_REGISTRY) {
      if (d.isFileBased) {
        expect(d.supportsSSLCA).toBe(false);
      }
    }
  });

  test("file-based drivers do not support create database", () => {
    for (const d of DRIVER_REGISTRY) {
      if (d.isFileBased) {
        expect(d.supportsCreateDatabase).toBe(false);
      }
    }
  });
});

// ─── getDriverConfig ──────────────────────────────────────────────────────────

describe("getDriverConfig", () => {
  test("returns the correct config for each driver", () => {
    expect(getDriverConfig("postgres").label).toBe("PostgreSQL");
    expect(getDriverConfig("mysql").label).toBe("MySQL");
    expect(getDriverConfig("starrocks").label).toBe("StarRocks");
    expect(getDriverConfig("doris").label).toBe("Apache Doris");
    expect(getDriverConfig("mssql").label).toBe("SQL Server");
    expect(getDriverConfig("clickhouse").label).toBe("ClickHouse");
    expect(getDriverConfig("duckdb").label).toBe("DuckDB");
    expect(getDriverConfig("elasticsearch").label).toBe("Elasticsearch");
    expect(getDriverConfig("db2").label).toBe("IBM Db2");
    expect(getDriverConfig("cassandra").label).toBe("Cassandra");
  });
});

// ─── getDefaultPort ───────────────────────────────────────────────────────────

describe("getDefaultPort", () => {
  test("returns correct ports for network drivers", () => {
    expect(getDefaultPort("postgres")).toBe(5432);
    expect(getDefaultPort("mysql")).toBe(3306);
    expect(getDefaultPort("mariadb")).toBe(3306);
    expect(getDefaultPort("tidb")).toBe(4000);
    expect(getDefaultPort("starrocks")).toBe(9030);
    expect(getDefaultPort("doris")).toBe(9030);
    expect(getDefaultPort("clickhouse")).toBe(8123);
    expect(getDefaultPort("mssql")).toBe(1433);
    expect(getDefaultPort("oracle")).toBe(1521);
    expect(getDefaultPort("db2")).toBe(50000);
    expect(getDefaultPort("redis")).toBe(6379);
    expect(getDefaultPort("elasticsearch")).toBe(9200);
    expect(getDefaultPort("mongodb")).toBe(27017);
    expect(getDefaultPort("cassandra")).toBe(9042);
  });

  test("returns null for file-based drivers", () => {
    expect(getDefaultPort("sqlite")).toBeNull();
    expect(getDefaultPort("duckdb")).toBeNull();
  });
});

// ─── isFileBasedDriver ────────────────────────────────────────────────────────

describe("isFileBasedDriver", () => {
  test("returns true for file-based drivers", () => {
    expect(isFileBasedDriver("sqlite")).toBe(true);
    expect(isFileBasedDriver("duckdb")).toBe(true);
  });

  test("returns false for network drivers", () => {
    const networkDrivers: Driver[] = [
      "postgres",
      "mysql",
      "mariadb",
      "tidb",
      "starrocks",
      "doris",
      "clickhouse",
      "mssql",
      "oracle",
      "db2",
      "redis",
      "elasticsearch",
      "mongodb",
      "cassandra",
    ];
    for (const d of networkDrivers) {
      expect(isFileBasedDriver(d)).toBe(false);
    }
  });
});

// ─── isMysqlFamilyDriver ──────────────────────────────────────────────────────

describe("isMysqlFamilyDriver", () => {
  test("returns true for MySQL-family drivers", () => {
    expect(isMysqlFamilyDriver("mysql")).toBe(true);
    expect(isMysqlFamilyDriver("mariadb")).toBe(true);
    expect(isMysqlFamilyDriver("tidb")).toBe(true);
    expect(isMysqlFamilyDriver("starrocks")).toBe(true);
    expect(isMysqlFamilyDriver("doris")).toBe(true);
  });

  test("returns false for non-MySQL drivers", () => {
    const others: Driver[] = [
      "postgres",
      "sqlite",
      "duckdb",
      "clickhouse",
      "mssql",
      "elasticsearch",
    ];
    for (const d of others) {
      expect(isMysqlFamilyDriver(d)).toBe(false);
    }
  });
});

// ─── supportsSSLCA ────────────────────────────────────────────────────────────

describe("supportsSSLCA", () => {
  test("returns true for drivers with SSL CA support", () => {
    expect(supportsSSLCA("postgres")).toBe(true);
    expect(supportsSSLCA("mysql")).toBe(true);
    expect(supportsSSLCA("mariadb")).toBe(true);
    expect(supportsSSLCA("tidb")).toBe(true);
    expect(supportsSSLCA("starrocks")).toBe(true);
    expect(supportsSSLCA("doris")).toBe(true);
    expect(supportsSSLCA("elasticsearch")).toBe(true);
  });

  test("returns false for drivers without SSL CA support", () => {
    expect(supportsSSLCA("sqlite")).toBe(false);
    expect(supportsSSLCA("duckdb")).toBe(false);
    expect(supportsSSLCA("clickhouse")).toBe(false);
    expect(supportsSSLCA("mssql")).toBe(false);
  });
});

// ─── supportsCreateDatabase ───────────────────────────────────────────────────

describe("supportsCreateDatabase", () => {
  test("returns true for drivers that can create databases", () => {
    expect(supportsCreateDatabase("postgres")).toBe(true);
    expect(supportsCreateDatabase("mysql")).toBe(true);
    expect(supportsCreateDatabase("mariadb")).toBe(true);
    expect(supportsCreateDatabase("tidb")).toBe(true);
    expect(supportsCreateDatabase("starrocks")).toBe(true);
    expect(supportsCreateDatabase("doris")).toBe(true);
    expect(supportsCreateDatabase("clickhouse")).toBe(true);
    expect(supportsCreateDatabase("mssql")).toBe(true);
    expect(supportsCreateDatabase("cassandra")).toBe(true);
  });

  test("returns false for drivers without create database support", () => {
    expect(supportsCreateDatabase("sqlite")).toBe(false);
    expect(supportsCreateDatabase("duckdb")).toBe(false);
    expect(supportsCreateDatabase("redis")).toBe(false);
    expect(supportsCreateDatabase("elasticsearch")).toBe(false);
    expect(supportsCreateDatabase("mongodb")).toBe(false);
    expect(supportsCreateDatabase("oracle")).toBe(false);
    expect(supportsCreateDatabase("db2")).toBe(false);
  });
});

// ─── supportsSchemaBrowsing ───────────────────────────────────────────────────

describe("supportsSchemaBrowsing", () => {
  test("returns true for drivers with schema node support", () => {
    expect(supportsSchemaBrowsing("postgres")).toBe(true);
    expect(supportsSchemaBrowsing("mssql")).toBe(true);
    expect(supportsSchemaBrowsing("oracle")).toBe(true);
    expect(supportsSchemaBrowsing("db2")).toBe(true);
  });

  test("returns false for drivers without schema node support", () => {
    const noSchema: Driver[] = [
      "mysql",
      "mariadb",
      "tidb",
      "starrocks",
      "doris",
      "sqlite",
      "duckdb",
      "clickhouse",
      "redis",
      "elasticsearch",
      "mongodb",
      "cassandra",
    ];
    for (const d of noSchema) {
      expect(supportsSchemaBrowsing(d)).toBe(false);
    }
  });
});

// ─── supportsRoutines ────────────────────────────────────────────────────────

describe("supportsRoutines", () => {
  test("returns true for drivers with routine support", () => {
    expect(supportsRoutines("postgres")).toBe(true);
    expect(supportsRoutines("mysql")).toBe(true);
    expect(supportsRoutines("mssql")).toBe(true);
    expect(supportsRoutines("db2")).toBe(true);
  });

  test("returns false for drivers without routine support", () => {
    const noRoutines: Driver[] = [
      "mariadb",
      "tidb",
      "starrocks",
      "doris",
      "sqlite",
      "duckdb",
      "clickhouse",
      "oracle",
      "redis",
      "elasticsearch",
      "mongodb",
      "cassandra",
    ];
    for (const d of noRoutines) {
      expect(supportsRoutines(d)).toBe(false);
    }
  });
});

// ─── importCapability ─────────────────────────────────────────────────────────

describe("importCapability", () => {
  test("clickhouse is read_only_not_supported", () => {
    expect(getDriverConfig("clickhouse").importCapability).toBe(
      "read_only_not_supported",
    );
  });

  test("starrocks import is unsupported", () => {
    expect(getDriverConfig("starrocks").importCapability).toBe("unsupported");
  });

  test("doris import is unsupported", () => {
    expect(getDriverConfig("doris").importCapability).toBe("unsupported");
  });

  test("all other drivers are supported", () => {
    const supported: Driver[] = [
      "postgres",
      "mysql",
      "mariadb",
      "tidb",
      "sqlite",
      "duckdb",
      "mssql",
      "oracle",
      "db2",
    ];
    for (const d of supported) {
      expect(getDriverConfig(d).importCapability).toBe("supported");
    }
  });

  test("unsupported drivers have unsupported import capability", () => {
    const unsupported: Driver[] = [
      "redis",
      "elasticsearch",
      "mongodb",
      "cassandra",
    ];
    for (const d of unsupported) {
      expect(getDriverConfig(d).importCapability).toBe("unsupported");
    }
  });
});

// ─── getConnectionIcon ────────────────────────────────────────────────────────

describe("getConnectionIcon", () => {
  test("returns a non-null value for all registered drivers", () => {
    for (const d of DRIVER_REGISTRY) {
      const icon = getConnectionIcon(d.id);
      expect(icon).not.toBeNull();
      expect(icon).not.toBeUndefined();
    }
  });

  test("handles common aliases", () => {
    expect(getConnectionIcon("postgresql")).not.toBeNull();
    expect(getConnectionIcon("pgsql")).not.toBeNull();
    expect(getConnectionIcon("sqlite3")).not.toBeNull();
  });

  test("returns fallback icon for unknown drivers", () => {
    expect(getConnectionIcon("oracle")).not.toBeNull();
    expect(getConnectionIcon("mongodb")).not.toBeNull();
  });

  test("handles undefined input", () => {
    expect(getConnectionIcon(undefined)).not.toBeNull();
  });
});
