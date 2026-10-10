import type { JSONContent } from "@tiptap/core";
import type { RichTextConfig, RichTextFeature } from "../types";

export const richTextFeatures: { id: RichTextFeature; label: string }[] = [
  { id: "bold", label: "Bold" },
  { id: "italic", label: "Italic" },
  { id: "underline", label: "Underline" },
  { id: "strike", label: "Strikethrough" },
  { id: "heading", label: "Headings" },
  { id: "bullet_list", label: "Bullet list" },
  { id: "ordered_list", label: "Numbered list" },
  { id: "blockquote", label: "Quote" },
  { id: "link", label: "Link" },
];
export const defaultRichTextFeatures = richTextFeatures.map((feature) => feature.id);

export function safeHref(href?: string): string | undefined {
  // Match the server's relative-path or HTTP(S) policy before URL normalization.
  // oxlint-disable-next-line no-control-regex
  if (!href || /[\u0000-\u0020\u007f\\\s]/.test(href) || href.startsWith("//")) return;
  if (href.startsWith("/")) return href;
  try {
    const url = new URL(href);
    if (url.protocol === "http:" || url.protocol === "https:") return href;
  } catch {}
}

export function richTextDocument(value: string): JSONContent {
  if (value.trimStart().startsWith("{")) {
    try {
      const doc = JSON.parse(value);
      if (doc.type === "doc" && Array.isArray(doc.content)) return doc;
    } catch {}
  }
  // Do not pass legacy strings to Tiptap's HTML parser.
  return {
    type: "doc",
    content: value.split("\n").map((text) => ({
      type: "paragraph",
      ...(text ? { content: [{ type: "text", text }] } : {}),
    })),
  };
}

export function richTextPlainText(value: string, config?: RichTextConfig): string {
  function text(node: JSONContent, depth = 0): string {
    if (depth > 20) return "";
    if (node.type === "text") return node.text ?? "";
    if (node.type === "hardBreak") return "\n";
    if (node.type === "icon")
      return config?.icons?.find((icon) => icon.id === node.attrs?.id)?.label ?? "";
    const separator = node.type === "paragraph" || node.type === "heading" ? "" : "\n";
    return (node.content ?? []).map((node) => text(node, depth + 1)).join(separator);
  }
  return text(richTextDocument(value));
}
