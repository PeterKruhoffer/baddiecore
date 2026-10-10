import type { ComponentDef, Page, Template } from "../types";
import { richTextPlainText } from "../lib/richtext.ts";

export type ContentSearchResult =
  | { kind: "page"; pageId: string; title: string; detail: string }
  | { kind: "block"; pageId: string; blockId: string; title: string; detail: string }
  | { kind: "templates" | "components"; definitionId: string; title: string; detail: string };

/** Searches only the accessible content supplied by the caller. No network requests. */
export function searchContent(
  query: string,
  pages: readonly Page[],
  components: readonly ComponentDef[],
  templates: readonly Template[],
  canManageDefinitions = true,
): ContentSearchResult[] {
  const terms = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  if (!terms.length) return [];
  const matches = (text: string) => {
    const normalized = text.toLocaleLowerCase();
    return terms.every((term) => normalized.includes(term));
  };
  const definitions = new Map(components.map((component) => [component.id, component]));
  const results: ContentSearchResult[] = [];
  for (const page of pages) {
    if (matches(`${page.title} ${page.slug}`)) {
      results.push({ kind: "page", pageId: page.id, title: page.title, detail: page.slug });
    }
    for (const block of page.blocks) {
      const definition = definitions.get(block.component_id);
      const name = definition?.name ?? "Unknown component";
      const text = Object.entries(block.fields)
        .map(([key, value]) => {
          const field = definition?.fields.find((field) => field.name === key);
          return field?.kind === "richtext" ? richTextPlainText(value, field.richtext) : value;
        })
        .join(" ")
        .replace(/\s+/g, " ")
        .trim();
      // Page context helps qualify content queries, but title-only matches belong to the page.
      const content = `${name} ${text}`;
      if (
        terms.some((term) => content.toLocaleLowerCase().includes(term)) &&
        matches(`${page.title} ${page.slug} ${content}`)
      ) {
        const matchAt = Math.min(
          ...terms.map((term) => text.toLocaleLowerCase().indexOf(term)).filter((at) => at >= 0),
        );
        const start = Number.isFinite(matchAt) ? Math.max(0, matchAt - 35) : 0;
        const snippet = `${start ? "…" : ""}${text.slice(start, start + 140)}${text.length > start + 140 ? "…" : ""}`;
        results.push({
          kind: "block",
          pageId: page.id,
          blockId: block.id,
          title: `${name} · ${page.title}`,
          detail: `${page.slug}${snippet ? ` · ${snippet}` : ""}`,
        });
      }
    }
  }
  if (!canManageDefinitions) return results;
  for (const [kind, definitions] of [
    ["templates", templates],
    ["components", components],
  ] as const) {
    for (const definition of definitions) {
      if (matches(`${definition.name} ${definition.description}`)) {
        results.push({
          kind,
          definitionId: definition.id,
          title: definition.name,
          detail: definition.description,
        });
      }
    }
  }
  return results;
}
