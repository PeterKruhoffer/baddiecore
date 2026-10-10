import { For } from "solid-js";
import type { JSX } from "@solidjs/web";
import type { JSONContent } from "@tiptap/core";
import * as stylex from "@stylexjs/stylex";
import type { RichTextConfig } from "../types";
import { richTextDocument, safeHref } from "../lib/richtext";

export const richTextStyles = stylex.create({
  content: { lineHeight: 1.65, overflowWrap: "anywhere", fontWeight: 400 },
  paragraph: { margin: "0 0 12px", whiteSpace: "pre-wrap" },
  heading: { lineHeight: 1.25, margin: "18px 0 10px" },
  quote: {
    borderLeftWidth: 3,
    borderLeftStyle: "solid",
    borderLeftColor: "#94a3b8",
    paddingLeft: 16,
    margin: "12px 0",
  },
  icon: {
    width: "1em",
    height: "1em",
    objectFit: "contain",
    verticalAlign: "-0.15em",
    marginInline: 2,
  },
  link: { color: "inherit", textDecoration: "underline" },
});

export function RichText(p: { value: string; config?: RichTextConfig; inline?: boolean }) {
  return <RichTextNode node={richTextDocument(p.value)} config={p.config} inline={p.inline} />;
}

export function RichTextNode(p: {
  node: JSONContent;
  config?: RichTextConfig;
  inline?: boolean;
  depth?: number;
}): JSX.Element {
  const children = () => (
    <For each={p.node.content}>
      {(node) => (
        <RichTextNode node={node} config={p.config} inline={p.inline} depth={(p.depth ?? 0) + 1} />
      )}
    </For>
  );
  if ((p.depth ?? 0) > 20) return null;
  const marked = (value: JSX.Element): JSX.Element => {
    for (const mark of p.node.marks ?? []) {
      const previous = value;
      switch (mark.type) {
        case "bold":
          value = <strong>{previous}</strong>;
          break;
        case "italic":
          value = <em>{previous}</em>;
          break;
        case "underline":
          value = <u>{previous}</u>;
          break;
        case "strike":
          value = <s>{previous}</s>;
          break;
        case "link": {
          const href = safeHref(mark.attrs?.href);
          if (href)
            value = (
              <a {...stylex.attrs(richTextStyles.link)} href={href}>
                {previous}
              </a>
            );
          break;
        }
      }
    }
    return value;
  };
  switch (p.node.type) {
    case "doc":
      return children();
    case "text":
      return marked(p.node.text ?? "");
    case "hardBreak":
      return <br />;
    case "icon": {
      const icon = p.config?.icons?.find((icon) => icon.id === p.node.attrs?.id);
      const src = safeHref(icon?.src);
      return icon && src
        ? marked(<img {...stylex.attrs(richTextStyles.icon)} src={src} alt={icon.label} />)
        : null;
    }
    default:
      if (p.inline) return <span>{children()} </span>;
      switch (p.node.type) {
        case "paragraph":
          return <p {...stylex.attrs(richTextStyles.paragraph)}>{children()}</p>;
        case "heading":
          return p.node.attrs?.level === 3 ? (
            <h3 {...stylex.attrs(richTextStyles.heading)}>{children()}</h3>
          ) : (
            <h2 {...stylex.attrs(richTextStyles.heading)}>{children()}</h2>
          );
        case "bulletList":
          return <ul>{children()}</ul>;
        case "orderedList":
          return (
            <ol start={p.node.attrs?.start ?? 1} type={p.node.attrs?.type ?? undefined}>
              {children()}
            </ol>
          );
        case "listItem":
          return <li>{children()}</li>;
        case "blockquote":
          return <blockquote {...stylex.attrs(richTextStyles.quote)}>{children()}</blockquote>;
        default:
          return null;
      }
  }
}
