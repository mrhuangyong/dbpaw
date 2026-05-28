import { describe, test, expect } from "bun:test";
import {
  getNormalizedCellRange,
  buildRangeTSV,
  buildRangeCSV,
  buildRangeInsertSQL,
  buildRangeUpdateSQL,
  buildRowsTSV,
  buildRowsCSV,
  buildRowsInsertSQL,
  buildRowsUpdateSQL,
} from "./selectionCopy";

const columns = ["id", "name", "email"];
const rows: Record<string, unknown>[] = [
  { id: 1, name: "Alice", email: "alice@example.com" },
  { id: 2, name: "Bob", email: "bob@example.com" },
  { id: 3, name: "Charlie", email: "charlie@example.com" },
];

const getCellValue = (_row: number, _col: string, raw: unknown) => raw;
const cellValueToString = (v: unknown) =>
  v === null || v === undefined ? "" : String(v);
const formatSQLValue = (str: string, _raw: unknown, _mode: string, _driver: string) =>
  `'${str}'`;
const quoteIdentFn = (_driver: string, ident: string) => `\`${ident}\``;
const escapeSQLFn = (s: string) => s.replace(/'/g, "''");
const buildUpdateStatementFn = (
  _driver: string,
  table: string,
  set: string,
  where: string,
) => `UPDATE ${table} SET ${set} WHERE ${where}`;

describe("getNormalizedCellRange", () => {
  test("normalizes anchor and tip into min/max", () => {
    const result = getNormalizedCellRange(
      { row: 2, colIndex: 3 },
      { row: 0, colIndex: 1 },
    );
    expect(result).toEqual({ minRow: 0, maxRow: 2, minCol: 1, maxCol: 3 });
  });

  test("handles same cell selection", () => {
    const result = getNormalizedCellRange(
      { row: 1, colIndex: 2 },
      { row: 1, colIndex: 2 },
    );
    expect(result).toEqual({ minRow: 1, maxRow: 1, minCol: 2, maxCol: 2 });
  });

  test("handles inverted anchor/tip", () => {
    const result = getNormalizedCellRange(
      { row: 5, colIndex: 0 },
      { row: 0, colIndex: 5 },
    );
    expect(result).toEqual({ minRow: 0, maxRow: 5, minCol: 0, maxCol: 5 });
  });

  test("handles zero coordinates", () => {
    const result = getNormalizedCellRange(
      { row: 0, colIndex: 0 },
      { row: 0, colIndex: 0 },
    );
    expect(result).toEqual({ minRow: 0, maxRow: 0, minCol: 0, maxCol: 0 });
  });
});

describe("buildRangeTSV", () => {
  test("builds TSV for single cell", () => {
    const range = { minRow: 0, maxRow: 0, minCol: 0, maxCol: 0 };
    const result = buildRangeTSV(
      range,
      columns,
      rows,
      getCellValue,
      cellValueToString,
    );
    expect(result).toBe("1");
  });

  test("builds TSV for multi-cell range", () => {
    const range = { minRow: 0, maxRow: 1, minCol: 0, maxCol: 2 };
    const result = buildRangeTSV(
      range,
      columns,
      rows,
      getCellValue,
      cellValueToString,
    );
    expect(result).toBe(
      "1\tAlice\talice@example.com\n2\tBob\tbob@example.com",
    );
  });

  test("handles null values as empty string", () => {
    const rowsWithNull = [{ id: 1, name: null, email: "a@b.com" }];
    const range = { minRow: 0, maxRow: 0, minCol: 0, maxCol: 2 };
    const result = buildRangeTSV(
      range,
      columns,
      rowsWithNull,
      getCellValue,
      cellValueToString,
    );
    expect(result).toBe("1\t\ta@b.com");
  });

  test("returns empty string for empty range", () => {
    const range = { minRow: 0, maxRow: -1, minCol: 0, maxCol: 0 };
    const result = buildRangeTSV(
      range,
      columns,
      rows,
      getCellValue,
      cellValueToString,
    );
    expect(result).toBe("");
  });
});

describe("buildRangeCSV", () => {
  test("builds CSV for multi-cell range", () => {
    const range = { minRow: 0, maxRow: 1, minCol: 0, maxCol: 1 };
    const result = buildRangeCSV(
      range,
      columns,
      rows,
      getCellValue,
      cellValueToString,
    );
    expect(result).toBe("1,Alice\n2,Bob");
  });

  test("escapes values containing commas", () => {
    const rowsWithComma = [{ id: 1, name: "Alice, Jr.", email: "a@b.com" }];
    const range = { minRow: 0, maxRow: 0, minCol: 0, maxCol: 2 };
    const result = buildRangeCSV(
      range,
      columns,
      rowsWithComma,
      getCellValue,
      cellValueToString,
    );
    expect(result).toBe('1,"Alice, Jr.",a@b.com');
  });

  test("escapes values containing quotes", () => {
    const rowsWithQuote = [{ id: 1, name: 'Alice "Ali"', email: "a@b.com" }];
    const range = { minRow: 0, maxRow: 0, minCol: 0, maxCol: 2 };
    const result = buildRangeCSV(
      range,
      columns,
      rowsWithQuote,
      getCellValue,
      cellValueToString,
    );
    expect(result).toBe('1,"Alice ""Ali""",a@b.com');
  });

  test("escapes values containing newlines", () => {
    const rowsWithNewline = [
      { id: 1, name: "Alice\nSmith", email: "a@b.com" },
    ];
    const range = { minRow: 0, maxRow: 0, minCol: 0, maxCol: 2 };
    const result = buildRangeCSV(
      range,
      columns,
      rowsWithNewline,
      getCellValue,
      cellValueToString,
    );
    expect(result).toBe('1,"Alice\nSmith",a@b.com');
  });

  test("returns empty string for empty range", () => {
    const range = { minRow: 0, maxRow: -1, minCol: 0, maxCol: 0 };
    const result = buildRangeCSV(range, columns, rows, getCellValue, cellValueToString);
    expect(result).toBe("");
  });

  test("builds CSV for single cell", () => {
    const range = { minRow: 0, maxRow: 0, minCol: 0, maxCol: 0 };
    const result = buildRangeCSV(range, columns, rows, getCellValue, cellValueToString);
    expect(result).toBe("1");
  });
});

describe("buildRangeInsertSQL", () => {
  test("builds INSERT for single row", () => {
    const range = { minRow: 0, maxRow: 0, minCol: 0, maxCol: 1 };
    const result = buildRangeInsertSQL(
      range,
      columns,
      rows,
      getCellValue,
      formatSQLValue,
      quoteIdentFn,
      "mysql",
      "`users`",
    );
    expect(result).toBe(
      "INSERT INTO `users` (`id`, `name`) VALUES ('1', 'Alice');",
    );
  });

  test("builds INSERT for multiple rows", () => {
    const range = { minRow: 0, maxRow: 2, minCol: 0, maxCol: 0 };
    const result = buildRangeInsertSQL(
      range,
      columns,
      rows,
      getCellValue,
      formatSQLValue,
      quoteIdentFn,
      "mysql",
      "`users`",
    );
    const lines = result.split("\n");
    expect(lines).toHaveLength(3);
    expect(lines[0]).toContain("INSERT INTO");
  });

  test("returns empty string for empty range", () => {
    const range = { minRow: 0, maxRow: -1, minCol: 0, maxCol: 0 };
    const result = buildRangeInsertSQL(
      range, columns, rows, getCellValue, formatSQLValue, quoteIdentFn, "mysql", "`users`",
    );
    expect(result).toBe("");
  });
});

describe("buildRangeUpdateSQL", () => {
  test("returns empty string when no primary keys", () => {
    const range = { minRow: 0, maxRow: 0, minCol: 0, maxCol: 1 };
    const result = buildRangeUpdateSQL(
      range,
      columns,
      rows,
      [],
      getCellValue,
      formatSQLValue,
      quoteIdentFn,
      escapeSQLFn,
      buildUpdateStatementFn,
      "mysql",
      "`users`",
    );
    expect(result).toBe("");
  });

  test("builds UPDATE with PK in WHERE", () => {
    const range = { minRow: 0, maxRow: 0, minCol: 1, maxCol: 2 };
    const result = buildRangeUpdateSQL(
      range,
      columns,
      rows,
      ["id"],
      getCellValue,
      formatSQLValue,
      quoteIdentFn,
      escapeSQLFn,
      buildUpdateStatementFn,
      "mysql",
      "`users`",
    );
    expect(result).toContain("UPDATE `users` SET");
    expect(result).toContain("WHERE");
    expect(result).toContain("`id` = 1");
  });

  test("handles NULL PK values", () => {
    const rowsWithNullPk = [{ id: null, name: "Alice", email: "a@b.com" }];
    const range = { minRow: 0, maxRow: 0, minCol: 1, maxCol: 1 };
    const result = buildRangeUpdateSQL(
      range,
      columns,
      rowsWithNullPk,
      ["id"],
      getCellValue,
      formatSQLValue,
      quoteIdentFn,
      escapeSQLFn,
      buildUpdateStatementFn,
      "mysql",
      "`users`",
    );
    expect(result).toContain("`id` IS NULL");
  });
});

describe("buildRowsTSV", () => {
  test("builds TSV for selected rows", () => {
    const result = buildRowsTSV(
      [0, 2],
      columns,
      rows,
      getCellValue,
      cellValueToString,
    );
    expect(result).toBe(
      "1\tAlice\talice@example.com\n3\tCharlie\tcharlie@example.com",
    );
  });

  test("sorts row indexes", () => {
    const result = buildRowsTSV(
      [2, 0],
      columns,
      rows,
      getCellValue,
      cellValueToString,
    );
    expect(result).toBe(
      "1\tAlice\talice@example.com\n3\tCharlie\tcharlie@example.com",
    );
  });

  test("returns empty string for empty row indexes", () => {
    const result = buildRowsTSV(
      [],
      columns,
      rows,
      getCellValue,
      cellValueToString,
    );
    expect(result).toBe("");
  });

  test("handles null values as empty strings", () => {
    const rowsWithNull = [{ id: 1, name: null, email: "a@b.com" }];
    const result = buildRowsTSV([0], columns, rowsWithNull, getCellValue, cellValueToString);
    expect(result).toBe("1\t\ta@b.com");
  });
});

describe("buildRowsCSV", () => {
  test("builds CSV for selected rows", () => {
    const result = buildRowsCSV(
      [0, 1],
      columns,
      rows,
      getCellValue,
      cellValueToString,
    );
    expect(result).toBe(
      "1,Alice,alice@example.com\n2,Bob,bob@example.com",
    );
  });

  test("escapes special characters", () => {
    const specialRows = [{ id: 1, name: "A,B", email: 'C"D' }];
    const result = buildRowsCSV(
      [0],
      columns,
      specialRows,
      getCellValue,
      cellValueToString,
    );
    expect(result).toBe('1,"A,B","C""D"');
  });

  test("returns empty string for empty row indexes", () => {
    const result = buildRowsCSV([], columns, rows, getCellValue, cellValueToString);
    expect(result).toBe("");
  });

  test("builds CSV for single row", () => {
    const result = buildRowsCSV([0], columns, rows, getCellValue, cellValueToString);
    expect(result).toBe("1,Alice,alice@example.com");
  });
});

describe("buildRowsInsertSQL", () => {
  test("builds INSERT for selected rows", () => {
    const result = buildRowsInsertSQL(
      [0, 1],
      columns,
      rows,
      getCellValue,
      formatSQLValue,
      quoteIdentFn,
      "mysql",
      "`users`",
    );
    const lines = result.split("\n");
    expect(lines).toHaveLength(2);
    expect(lines[0]).toContain("INSERT INTO `users`");
  });

  test("returns empty string for empty row indexes", () => {
    const result = buildRowsInsertSQL(
      [],
      columns,
      rows,
      getCellValue,
      formatSQLValue,
      quoteIdentFn,
      "mysql",
      "`users`",
    );
    expect(result).toBe("");
  });

  test("builds INSERT for single row", () => {
    const result = buildRowsInsertSQL(
      [0], columns, rows, getCellValue, formatSQLValue, quoteIdentFn, "mysql", "`users`",
    );
    expect(result).toContain("INSERT INTO `users`");
    expect(result).toContain("'1'");
    expect(result).toContain("'Alice'");
  });
});

describe("buildRowsUpdateSQL", () => {
  test("returns empty string when no primary keys", () => {
    const result = buildRowsUpdateSQL(
      [0],
      columns,
      rows,
      [],
      getCellValue,
      formatSQLValue,
      quoteIdentFn,
      escapeSQLFn,
      buildUpdateStatementFn,
      "mysql",
      "`users`",
    );
    expect(result).toBe("");
  });

  test("builds UPDATE for selected rows with PK", () => {
    const result = buildRowsUpdateSQL(
      [0, 1],
      columns,
      rows,
      ["id"],
      getCellValue,
      formatSQLValue,
      quoteIdentFn,
      escapeSQLFn,
      buildUpdateStatementFn,
      "mysql",
      "`users`",
    );
    const lines = result.split("\n");
    expect(lines).toHaveLength(2);
    expect(lines[0]).toContain("UPDATE `users` SET");
    expect(lines[0]).toContain("`id` = 1");
  });

  test("builds UPDATE for single row with PK", () => {
    const result = buildRowsUpdateSQL(
      [0], columns, rows, ["id"], getCellValue, formatSQLValue, quoteIdentFn, escapeSQLFn, buildUpdateStatementFn, "mysql", "`users`",
    );
    expect(result).toContain("UPDATE `users` SET");
    expect(result).toContain("`id` = 1");
    expect(result).toContain("'Alice'");
  });
});
