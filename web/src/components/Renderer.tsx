import { For, Show } from "solid-js";
import type { JSX } from "@solidjs/web";
import * as stylex from "@stylexjs/stylex";
import type { Block, ComponentDef, RendererName } from "../types";
import { RichText, RichTextNode } from "./RichText";
import { richTextDocument, safeHref } from "../lib/richtext";
import { common } from "../common.stylex";
const styles = stylex.create({
  render: { padding: "70px clamp(30px, 8vw, 90px)" },
  heading: {
    fontFamily: "Georgia, serif",
    margin: "0 0 15px",
    lineHeight: 1.04,
  },
  h1: { fontSize: "clamp(42px, 7vw, 76px)" },
  h2: { fontSize: 38 },
  paragraph: {
    whiteSpace: "pre-line",
    lineHeight: 1.65,
    maxWidth: 680,
    color: "#58524c",
  },
  button: {
    display: "inline-block",
    padding: "11px 17px",
    borderRadius: 7,
    marginTop: 12,
  },
  hero: { backgroundColor: "#f0eaf9", paddingBlock: 100 },
  text: { maxWidth: 850, margin: "auto" },
  callout: {
    margin: 35,
    backgroundColor: "#282521",
    color: "white",
    borderRadius: 13,
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: 45,
  },
  calloutParagraph: { color: "#d1cbc4" },
  cards: { backgroundColor: "#f6f1e8" },
  cardGrid: {
    display: "grid",
    gridTemplateColumns: {
      default: "repeat(3, 1fr)",
      "@media (max-width: 720px)": "1fr",
    },
    gap: 13,
  },
  card: {
    backgroundColor: "white",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#ddd6cb",
    borderRadius: 10,
    padding: 25,
  },
});
type Props = { block: Block; definition: ComponentDef };
function FieldValue(p: Props & { name: string; inline?: boolean; fallback?: string }) {
  const field = () => p.definition.fields.find((field) => field.name === p.name);
  const value = () => p.block.fields[p.name] || p.fallback || "";
  return (
    <Show when={field()?.kind === "richtext"} fallback={value()}>
      <RichText value={value()} config={field()?.richtext} inline={p.inline} />
    </Show>
  );
}
function Body(p: Props & { callout?: boolean }) {
  return (
    <Show
      when={p.definition.fields.find((f) => f.name === "body")?.kind === "richtext"}
      fallback={
        <p {...stylex.attrs(styles.paragraph, p.callout && styles.calloutParagraph)}>
          {p.block.fields.body}
        </p>
      }
    >
      <div {...stylex.attrs(styles.paragraph, p.callout && styles.calloutParagraph)}>
        <FieldValue {...p} name="body" />
      </div>
    </Show>
  );
}
const Link = (p: { href?: string; children: JSX.Element }) => (
  <Show when={safeHref(p.href)}>
    {(href) => (
      <a {...stylex.attrs(common.primary, styles.button)} href={href()}>
        {p.children}
      </a>
    )}
  </Show>
);
export function blocksInTemplateOrder(blocks: Block[], regions: { name: string }[]) {
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
    <section {...stylex.attrs(styles.render, styles.hero)}>
      <p {...stylex.attrs(common.eyebrow, styles.paragraph)}>
        <FieldValue {...p} name="eyebrow" inline />
      </p>
      <h1 {...stylex.attrs(styles.heading, styles.h1)}>
        <FieldValue {...p} name="title" inline fallback="Untitled hero" />
      </h1>
      <Body {...p} />
      <Link href={p.block.fields.button_url}>
        <FieldValue {...p} name="button_label" inline />
      </Link>
    </section>
  ),
  text: (p) => (
    <section {...stylex.attrs(styles.render, styles.text)}>
      <h2 {...stylex.attrs(styles.heading, styles.h2)}>
        <FieldValue {...p} name="title" inline fallback="Untitled section" />
      </h2>
      <Body {...p} />
    </section>
  ),
  callout: (p) => (
    <section {...stylex.attrs(styles.render, styles.callout)}>
      <div>
        <h2 {...stylex.attrs(styles.heading, styles.h2)}>
          <FieldValue {...p} name="title" inline fallback="A useful callout" />
        </h2>
        <Body {...p} callout />
      </div>
      <Link href={p.block.fields.button_url}>
        <FieldValue {...p} name="button_label" inline />
      </Link>
    </section>
  ),
  cards: (p) => (
    <section {...stylex.attrs(styles.render, styles.cards)}>
      <h2 {...stylex.attrs(styles.heading, styles.h2)}>
        <FieldValue {...p} name="title" inline fallback="Cards" />
      </h2>
      <div {...stylex.attrs(styles.cardGrid)}>
        <Show
          when={p.definition.fields.find((f) => f.name === "body")?.kind !== "richtext"}
          fallback={
            <For each={richTextDocument(p.block.fields.body ?? "").content}>
              {(node) => (
                <article {...stylex.attrs(styles.card)}>
                  <RichTextNode
                    node={node}
                    config={p.definition.fields.find((f) => f.name === "body")?.richtext}
                  />
                </article>
              )}
            </For>
          }
        >
          <For each={p.block.fields.body?.split("\n").filter(Boolean) || ["Add one card per line"]}>
            {(item) => <article {...stylex.attrs(styles.card)}>{item}</article>}
          </For>
        </Show>
      </div>
    </section>
  ),
  external: (p) => (
    <section {...stylex.attrs(styles.render, styles.cards)}>
      <p {...stylex.attrs(common.eyebrow)}>External component · {p.definition.id}</p>
      <h2 {...stylex.attrs(styles.heading, styles.h2)}>{p.definition.name}</h2>
      <p {...stylex.attrs(styles.paragraph)}>
        Content preview. The connected app controls this component's appearance.
      </p>
      <dl>
        <For each={p.definition.fields}>
          {(field) => (
            <>
              <dt>
                <strong>{field.label}</strong>
              </dt>
              <dd {...stylex.attrs(styles.paragraph)}>
                <FieldValue {...p} name={field.name} fallback="Not set" />
              </dd>
            </>
          )}
        </For>
      </dl>
    </section>
  ),
};
export function BlockRenderer(p: Props) {
  return <>{renderers[p.definition.renderer](p)}</>;
}
