import { describe, test, expect } from "bun:test";
import { buildDiagramData } from "./types";
import type { SchemaOverview, SchemaForeignKey } from "@/services/api";

function makeOverview(
  tables: Array<{
    schema: string;
    name: string;
    columns: Array<{ name: string; type: string }>;
  }>,
): SchemaOverview {
  return {
    tables: tables.map((t) => ({
      schema: t.schema,
      name: t.name,
      columns: t.columns.map((c) => ({
        name: c.name,
        type: c.type,
      })),
    })),
  } as SchemaOverview;
}

function makeFK(
  partial: Partial<SchemaForeignKey> & {
    name: string;
    sourceTable: string;
    sourceColumn: string;
    targetTable: string;
    targetColumn: string;
  },
): SchemaForeignKey {
  return {
    sourceSchema: null,
    targetSchema: null,
    onUpdate: null,
    onDelete: null,
    ...partial,
  } as SchemaForeignKey;
}

describe("buildDiagramData", () => {
  test("returns empty nodes and edges for empty overview", () => {
    const overview = makeOverview([]);
    const result = buildDiagramData(overview, []);
    expect(result.nodes).toEqual([]);
    expect(result.edges).toEqual([]);
  });

  test("filters out tables with no FK-related columns", () => {
    const overview = makeOverview([
      {
        schema: "public",
        name: "users",
        columns: [{ name: "id", type: "int" }],
      },
      {
        schema: "public",
        name: "logs",
        columns: [{ name: "message", type: "text" }],
      },
    ]);
    const fks = [
      makeFK({
        name: "fk_orders_user",
        sourceTable: "orders",
        sourceColumn: "user_id",
        targetTable: "users",
        targetColumn: "id",
      }),
    ];
    const result = buildDiagramData(overview, fks);
    expect(result.nodes).toHaveLength(1);
    expect(result.nodes[0].name).toBe("users");
  });

  test("flags FK source columns as isForeignKey", () => {
    const overview = makeOverview([
      {
        schema: "public",
        name: "orders",
        columns: [
          { name: "user_id", type: "int" },
          { name: "total", type: "decimal" },
        ],
      },
      {
        schema: "public",
        name: "users",
        columns: [{ name: "id", type: "int" }],
      },
    ]);
    const fks = [
      makeFK({
        name: "fk_orders_user",
        sourceTable: "orders",
        sourceColumn: "user_id",
        targetTable: "users",
        targetColumn: "id",
      }),
    ];
    const result = buildDiagramData(overview, fks);
    const ordersNode = result.nodes.find((n) => n.name === "orders")!;
    const userIdCol = ordersNode.columns.find((c) => c.name === "user_id")!;
    expect(userIdCol.isForeignKey).toBe(true);
  });

  test("includes FK target columns but does not flag as isForeignKey", () => {
    const overview = makeOverview([
      {
        schema: "public",
        name: "orders",
        columns: [{ name: "user_id", type: "int" }],
      },
      {
        schema: "public",
        name: "users",
        columns: [
          { name: "id", type: "int" },
          { name: "name", type: "text" },
        ],
      },
    ]);
    const fks = [
      makeFK({
        name: "fk_orders_user",
        sourceTable: "orders",
        sourceColumn: "user_id",
        targetTable: "users",
        targetColumn: "id",
      }),
    ];
    const result = buildDiagramData(overview, fks);
    const usersNode = result.nodes.find((n) => n.name === "users")!;
    const idCol = usersNode.columns.find((c) => c.name === "id")!;
    expect(idCol.isForeignKey).toBe(false);
    expect(usersNode.columns.find((c) => c.name === "name")).toBeUndefined();
  });

  test("uses fk.sourceSchema for edge source when present", () => {
    const overview = makeOverview([
      {
        schema: "myschema",
        name: "orders",
        columns: [{ name: "user_id", type: "int" }],
      },
      {
        schema: "public",
        name: "users",
        columns: [{ name: "id", type: "int" }],
      },
    ]);
    const fks = [
      makeFK({
        name: "fk1",
        sourceSchema: "myschema",
        sourceTable: "orders",
        sourceColumn: "user_id",
        targetTable: "users",
        targetColumn: "id",
      }),
    ];
    const result = buildDiagramData(overview, fks);
    expect(result.edges[0].source).toBe("myschema.orders");
  });

  test("falls back to table schema from overview when fk.sourceSchema is null", () => {
    const overview = makeOverview([
      {
        schema: "myschema",
        name: "orders",
        columns: [{ name: "user_id", type: "int" }],
      },
      {
        schema: "public",
        name: "users",
        columns: [{ name: "id", type: "int" }],
      },
    ]);
    const fks = [
      makeFK({
        name: "fk1",
        sourceTable: "orders",
        sourceColumn: "user_id",
        targetTable: "users",
        targetColumn: "id",
      }),
    ];
    const result = buildDiagramData(overview, fks);
    expect(result.edges[0].source).toBe("myschema.orders");
    expect(result.edges[0].target).toBe("public.users");
  });

  test('falls back to "public" when schema not found', () => {
    const overview = makeOverview([]);
    const fks = [
      makeFK({
        name: "fk1",
        sourceTable: "orders",
        sourceColumn: "user_id",
        targetTable: "users",
        targetColumn: "id",
      }),
    ];
    const result = buildDiagramData(overview, fks);
    expect(result.edges[0].source).toBe("public.orders");
    expect(result.edges[0].target).toBe("public.users");
  });

  test("generates deterministic edge IDs", () => {
    const overview = makeOverview([
      {
        schema: "public",
        name: "orders",
        columns: [{ name: "user_id", type: "int" }],
      },
      {
        schema: "public",
        name: "users",
        columns: [{ name: "id", type: "int" }],
      },
    ]);
    const fks = [
      makeFK({
        name: "fk_orders_user",
        sourceTable: "orders",
        sourceColumn: "user_id",
        targetTable: "users",
        targetColumn: "id",
      }),
    ];
    const result1 = buildDiagramData(overview, fks);
    const result2 = buildDiagramData(overview, fks);
    expect(result1.edges[0].id).toBe(result2.edges[0].id);
    expect(result1.edges[0].id).toBe("fk_orders_user-orders.user_id-users.id");
  });

  test("produces multiple edges for multiple FKs", () => {
    const overview = makeOverview([
      {
        schema: "public",
        name: "orders",
        columns: [
          { name: "user_id", type: "int" },
          { name: "product_id", type: "int" },
        ],
      },
      {
        schema: "public",
        name: "users",
        columns: [{ name: "id", type: "int" }],
      },
      {
        schema: "public",
        name: "products",
        columns: [{ name: "id", type: "int" }],
      },
    ]);
    const fks = [
      makeFK({
        name: "fk1",
        sourceTable: "orders",
        sourceColumn: "user_id",
        targetTable: "users",
        targetColumn: "id",
      }),
      makeFK({
        name: "fk2",
        sourceTable: "orders",
        sourceColumn: "product_id",
        targetTable: "products",
        targetColumn: "id",
      }),
    ];
    const result = buildDiagramData(overview, fks);
    expect(result.edges).toHaveLength(2);
  });

  test("handles self-referential FK", () => {
    const overview = makeOverview([
      {
        schema: "public",
        name: "employees",
        columns: [
          { name: "id", type: "int" },
          { name: "manager_id", type: "int" },
        ],
      },
    ]);
    const fks = [
      makeFK({
        name: "fk_manager",
        sourceTable: "employees",
        sourceColumn: "manager_id",
        targetTable: "employees",
        targetColumn: "id",
      }),
    ];
    const result = buildDiagramData(overview, fks);
    expect(result.nodes).toHaveLength(1);
    expect(result.nodes[0].columns).toHaveLength(2);
    expect(result.edges[0].source).toBe(result.edges[0].target);
  });

  test("passes through onUpdate and onDelete values", () => {
    const overview = makeOverview([
      {
        schema: "public",
        name: "orders",
        columns: [{ name: "user_id", type: "int" }],
      },
      {
        schema: "public",
        name: "users",
        columns: [{ name: "id", type: "int" }],
      },
    ]);
    const fks = [
      makeFK({
        name: "fk1",
        sourceTable: "orders",
        sourceColumn: "user_id",
        targetTable: "users",
        targetColumn: "id",
        onUpdate: "CASCADE",
        onDelete: "SET NULL",
      }),
    ];
    const result = buildDiagramData(overview, fks);
    expect(result.edges[0].onUpdate).toBe("CASCADE");
    expect(result.edges[0].onDelete).toBe("SET NULL");
  });

  test("node ID is schema.table format", () => {
    const overview = makeOverview([
      {
        schema: "myschema",
        name: "orders",
        columns: [{ name: "user_id", type: "int" }],
      },
      {
        schema: "public",
        name: "users",
        columns: [{ name: "id", type: "int" }],
      },
    ]);
    const fks = [
      makeFK({
        name: "fk1",
        sourceTable: "orders",
        sourceColumn: "user_id",
        targetTable: "users",
        targetColumn: "id",
      }),
    ];
    const result = buildDiagramData(overview, fks);
    expect(result.nodes[0].id).toBe("myschema.orders");
  });
});
