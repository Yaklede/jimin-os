import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { AssistantMarkdown } from "./AssistantMarkdown";

const render = (text: string) =>
  renderToStaticMarkup(<AssistantMarkdown text={text} />);

describe("assistant Markdown", () => {
  it("renders a Markdown table with semantic headings, alignment and escaped cell delimiters", () => {
    const markup = render(
      "요약입니다.\n| 일감 | 수량 |\n| --- | ---: |\n| **데이터 \\| 검증** | 3 |",
    );
    expect(markup).toContain("<p>요약입니다.</p>");
    expect(markup).toContain('<th scope="col" data-align="right">수량</th>');
    expect(markup).toContain("<strong>데이터 | 검증</strong>");
    expect(markup).toContain('<td data-align="right">3</td>');
  });

  it("renders a labeled Mermaid branch inline and keeps its source available", () => {
    const markup = render(
      "```mermaid\nflowchart TD\nA[접수] --> B{승인?}\nB -->|예| C[등록]\nB -->|아니요| D[보류]\n```",
    );
    expect(markup).toContain("<svg");
    expect(markup.match(/<line /gu)).toHaveLength(3);
    expect(markup).toContain("승인? → 등록 (예)");
    expect(markup).toContain("<details");
    expect(markup).toContain("흐름도 원문 보기");
  });

  it("waits for a streaming Mermaid fence to close", () => {
    const draft = "```mermaid\nflowchart LR\nA --> B";
    const pending = renderToStaticMarkup(
      <AssistantMarkdown text={draft} streaming />,
    );
    expect(pending).toContain("흐름도를 그리고 있어요.");
    expect(pending).not.toContain("<svg");
    expect(pending).not.toContain("flowchart LR");
    const closed = renderToStaticMarkup(
      <AssistantMarkdown text={draft + "\n```\n"} streaming />,
    );
    expect(closed).toContain("<svg");
    expect(closed).not.toContain("흐름도를 그리고 있어요.");
  });

  it("does not execute or partially draw unsupported Mermaid directives", () => {
    const markup = render(
      '```mermaid\nflowchart TB\nA --> B\nclick A "javascript:alert(1)"\n```',
    );
    expect(markup).not.toContain("<svg");
    expect(markup).not.toContain("href=");
    expect(markup).toContain("<details");
    expect(markup).toContain("javascript:alert(1)");
  });

  it("renders a structured streamed answer and HTTPS references", () => {
    const markup = render(
      "## 내일 할 일\n\n- **첫 작업**\n- 두 번째\n\n3. 순서\n\n[원문](https://example.com/task)\n\n```ts\nconst value = 1;",
    );
    expect(markup).toContain("<h3>내일 할 일</h3>");
    expect(markup).toContain("<ul><li><strong>첫 작업</strong></li>");
    expect(markup).toContain('<ol start="3"><li>순서</li></ol>');
    expect(markup).toContain('href="https://example.com/task"');
    expect(markup).toContain("<pre><code>const value = 1;</code></pre>");
  });

  it("does not execute HTML, load remote images, or open unsafe links", () => {
    const markup = render(
      '<img src="x" onerror="alert(1)">\n![image](https://example.com/image)\n[unsafe](javascript:alert)',
    );
    expect(markup).toContain("&lt;img");
    expect(markup).not.toContain("<img");
    expect(markup).not.toContain('href="javascript:');
  });
});
