import { For, Show } from "solid-js";
import * as stylex from "@stylexjs/stylex";
import { createRequest } from "./lib/resource";
import { api } from "./lib/api";
import { BlockRenderer, blocksInTemplateOrder } from "./components/Renderer";
import { common } from "./common.stylex";
const styles = stylex.create({
  public: { minHeight: "100vh", backgroundColor: "#fff" },
  state: { padding: "15vh 10vw" },
});
export function PublicPage() {
  const request = createRequest(() => api.content(location.pathname));
  const content = request.value;
  return (
    <main {...stylex.attrs(styles.public)}>
      <Show when={request.loading()}>
        <div {...stylex.attrs(styles.state)}>Loading page…</div>
      </Show>
      <Show when={request.error()}>
        <div {...stylex.attrs(styles.state)}>
          <p {...stylex.attrs(common.eyebrow)}>404</p>
          <h1>Page not found</h1>
          <p>This page is not published yet.</p>
        </div>
      </Show>
      <Show when={content()} keyed>
        {(data) => (
          <For each={blocksInTemplateOrder(data.page.blocks, data.template.regions)}>
            {(block) => {
              const def = data.components.find((c) => c.id === block.component_id);
              return (
                <Show when={def}>{(d) => <BlockRenderer block={block} definition={d()} />}</Show>
              );
            }}
          </For>
        )}
      </Show>
    </main>
  );
}
