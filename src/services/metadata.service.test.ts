import { describe, expect, test } from "bun:test";
import { invokeMock } from "./mocks";

describe("Metadata模块", () => {
  test("list_tables - 列出表", async () => {
    const result = await invokeMock<{ schema: string; name: string; type: string }[]>(
      "list_tables",
      { id: 1, database: "testdb", schema: "public" }
    );
    expect(result).toBeDefined();
    expect(Array.isArray(result)).toBe(true);
    expect(result.length).toBeGreaterThan(0);
    expect(result[0]).toHaveProperty("schema");
    expect(result[0]).toHaveProperty("name");
    expect(result[0]).toHaveProperty("type");
  });

  test("list_routines - 列出存储过程/函数", async () => {
    const result = await invokeMock<{ schema: string; name: string; type: string }[]>(
      "list_routines",
      { id: 1, database: "testdb", schema: "dbo" }
    );
    expect(result).toBeDefined();
    expect(Array.isArray(result)).toBe(true);
  });

  test("get_table_structure - 获取表结构", async () => {
    const result = await invokeMock<{ columns: { name: string; type: string; nullable: boolean }[] }>(
      "get_table_structure",
      { id: 1, schema: "public", table: "users" }
    );
    expect(result).toBeDefined();
    expect(result.columns).toBeDefined();
    expect(Array.isArray(result.columns)).toBe(true);
    expect(result.columns.length).toBeGreaterThan(0);
  });

  test("get_table_ddl - 获取表DDL", async () => {
    const result = await invokeMock<string>(
      "get_table_ddl",
      { id: 1, database: "testdb", schema: "public", table: "users" }
    );
    expect(result).toBeDefined();
    expect(typeof result).toBe("string");
    expect(result.length).toBeGreaterThan(0);
  });

  test("get_routine_ddl - 获取存储过程/函数DDL", async () => {
    const result = await invokeMock<string>(
      "get_routine_ddl",
      { id: 1, database: "testdb", schema: "dbo", name: "sync_user_stats", routineType: "procedure" }
    );
    expect(result).toBeDefined();
    expect(typeof result).toBe("string");
    expect(result.length).toBeGreaterThan(0);
  });

  test("get_table_metadata - 获取表元数据", async () => {
    const result = await invokeMock<any>(
      "get_table_metadata",
      { id: 1, database: "testdb", schema: "public", table: "users" }
    );
    expect(result).toBeDefined();
    expect(result.columns).toBeDefined();
    expect(result.indexes).toBeDefined();
    expect(result.foreignKeys).toBeDefined();
    expect(result.specialTypeSummaries).toBeDefined();
  });

  test("list_tables_by_conn - 通过连接列出表", async () => {
    const result = await invokeMock<{ schema: string; name: string; type: string }[]>(
      "list_tables_by_conn",
      {
        form: {
          driver: "postgres",
          host: "localhost",
          port: 5432,
          database: "testdb",
          username: "postgres",
        }
      }
    );
    expect(result).toBeDefined();
    expect(Array.isArray(result)).toBe(true);
    expect(result.length).toBeGreaterThan(0);
  });

  test("list_databases - 列出数据库", async () => {
    const result = await invokeMock<string[]>(
      "list_databases",
      {
        form: {
          driver: "postgres",
          host: "localhost",
          port: 5432,
          database: "testdb",
          username: "postgres",
        }
      }
    );
    expect(result).toBeDefined();
    expect(Array.isArray(result)).toBe(true);
    expect(result.length).toBeGreaterThan(0);
  });

  test("list_databases_by_id - 通过ID列出数据库", async () => {
    const result = await invokeMock<string[]>(
      "list_databases_by_id",
      { id: 1 }
    );
    expect(result).toBeDefined();
    expect(Array.isArray(result)).toBe(true);
    expect(result.length).toBeGreaterThan(0);
  });

  test("get_schema_overview - 获取schema概览", async () => {
    const result = await invokeMock<any>(
      "get_schema_overview",
      { id: 1, database: "testdb", schema: "public" }
    );
    expect(result).toBeDefined();
    expect(result.tables).toBeDefined();
    expect(Array.isArray(result.tables)).toBe(true);
  });

  test("get_schema_foreign_keys - 获取外键", async () => {
    const result = await invokeMock<any[]>(
      "get_schema_foreign_keys",
      { id: 1, database: "testdb", schema: "public" }
    );
    expect(result).toBeDefined();
    expect(Array.isArray(result)).toBe(true);
  });

  test("list_events - 列出事件", async () => {
    const result = await invokeMock<any[]>(
      "list_events",
      { connectionId: "1", database: "testdb" }
    );
    expect(result).toBeDefined();
    expect(Array.isArray(result)).toBe(true);
  });

  test("list_sequences - 列出序列", async () => {
    const result = await invokeMock<any[]>(
      "list_sequences",
      { connectionId: "1", database: "testdb" }
    );
    expect(result).toBeDefined();
    expect(Array.isArray(result)).toBe(true);
  });

  test("list_types - 列出类型", async () => {
    const result = await invokeMock<any[]>(
      "list_types",
      { connectionId: "1", database: "testdb" }
    );
    expect(result).toBeDefined();
    expect(Array.isArray(result)).toBe(true);
  });

  test("list_synonyms - 列出同义词", async () => {
    const result = await invokeMock<any[]>(
      "list_synonyms",
      { connectionId: "1", database: "testdb" }
    );
    expect(result).toBeDefined();
    expect(Array.isArray(result)).toBe(true);
  });

  test("list_packages - 列出包", async () => {
    const result = await invokeMock<any[]>(
      "list_packages",
      { connectionId: "1", database: "testdb" }
    );
    expect(result).toBeDefined();
    expect(Array.isArray(result)).toBe(true);
  });
});