import { For, type Component } from "solid-js";
import { Dynamic } from "solid-js/web";
import type { LayoutResponse, Rendering } from "./cmsTypes";
import { sanitizePreviewUrl } from "./previewUrl";

type RenderingProps = {
  rendering: Rendering;
  selected: boolean;
  onSelect: (renderingId: string) => void;
};

function field(rendering: Rendering, name: string): string {
  return rendering.fields.find((item) => item.name === name)?.value ?? "";
}

function selectionHandler(props: RenderingProps) {
  return (event: MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
    props.onSelect(props.rendering.id);
  };
}

function HeroRendering(props: RenderingProps) {
  return (
    <section
      class="hero"
      classList={{ selected: props.selected }}
      data-rendering-id={props.rendering.id}
      onClick={selectionHandler(props)}
    >
      <p class="eyebrow">{field(props.rendering, "eyebrow")}</p>
      <h1>{field(props.rendering, "heading")}</h1>
      <p>{field(props.rendering, "body")}</p>
    </section>
  );
}

function RichTextRendering(props: RenderingProps) {
  return (
    <section
      class="content-block"
      classList={{ selected: props.selected }}
      data-rendering-id={props.rendering.id}
      onClick={selectionHandler(props)}
    >
      <h2>{field(props.rendering, "heading")}</h2>
      <p>{field(props.rendering, "body")}</p>
    </section>
  );
}

function CtaRendering(props: RenderingProps) {
  return (
    <section
      class="cta"
      classList={{ selected: props.selected }}
      data-rendering-id={props.rendering.id}
      onClick={selectionHandler(props)}
    >
      <h2>{field(props.rendering, "heading")}</h2>
      <a href={sanitizePreviewUrl(field(props.rendering, "buttonUrl"))}>
        {field(props.rendering, "buttonText")}
      </a>
    </section>
  );
}

function UnknownRendering(props: RenderingProps) {
  return (
    <section
      class="content-block"
      classList={{ selected: props.selected }}
      data-rendering-id={props.rendering.id}
      onClick={selectionHandler(props)}
    >
      <h2>{props.rendering.name}</h2>
      <For each={props.rendering.fields}>
        {(item) => <p><strong>{item.label}:</strong> {item.value}</p>}
      </For>
    </section>
  );
}

const renderingComponents: Record<string, Component<RenderingProps>> = {
  hero: HeroRendering,
  "rich-text": RichTextRendering,
  cta: CtaRendering
};

export function PreviewPage(props: {
  layout: LayoutResponse;
  selectedRenderingId?: string;
  onSelect: (renderingId: string) => void;
}) {
  return (
    <For
      each={props.layout.placeholders.flatMap((placeholder) => placeholder.renderings)}
      fallback={<section class="content-block"><h2>No renderings</h2><p>This item has no composed page content.</p></section>}
    >
      {(rendering) => (
        <Dynamic
          component={renderingComponents[rendering.componentKey] ?? UnknownRendering}
          rendering={rendering}
          selected={rendering.id === props.selectedRenderingId}
          onSelect={props.onSelect}
        />
      )}
    </For>
  );
}
