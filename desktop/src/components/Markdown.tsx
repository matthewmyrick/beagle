// Renders parsed markdown blocks as styled HTML. Pure projection of
// lib/markdown.ts's output — no parsing here.

import type { JSX } from "react";

import { parseBlocks, parseInline } from "../lib/markdown";
import type { Block } from "../lib/markdown";

function InlineRuns({ text }: { text: string }): JSX.Element {
  return (
    <>
      {parseInline(text).map((run, i) =>
        run.kind === "bold" ? (
          <strong key={i}>{run.text}</strong>
        ) : run.kind === "code" ? (
          <code key={i}>{run.text}</code>
        ) : (
          <span key={i}>{run.text}</span>
        ),
      )}
    </>
  );
}

function BlockView({ block }: { block: Block }): JSX.Element {
  switch (block.kind) {
    case "heading": {
      const Tag = `h${String(block.level + 1)}` as "h2" | "h3" | "h4";
      return (
        <Tag className={`md-h${String(block.level)}`}>
          <InlineRuns text={block.text} />
        </Tag>
      );
    }
    case "rule":
      return <hr className="md-rule" />;
    case "quote":
      return (
        <blockquote className="md-quote">
          <InlineRuns text={block.text} />
        </blockquote>
      );
    case "code":
      return <pre className="md-code">{block.text}</pre>;
    case "bullet":
      return (
        <div
          className={`md-bullet${block.checkbox === "done" ? " done" : ""}`}
          style={{ marginLeft: block.indent * 8 }}
        >
          <span className="md-marker">
            {block.checkbox === null ? "•" : block.checkbox === "done" ? "☑" : "☐"}
          </span>
          <span>
            <InlineRuns text={block.text} />
          </span>
        </div>
      );
    case "paragraph":
      return (
        <p className="md-p">
          <InlineRuns text={block.text} />
        </p>
      );
  }
}

export function Markdown({ source }: { source: string }): JSX.Element {
  return (
    <div className="markdown">
      {parseBlocks(source).map((block, i) => (
        <BlockView key={i} block={block} />
      ))}
    </div>
  );
}
