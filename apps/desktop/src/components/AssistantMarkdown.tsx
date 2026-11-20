import { Fragment, type ReactNode } from "react";

import { copy } from "../copy";
import { AssistantFlowchart } from "./AssistantFlowchart";
import {
  LinkifiedText,
  SafeExternalLink,
  trustedExternalUrl,
} from "./ExternalTextLink";

// Render a bounded Markdown subset as React nodes. Never interpret HTML,
// images, or arbitrary URLs from model output.
const inlineTokens =
  /`[^`\n]+`|\[[^\[\]\n]+\]\([^\s()]+\)|\*\*[^*\n]+\*\*|\*[^*\n]+\*/gu;
const heading = /^(#{1,6})[ \t]+(.+)$/u;
const fence = /^ {0,3}(`{3,}|~{3,})([^`~]*)$/u;
const unordered = /^ {0,3}[-+*][ \t]+(.+)$/u;
const ordered = /^ {0,3}(\d{1,9})[.)][ \t]+(.+)$/u;
const quote = /^ {0,3}>[ \t]?(.*)$/u;

function tableCells(line: string) {
  const text = line.trim();
  const cells = [""];
  let lastDelimiter = -1;
  for (let i = 0; i < text.length; i += 1) {
    if (text[i] === "\\" && (text[i + 1] === "|" || text[i + 1] === "\\")) {
      cells[cells.length - 1] += text[++i];
    } else if (text[i] === "|") {
      cells.push("");
      lastDelimiter = i;
    } else {
      cells[cells.length - 1] += text[i];
    }
  }
  if (lastDelimiter === -1) return;
  if (text.startsWith("|")) cells.shift();
  if (lastDelimiter === text.length - 1) cells.pop();
  return cells.length ? cells.map((cell) => cell.trim()) : undefined;
}

function tableAt(lines: string[], start: number) {
  const labels = tableCells(lines[start] ?? "");
  const divider = tableCells(lines[start + 1] ?? "");
  if (
    !labels ||
    !divider ||
    labels.length !== divider.length ||
    !divider.every((cell) => /^:?-+:?$/u.test(cell))
  )
    return;
  const alignment = divider.map((cell) =>
    cell.endsWith(":") ? (cell.startsWith(":") ? "center" : "right") : "left",
  );
  return { labels, alignment };
}

function inline(value: string): ReactNode[] {
  const parts: ReactNode[] = [];
  let cursor = 0;
  for (const match of value.matchAll(inlineTokens)) {
    const start = match.index;
    if (start > cursor) {
      parts.push(
        <LinkifiedText key={cursor} text={value.slice(cursor, start)} />,
      );
    }
    const token = match[0];
    if (token.startsWith("`")) {
      parts.push(<code key={start}>{token.slice(1, -1)}</code>);
    } else if (token.startsWith("[")) {
      const boundary = token.lastIndexOf("](");
      const href = trustedExternalUrl(token.slice(boundary + 2, -1));
      parts.push(
        href ? (
          <SafeExternalLink key={start} href={href}>
            {token.slice(1, boundary)}
          </SafeExternalLink>
        ) : (
          token
        ),
      );
    } else if (token.startsWith("**")) {
      parts.push(
        <strong key={start}>
          <LinkifiedText text={token.slice(2, -2)} />
        </strong>,
      );
    } else {
      parts.push(
        <em key={start}>
          <LinkifiedText text={token.slice(1, -1)} />
        </em>,
      );
    }
    cursor = start + token.length;
  }
  if (cursor < value.length) {
    parts.push(<LinkifiedText key={cursor} text={value.slice(cursor)} />);
  }
  return parts;
}

export function AssistantMarkdown({
  text,
  streaming = false,
}: {
  text: string;
  streaming?: boolean;
}) {
  const lines = text.replace(/\r\n?/gu, "\n").split("\n");
  const blocks: ReactNode[] = [];
  let i = 0;
  const simpleBlock = (line: string) =>
    !line.trim() ||
    fence.test(line) ||
    heading.test(line) ||
    unordered.test(line) ||
    ordered.test(line) ||
    quote.test(line);

  while (i < lines.length) {
    const start = i;
    const line = lines[i++];
    if (!line.trim()) continue;

    const code = fence.exec(line);
    const title = heading.exec(line);
    const firstOrdered = ordered.exec(line);
    const table = !simpleBlock(line) ? tableAt(lines, start) : undefined;
    if (code) {
      const codeLines: string[] = [];
      let closed = false;
      while (i < lines.length) {
        const end = fence.exec(lines[i]);
        if (
          end &&
          end[1][0] === code[1][0] &&
          end[1].length >= code[1].length &&
          !end[2].trim()
        ) {
          i += 1;
          closed = true;
          break;
        }
        codeLines.push(lines[i++]);
      }
      const source = codeLines.join("\n");
      if (code[2].trim().toLowerCase() === "mermaid") {
        blocks.push(
          streaming && !closed ? (
            <p key={start} className="assistant-flowchart__notice">
              {copy.conversations.flowchartPreparing}
            </p>
          ) : (
            <AssistantFlowchart key={start} source={source} />
          ),
        );
      } else {
        blocks.push(
          <pre key={start}>
            <code>{source}</code>
          </pre>,
        );
      }
    } else if (title) {
      const Heading =
        title[1].length === 1 ? "h2" : title[1].length === 2 ? "h3" : "h4";
      blocks.push(<Heading key={start}>{inline(title[2])}</Heading>);
    } else if (firstOrdered || unordered.test(line)) {
      const pattern = firstOrdered ? ordered : unordered;
      const items: ReactNode[] = [];
      do {
        const item = pattern.exec(lines[i - 1])!;
        items.push(<li key={i - 1}>{inline(item[firstOrdered ? 2 : 1])}</li>);
        if (i >= lines.length || !pattern.test(lines[i])) break;
        i += 1;
      } while (i <= lines.length);
      blocks.push(
        firstOrdered ? (
          <ol key={start} start={Number(firstOrdered[1])}>
            {items}
          </ol>
        ) : (
          <ul key={start}>{items}</ul>
        ),
      );
    } else if (quote.test(line)) {
      const content = [quote.exec(line)![1]];
      while (i < lines.length && quote.test(lines[i])) {
        content.push(quote.exec(lines[i++])![1]);
      }
      blocks.push(
        <blockquote key={start}>{inline(content.join("\n"))}</blockquote>,
      );
    } else if (table) {
      i += 1; // Skip the delimiter row.
      const rows: string[][] = [];
      while (i < lines.length && !simpleBlock(lines[i]) && !tableAt(lines, i)) {
        const cells = tableCells(lines[i]);
        if (!cells) break;
        rows.push(cells);
        i += 1;
      }
      blocks.push(
        <div
          key={start}
          className="assistant-rich-scroll focus-visible-control"
          role="region"
          tabIndex={0}
          aria-label={copy.conversations.markdownTable}
        >
          <table className="assistant-markdown-table">
            <thead>
              <tr>
                {table.labels.map((label, col) => (
                  <th key={col} scope="col" data-align={table.alignment[col]}>
                    {inline(label)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((row, index) => (
                <tr key={index}>
                  {table.labels.map((_, col) => (
                    <td key={col} data-align={table.alignment[col]}>
                      {inline(row[col] ?? "")}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>,
      );
    } else {
      const paragraph = [line];
      while (i < lines.length && !simpleBlock(lines[i]) && !tableAt(lines, i)) {
        paragraph.push(lines[i++]);
      }
      blocks.push(<p key={start}>{inline(paragraph.join("\n"))}</p>);
    }
  }

  return <Fragment>{blocks}</Fragment>;
}
