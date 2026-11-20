import { useId } from "react";

import { copy } from "../copy";

type Shape = "box" | "rounded" | "decision";
type FlowNode = { id: string; label: string; shape: Shape };
type FlowEdge = { from: string; to: string; label: string };
type Direction = "TD" | "TB" | "BT" | "LR" | "RL";
type Point = { x: number; y: number };

const header = /^(?:flowchart|graph)\s+(TD|TB|BT|LR|RL)$/u;
const node =
  /^([A-Za-z0-9_]+(?:-[A-Za-z0-9_]+)*)(?:\s*(?:\[([^\]\r\n]{1,100})\]|\(([^)\r\n]{1,100})\)|\{([^}\r\n]{1,100})\}))?/u;
const arrow = /^-->\s*(?:\|([^|\r\n]{1,64})\|\s*)?/u;
const nodeWidth = 144;
const nodeHeight = 64;
const margin = 16;
const gapAcross = 24;
const gapAlong = 48;

function unquote(label: string) {
  const text = label.trim();
  return text.startsWith('"') && text.endsWith('"') ? text.slice(1, -1) : text;
}

// Bounded data-only subset. Unsupported syntax is preserved as source.
function parseFlowchart(source: string) {
  if (source.length > 6_000) return;
  const lines = source
    .split(/[;\n]/u)
    .map((line) => line.trim())
    .filter(
      (line) => line && (!line.startsWith("%%") || line.startsWith("%%{")),
    );
  const direction = header.exec(lines.shift() ?? "")?.[1] as
    Direction | undefined;
  if (!direction) return;
  const nodes = new Map<string, FlowNode>();
  const edges: FlowEdge[] = [];

  function readNode(value: string) {
    const match = node.exec(value);
    if (!match) return;
    const [raw, id, box, rounded, decision] = match;
    const label = box ?? rounded ?? decision;
    if (label !== undefined || !nodes.has(id)) {
      nodes.set(id, {
        id,
        label: label === undefined ? id : unquote(label),
        shape: decision ? "decision" : rounded ? "rounded" : "box",
      });
    }
    return { id, rest: value.slice(raw.length).trimStart() };
  }

  for (const line of lines) {
    let current = readNode(line);
    if (!current) return;
    while (current.rest) {
      const link = arrow.exec(current.rest);
      if (!link) return;
      const target = readNode(current.rest.slice(link[0].length));
      if (!target) return;
      edges.push({
        from: current.id,
        to: target.id,
        label: unquote(link[1] ?? ""),
      });
      current = target;
      if (edges.length > 32) return;
    }
    if (nodes.size > 20) return;
  }
  if (!nodes.size) return;
  return { direction, nodes, edges };
}

function layoutFlowchart(source: string) {
  const graph = parseFlowchart(source);
  if (!graph) return;
  const { nodes, edges, direction } = graph;
  const indegree = new Map(Array.from(nodes.keys(), (id) => [id, 0]));
  const level = new Map(indegree);
  for (const edge of edges) indegree.set(edge.to, indegree.get(edge.to)! + 1);
  const queue = Array.from(nodes.keys()).filter((id) => indegree.get(id) === 0);
  for (let i = 0; i < queue.length; i += 1) {
    for (const edge of edges) {
      if (edge.from !== queue[i]) continue;
      level.set(
        edge.to,
        Math.max(level.get(edge.to)!, level.get(edge.from)! + 1),
      );
      const remaining = indegree.get(edge.to)! - 1;
      indegree.set(edge.to, remaining);
      if (remaining === 0) queue.push(edge.to);
    }
  }
  if (queue.length !== nodes.size) return; // Cycles require a full layout engine.

  const rows: string[][] = [];
  for (const [id, rank] of level) (rows[rank] ??= []).push(id);
  const horizontal = direction === "LR" || direction === "RL";
  const reversed = direction === "BT" || direction === "RL";
  const along = (horizontal ? nodeWidth : nodeHeight) + gapAlong;
  const across = (horizontal ? nodeHeight : nodeWidth) + gapAcross;
  const depth = margin * 2 + rows.length * along - gapAlong;
  const breadth =
    margin * 2 +
    Math.max(...rows.map((row) => row.length)) * across -
    gapAcross;
  const points = new Map<string, Point>();
  rows.forEach((row, rank) => {
    const position = margin + (along - gapAlong) / 2 + rank * along;
    const axis = reversed ? depth - position : position;
    row.forEach((id, index) => {
      const offset = breadth / 2 + (index - (row.length - 1) / 2) * across;
      points.set(
        id,
        horizontal ? { x: axis, y: offset } : { x: offset, y: axis },
      );
    });
  });
  return {
    ...graph,
    points,
    horizontal,
    reversed,
    width: horizontal ? depth : breadth,
    height: horizontal ? breadth : depth,
  };
}

function labelLines(label: string, columns = 18, maxLines = 3) {
  const lines = [""];
  let size = 0;
  for (const char of label) {
    const weight = /^[\x00-\x7F]$/u.test(char) ? 1 : 2;
    if (size + weight > columns) {
      if (lines.length === maxLines) {
        const last = maxLines - 1;
        lines[last] = Array.from(lines[last]).slice(0, -1).join("") + "…";
        break;
      }
      lines.push("");
      size = 0;
    }
    lines[lines.length - 1] += char;
    size += weight;
  }
  return lines.map((line) => line.trim());
}

export function AssistantFlowchart({ source }: { source: string }) {
  const id = useId();
  const graph = layoutFlowchart(source);
  const text = copy.conversations;
  return (
    <figure className="assistant-flowchart">
      {graph ? (
        <div
          className="assistant-rich-scroll focus-visible-control"
          role="region"
          tabIndex={0}
          aria-label={text.flowchart}
        >
          <svg
            width={graph.width}
            height={graph.height}
            style={{ width: graph.width, height: graph.height }}
            viewBox={["0", "0", graph.width, graph.height].join(" ")}
            role="img"
            aria-labelledby={id + "-title"}
            aria-describedby={id + "-description"}
          >
            <title id={id + "-title"}>{text.flowchart}</title>
            <desc id={id + "-description"}>
              {Array.from(graph.nodes.values(), (item) => item.label).join(
                ", ",
              )}
              {graph.edges
                .map(
                  (edge) =>
                    " · " +
                    graph.nodes.get(edge.from)?.label +
                    " → " +
                    graph.nodes.get(edge.to)?.label +
                    (edge.label ? " (" + edge.label + ")" : ""),
                )
                .join("")}
            </desc>
            <defs>
              <marker
                id={id + "-arrow"}
                viewBox="0 0 8 8"
                refX={8}
                refY={4}
                markerWidth={8}
                markerHeight={8}
                orient="auto"
              >
                <path
                  className="assistant-flowchart__arrowhead"
                  d="M 0 0 L 8 4 L 0 8 Z"
                />
              </marker>
            </defs>
            {graph.edges.map((edge, index) => {
              const start = graph.points.get(edge.from)!;
              const end = graph.points.get(edge.to)!;
              const delta =
                ((graph.horizontal ? nodeWidth : nodeHeight) / 2) *
                (graph.reversed ? -1 : 1);
              const x1 = start.x + (graph.horizontal ? delta : 0);
              const x2 = end.x - (graph.horizontal ? delta : 0);
              const y1 = start.y + (graph.horizontal ? 0 : delta);
              const y2 = end.y - (graph.horizontal ? 0 : delta);
              return (
                <g key={index}>
                  <line
                    x1={x1}
                    x2={x2}
                    y1={y1}
                    y2={y2}
                    className="assistant-flowchart__edge"
                    markerEnd={"url(#" + id + "-arrow)"}
                  />
                  {edge.label && (
                    <text
                      className="assistant-flowchart__edge-label"
                      x={(x1 + x2) / 2}
                      y={(y1 + y2) / 2 - 8}
                      textAnchor="middle"
                    >
                      <title>{edge.label}</title>
                      {labelLines(edge.label, 16, 1)[0]}
                    </text>
                  )}
                </g>
              );
            })}
            {Array.from(graph.nodes.values(), (item) => {
              const point = graph.points.get(item.id)!;
              const isDecision = item.shape === "decision";
              const lines = isDecision
                ? labelLines(item.label, 12, 2)
                : labelLines(item.label);
              return (
                <g
                  key={item.id}
                  transform={"translate(" + point.x + " " + point.y + ")"}
                >
                  <title>{item.label}</title>
                  {isDecision ? (
                    <polygon
                      className="assistant-flowchart__node"
                      points={[
                        [0, -nodeHeight / 2],
                        [nodeWidth / 2, 0],
                        [0, nodeHeight / 2],
                        [-nodeWidth / 2, 0],
                      ]
                        .map((vertex) => vertex.join(","))
                        .join(" ")}
                    />
                  ) : (
                    <rect
                      className="assistant-flowchart__node"
                      x={-nodeWidth / 2}
                      y={-nodeHeight / 2}
                      width={nodeWidth}
                      height={nodeHeight}
                      rx={item.shape === "rounded" ? 16 : 10}
                    />
                  )}
                  <text
                    className="assistant-flowchart__label"
                    textAnchor="middle"
                  >
                    {lines.map((line, i) => (
                      <tspan
                        key={i}
                        x={0}
                        y={4 + (i - (lines.length - 1) / 2) * 16}
                      >
                        {line}
                      </tspan>
                    ))}
                  </text>
                </g>
              );
            })}
          </svg>
        </div>
      ) : (
        <p className="assistant-flowchart__notice">
          {text.flowchartUnavailable}
        </p>
      )}
      <details className="assistant-flowchart__source">
        <summary className="focus-visible-control">
          {text.flowchartSource}
        </summary>
        <pre>
          <code>{source}</code>
        </pre>
      </details>
    </figure>
  );
}
