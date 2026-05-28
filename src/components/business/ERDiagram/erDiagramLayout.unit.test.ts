import { describe, test, expect } from "bun:test";
import { computeLayout } from "./erDiagramLayout";
import type { Node, Edge } from "@xyflow/react";

function makeNode(id: string, columnCount: number): Node {
  return {
    id,
    type: "tableNode",
    position: { x: 0, y: 0 },
    data: {
      columns: Array.from({ length: columnCount }, (_, i) => ({
        name: `col_${i}`,
        type: "text",
        isForeignKey: false,
      })),
    },
  };
}

function makeEdge(id: string, source: string, target: string): Edge {
  return { id, source, target, type: "default" };
}

describe("computeLayout", () => {
  test("assigns positions to all nodes", () => {
    const nodes = [makeNode("a", 3), makeNode("b", 2)];
    const edges = [makeEdge("e1", "a", "b")];
    const result = computeLayout(nodes, edges);
    for (const node of result.nodes) {
      expect(typeof node.position.x).toBe("number");
      expect(typeof node.position.y).toBe("number");
      expect(isFinite(node.position.x)).toBe(true);
      expect(isFinite(node.position.y)).toBe(true);
    }
  });

  test("returns empty nodes and edges for empty input", () => {
    const result = computeLayout([], []);
    expect(result.nodes).toEqual([]);
    expect(result.edges).toEqual([]);
  });

  test("preserves edges unchanged", () => {
    const nodes = [makeNode("a", 1), makeNode("b", 1)];
    const edges = [makeEdge("e1", "a", "b")];
    const result = computeLayout(nodes, edges);
    expect(result.edges).toEqual(edges);
  });

  test("single node is positioned", () => {
    const nodes = [makeNode("a", 2)];
    const result = computeLayout(nodes, []);
    expect(result.nodes[0].position.x).toBeDefined();
    expect(result.nodes[0].position.y).toBeDefined();
  });
});
