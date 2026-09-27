import { For, Show } from "solid-js";
import type { JSX } from "@solidjs/web";
import type { Block, ComponentDef, RendererName } from "../types";
type Props={block:Block; definition:ComponentDef};
const Link=(p:{href?:string;children:JSX.Element})=><Show when={p.href}><a class="button" href={p.href}>{p.children}</a></Show>;
export const renderers:Record<RendererName,(p:Props)=>JSX.Element>={
 hero:p=><section class="render hero"><p class="eyebrow">{p.block.fields.eyebrow}</p><h1>{p.block.fields.title||"Untitled hero"}</h1><p>{p.block.fields.body}</p><Link href={p.block.fields.button_url}>{p.block.fields.button_label}</Link></section>,
 text:p=><section class="render text"><h2>{p.block.fields.title||"Untitled section"}</h2><p>{p.block.fields.body}</p></section>,
 callout:p=><section class="render callout"><div><h2>{p.block.fields.title||"A useful callout"}</h2><p>{p.block.fields.body}</p></div><Link href={p.block.fields.button_url}>{p.block.fields.button_label}</Link></section>,
 cards:p=><section class="render cards"><h2>{p.block.fields.title||"Cards"}</h2><div class="card-grid"><For each={p.block.fields.body?.split("\n").filter(Boolean)||["Add one card per line"]}>{item=><article>{item}</article>}</For></div></section>
};
export function BlockRenderer(p:Props){return <>{renderers[p.definition.renderer](p)}</>}
