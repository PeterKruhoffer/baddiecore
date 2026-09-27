import { For, Show } from "solid-js";
import type { JSX } from "@solidjs/web";
import type { Block, ComponentDef, RendererName } from "../types";
type Props = { block: Block; definition: ComponentDef };
function safeHref(href?: string) {
  if (!href || /[\u0000-\u001f\u007f\\]/.test(href) || /^\s*\/\//.test(href))
    return;
  try {
    const url = new URL(href, "https://example.invalid");
    if (
      url.protocol === "http:" ||
      url.protocol === "https:" ||
      (url.origin === "https://example.invalid" && url.protocol === "https:")
    )
      return href;
  } catch {}
}
const Link = (p: { href?: string; children: JSX.Element }) => (
  <Show when={safeHref(p.href)}>
    {(href) => (
      <a class="button" href={href()}>
        {p.children}
      </a>
    )}
  </Show>
);
export function blocksInTemplateOrder(
  blocks: Block[],
  regions: { name: string }[],
) {
  const order = new Map(regions.map((region, index) => [region.name, index]));
  return blocks
    .map((block, index) => ({ block, index }))
    .sort(
      (a, b) =>
        (order.get(a.block.region) ?? regions.length) -
          (order.get(b.block.region) ?? regions.length) || a.index - b.index,
    )
    .map(({ block }) => block);
}
export const renderers: Record<RendererName, (p: Props) => JSX.Element> = {
  hero: (p) => (
    <section class="render hero">
      <p class="eyebrow">{p.block.fields.eyebrow}</p>
      <h1>{p.block.fields.title || "Untitled hero"}</h1>
      <p>{p.block.fields.body}</p>
      <Link href={p.block.fields.button_url}>
        {p.block.fields.button_label}
      </Link>
    </section>
  ),
  text: (p) => (
    <section class="render text">
      <h2>{p.block.fields.title || "Untitled section"}</h2>
      <p>{p.block.fields.body}</p>
    </section>
  ),
  callout: (p) => (
    <section class="render callout">
      <div>
        <h2>{p.block.fields.title || "A useful callout"}</h2>
        <p>{p.block.fields.body}</p>
      </div>
      <Link href={p.block.fields.button_url}>
        {p.block.fields.button_label}
      </Link>
    </section>
  ),
  cards: (p) => (
    <section class="render cards">
      <h2>{p.block.fields.title || "Cards"}</h2>
      <div class="card-grid">
        <For
          each={
            p.block.fields.body?.split("\n").filter(Boolean) || [
              "Add one card per line",
            ]
          }
        >
          {(item) => <article>{item}</article>}
        </For>
      </div>
    </section>
  ),
};
export function BlockRenderer(p: Props) {
  return <>{renderers[p.definition.renderer](p)}</>;
}
