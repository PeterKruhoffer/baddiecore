import { createSignal, For, Show } from "solid-js";
import * as stylex from "@stylexjs/stylex";
import type { Block, ComponentDef, Page, Region, Template } from "../types";
import { BlockRenderer, blocksInTemplateOrder } from "../components/Renderer";
import { common } from "../common.stylex";
const styles = stylex.create({
  editor: {
    height: { default: "100vh", "@media (max-width: 1050px)": "auto" },
    minHeight: { default: null, "@media (max-width: 1050px)": "100vh" },
    display: "flex",
    flexDirection: "column",
  },
  head: {
    minHeight: 83,
    backgroundColor: "#fffdf8",
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "#ddd6cb",
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "12px 20px",
    flexWrap: { default: null, "@media (max-width: 1050px)": "wrap" },
    gap: { default: null, "@media (max-width: 720px)": 10 },
  },
  bareInput: {
    borderWidth: 0,
    backgroundColor: "transparent",
    padding: 0,
    boxShadow: { default: "none", ":focus": "none" },
  },
  title: { font: "600 24px Georgia, serif", width: 320 },
  slug: { fontSize: 12, color: "#77716a", display: "block" },
  actions: {
    display: "flex",
    gap: { default: 8, "@media (max-width: 720px)": 10 },
    alignItems: "center",
    flexWrap: { default: null, "@media (max-width: 1050px)": "wrap" },
  },
  status: { fontSize: 11, color: "#77716a" },
  grid: {
    display: "grid",
    gridTemplateColumns: {
      default: "210px minmax(420px, 1fr) 260px",
      "@media (max-width: 1050px)": "190px 1fr",
      "@media (max-width: 720px)": "1fr",
    },
    minHeight: 0,
    flex: 1,
  },
  side: {
    backgroundColor: "#fffdf8",
    padding: 17,
    borderRightWidth: 1,
    borderRightStyle: "solid",
    borderRightColor: "#ddd6cb",
    overflow: { default: "auto", "@media (max-width: 1050px)": "visible" },
  },
  inspector: {
    borderLeftWidth: 1,
    borderLeftStyle: "solid",
    borderLeftColor: "#ddd6cb",
    borderRightWidth: 0,
    position: { default: null, "@media (max-width: 1050px)": "static" },
    gridColumn: { default: null, "@media (max-width: 1050px)": "1 / -1" },
    width: { default: null, "@media (max-width: 1050px)": "auto" },
  },
  sideHeading: { font: "600 17px Georgia, serif" },
  region: {
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "#ddd6cb",
    padding: "15px 0",
  },
  between: { display: "flex", justifyContent: "space-between" },
  small: { color: "#77716a" },
  blockRow: { display: "flex", margin: "5px 0" },
  blockMain: { flex: 1, textAlign: "left" },
  selectedButton: { backgroundColor: "#eee9fa", borderColor: "#aa98da" },
  canvasWrap: {
    overflow: { default: "auto", "@media (max-width: 1050px)": "visible" },
    backgroundColor: "#e8e3db",
    padding: "0 25px 50px",
  },
  tools: {
    height: 48,
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    fontSize: 12,
  },
  activeTool: { backgroundColor: "#fffdf8" },
  canvas: {
    maxWidth: 920,
    minHeight: "calc(100% - 40px)",
    margin: "auto",
    backgroundColor: "#fff",
    boxShadow: "0 5px 22px #372d2420",
  },
  mobile: { maxWidth: 390 },
  preview: {
    outlineWidth: 2,
    outlineStyle: "solid",
    cursor: "pointer",
    outlineColor: { default: "transparent", ":hover": "#6650a5" },
  },
  previewSelected: { outlineColor: "#6650a5" },
  inspectorTitle: {
    display: "flex",
    justifyContent: "space-between",
    padding: 11,
    backgroundColor: "#f4f0e8",
    borderRadius: 8,
  },
});
type Props = {
  page: Page;
  templates: Template[];
  components: ComponentDef[];
  onSave: (p: Page) => Promise<Page>;
  onPublish: (p: Page) => Promise<Page>;
  onDelete: (p: Page) => Promise<void>;
  onDirty: (dirty: boolean) => void;
};
export function PageEditor(p: Props) {
  const [draft, setDraft] = createSignal(structuredClone(p.page));
  const [selected, setSelected] = createSignal<string>();
  const [mobile, setMobile] = createSignal(false);
  const [busy, setBusy] = createSignal("");
  const [error, setError] = createSignal("");
  const [dirty, setDirty] = createSignal(false);
  let dragged: string | undefined;
  const template = () => p.templates.find((t) => t.id === draft().template_id);
  const update = (value: Page) => {
    setDraft(value);
    setDirty(true);
    p.onDirty(true);
  };
  const blocks = (r: string) => draft().blocks.filter((b) => b.region === r);
  function add(region: Region, id: string) {
    if (blocks(region.name).length >= region.max_components)
      return setError(`${region.name} allows up to ${region.max_components} components.`);
    const def = p.components.find((c) => c.id === id);
    if (!def || !region.allowed_components.includes(id))
      return setError("That component is not allowed in this region.");
    const block: Block = {
      id: crypto.randomUUID(),
      component_id: id,
      region: region.name,
      fields: Object.fromEntries(def.fields.map((f) => [f.name, ""])),
    };
    update({ ...draft(), blocks: [...draft().blocks, block] });
    setSelected(block.id);
    setError("");
  }
  function move(id: string, delta: number) {
    const region = draft().blocks.find((b) => b.id === id)?.region;
    if (!region) return;
    const indexes = draft()
        .blocks.map((b, i) => (b.region === region ? i : -1))
        .filter((i) => i >= 0),
      at = indexes.findIndex((i) => draft().blocks[i].id === id),
      swap = indexes[at + delta];
    if (swap === undefined) return;
    const copy = [...draft().blocks],
      own = indexes[at];
    [copy[own], copy[swap]] = [copy[swap], copy[own]];
    update({ ...draft(), blocks: copy });
  }
  function remove(id: string) {
    update({ ...draft(), blocks: draft().blocks.filter((b) => b.id !== id) });
    setSelected(undefined);
  }
  async function act(name: string, fn: () => Promise<Page>) {
    setBusy(name);
    setError("");
    try {
      const saved = await fn();
      setDraft(structuredClone(saved));
      setDirty(false);
      p.onDirty(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Request failed");
    } finally {
      setBusy("");
    }
  }
  async function publish() {
    setBusy("publish");
    setError("");
    try {
      const saved = await p.onSave(draft());
      setDraft(structuredClone(saved));
      setDirty(false);
      p.onDirty(false);
      setDraft(structuredClone(await p.onPublish(saved)));
    } catch (e) {
      setError(e instanceof Error ? e.message : "Request failed");
    } finally {
      setBusy("");
    }
  }
  async function deletePage() {
    if (!confirm(`Delete ${draft().title}?`)) return;
    setBusy("delete");
    setError("");
    try {
      await p.onDelete(draft());
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not delete page");
      setBusy("");
    }
  }
  const active = () => draft().blocks.find((b) => b.id === selected());
  return (
    <section {...stylex.attrs(styles.editor)}>
      <header {...stylex.attrs(styles.head)}>
        <div>
          <p {...stylex.attrs(common.eyebrow)}>Page editor</p>
          <input
            {...stylex.attrs(common.control, styles.bareInput, styles.title)}
            aria-label="Page title"
            disabled={!!busy()}
            value={draft().title}
            onInput={(e) => update({ ...draft(), title: e.currentTarget.value })}
          />
          <input
            {...stylex.attrs(common.control, styles.bareInput, styles.slug)}
            aria-label="Page path"
            disabled={!!busy()}
            value={draft().slug}
            onInput={(e) => update({ ...draft(), slug: e.currentTarget.value })}
          />
        </div>
        <div {...stylex.attrs(styles.actions)}>
          <span {...stylex.attrs(styles.status)}>
            {dirty()
              ? "Unsaved changes"
              : draft().published_revision === draft().revision
                ? "Published"
                : "Draft changes"}
          </span>
          <button
            {...stylex.attrs(common.button)}
            disabled={!!busy()}
            onClick={() => act("save", () => p.onSave(draft()))}
          >
            {busy() === "save" ? "Saving…" : "Save draft"}
          </button>
          <button
            {...stylex.attrs(common.button, common.primary)}
            disabled={!!busy()}
            onClick={publish}
          >
            {busy() === "publish" ? "Publishing…" : "Publish"}
          </button>
          <button
            {...stylex.attrs(common.button, common.danger)}
            aria-label="Delete page"
            disabled={!!busy()}
            onClick={deletePage}
          >
            Delete
          </button>
        </div>
      </header>
      {error() && (
        <p {...stylex.attrs(common.error)} role="alert">
          {error()}
        </p>
      )}
      <div {...stylex.attrs(styles.grid)} inert={!!busy()}>
        <aside {...stylex.attrs(styles.side)}>
          <h3 {...stylex.attrs(styles.sideHeading)}>Page structure</h3>
          <label {...stylex.attrs(common.label)}>
            Template
            <select
              {...stylex.attrs(common.control)}
              value={draft().template_id}
              onChange={(e) => update({ ...draft(), template_id: e.currentTarget.value })}
            >
              <For each={p.templates}>{(t) => <option value={t.id}>{t.name}</option>}</For>
            </select>
          </label>
          <For each={template()?.regions}>
            {(region) => (
              <div {...stylex.attrs(styles.region)}>
                <div {...stylex.attrs(styles.between)}>
                  <strong>{region.name}</strong>
                  <small {...stylex.attrs(styles.small)}>
                    {blocks(region.name).length}/{region.max_components}
                  </small>
                </div>
                <For each={blocks(region.name)}>
                  {(block) => {
                    const def = p.components.find((c) => c.id === block.component_id)!;
                    return (
                      <div {...stylex.attrs(styles.blockRow)}>
                        <button
                          {...stylex.attrs(
                            common.button,
                            styles.blockMain,
                            selected() === block.id && styles.selectedButton,
                          )}
                          draggable="true"
                          onDragStart={() => (dragged = block.id)}
                          onDragOver={(e) => e.preventDefault()}
                          onDrop={() => {
                            if (dragged && dragged !== block.id) {
                              const from = draft().blocks.findIndex((b) => b.id === dragged),
                                to = draft().blocks.findIndex((b) => b.id === block.id),
                                copy = [...draft().blocks];
                              copy.splice(to, 0, ...copy.splice(from, 1));
                              update({ ...draft(), blocks: copy });
                            }
                          }}
                          onClick={() => setSelected(block.id)}
                        >
                          <span>⠿</span>
                          {def?.name}
                        </button>
                        <button
                          {...stylex.attrs(common.button)}
                          type="button"
                          onClick={() => move(block.id, -1)}
                          aria-label={`Move ${def?.name} up`}
                        >
                          ↑
                        </button>
                        <button
                          {...stylex.attrs(common.button)}
                          type="button"
                          onClick={() => move(block.id, 1)}
                          aria-label={`Move ${def?.name} down`}
                        >
                          ↓
                        </button>
                      </div>
                    );
                  }}
                </For>
                <select
                  {...stylex.attrs(common.control)}
                  aria-label={`Add to ${region.name}`}
                  value=""
                  onChange={(e) => {
                    add(region, e.currentTarget.value);
                    e.currentTarget.value = "";
                  }}
                >
                  <option value="">+ Add component</option>
                  <For each={p.components.filter((c) => region.allowed_components.includes(c.id))}>
                    {(c) => <option value={c.id}>{c.name}</option>}
                  </For>
                </select>
              </div>
            )}
          </For>
        </aside>
        <main {...stylex.attrs(styles.canvasWrap)}>
          <div {...stylex.attrs(styles.tools)}>
            <span>Live preview</span>
            <div>
              <button
                {...stylex.attrs(common.button, !mobile() && styles.activeTool)}
                onClick={() => setMobile(false)}
              >
                Desktop
              </button>
              <button
                {...stylex.attrs(common.button, mobile() && styles.activeTool)}
                onClick={() => setMobile(true)}
              >
                Mobile
              </button>
            </div>
          </div>
          <div
            {...stylex.attrs(styles.canvas, mobile() && styles.mobile)}
            onClick={(e) => {
              if ((e.target as HTMLElement).closest("a")) e.preventDefault();
            }}
          >
            <For each={blocksInTemplateOrder(draft().blocks, template()?.regions ?? [])}>
              {(block) => {
                const def = p.components.find((c) => c.id === block.component_id);
                return (
                  <Show when={def}>
                    {(d) => (
                      <div
                        {...stylex.attrs(
                          styles.preview,
                          selected() === block.id && styles.previewSelected,
                        )}
                        onClick={() => setSelected(block.id)}
                      >
                        <BlockRenderer block={block} definition={d()} />
                      </div>
                    )}
                  </Show>
                );
              }}
            </For>
            <Show when={!draft().blocks.length}>
              <div>
                <p {...stylex.attrs(common.eyebrow)}>Empty canvas</p>
                <h2>Add your first component</h2>
                <p>Use a region on the left to begin.</p>
              </div>
            </Show>
          </div>
        </main>
        <aside {...stylex.attrs(styles.side, styles.inspector)}>
          <h3 {...stylex.attrs(styles.sideHeading)}>Properties</h3>
          <Show
            when={active()}
            fallback={
              <p {...stylex.attrs(common.muted)}>
                Select a component on the canvas to edit its content.
              </p>
            }
          >
            {(block) => {
              const def = () => p.components.find((c) => c.id === block()?.component_id)!;
              return (
                <>
                  <div {...stylex.attrs(styles.inspectorTitle)}>
                    <div>
                      <strong>{def().name}</strong>
                      <small {...stylex.attrs(styles.small)}>{block()?.region}</small>
                    </div>
                    <button
                      {...stylex.attrs(common.button, common.danger)}
                      onClick={() => block() && remove(block()!.id)}
                    >
                      Remove
                    </button>
                  </div>
                  <For each={def().fields}>
                    {(field) => (
                      <label {...stylex.attrs(common.label)}>
                        {field.label}
                        {field.kind === "textarea" ? (
                          <textarea
                            {...stylex.attrs(common.control, common.textarea)}
                            required={field.required}
                            value={block()?.fields[field.name] || ""}
                            onInput={(e) =>
                              update({
                                ...draft(),
                                blocks: draft().blocks.map((b) =>
                                  b.id === block()?.id
                                    ? {
                                        ...b,
                                        fields: {
                                          ...b.fields,
                                          [field.name]: e.currentTarget.value,
                                        },
                                      }
                                    : b,
                                ),
                              })
                            }
                          />
                        ) : (
                          <input
                            {...stylex.attrs(common.control)}
                            type={field.kind === "url" ? "url" : "text"}
                            required={field.required}
                            value={block()?.fields[field.name] || ""}
                            onInput={(e) =>
                              update({
                                ...draft(),
                                blocks: draft().blocks.map((b) =>
                                  b.id === block()?.id
                                    ? {
                                        ...b,
                                        fields: {
                                          ...b.fields,
                                          [field.name]: e.currentTarget.value,
                                        },
                                      }
                                    : b,
                                ),
                              })
                            }
                          />
                        )}
                      </label>
                    )}
                  </For>
                </>
              );
            }}
          </Show>
        </aside>
      </div>
    </section>
  );
}
