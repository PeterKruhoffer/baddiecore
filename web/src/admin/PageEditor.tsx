import { createSignal, For, lazy, Show } from "solid-js";
import * as stylex from "@stylexjs/stylex";
import type { Block, ComponentDef, Page, Region, Review, Template } from "../types";
import { BlockRenderer, blocksInTemplateOrder } from "../components/Renderer";
import { common } from "../common.stylex";
import { joinPath, pageParentPaths, parentPath, pathSegment } from "./pageTree";
const RichTextEditor = lazy(() => import("./RichTextEditor"), { export: "RichTextEditor" });
const styles = stylex.create({
  editor: {
    height: { default: "100%", "@media (max-width: 1050px)": "auto" },
    minHeight: "100%",
    display: "flex",
    flexDirection: "column",
  },
  head: {
    minHeight: 83,
    backgroundColor: "#fff",
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "#dce2ea",
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
  title: { fontSize: 24, fontWeight: 650, width: "min(320px, 100%)" },
  slug: { fontSize: 12, color: "#64748b", display: "block", overflowWrap: "anywhere" },
  actions: {
    display: "flex",
    gap: { default: 8, "@media (max-width: 720px)": 10 },
    alignItems: "center",
    flexWrap: { default: null, "@media (max-width: 1050px)": "wrap" },
  },
  grid: {
    display: "grid",
    gridTemplateColumns: {
      default: "minmax(320px, 0.85fr) minmax(0, 1.15fr)",
      "@media (max-width: 1050px)": "minmax(280px, 1fr) minmax(0, 1fr)",
      "@media (max-width: 720px)": "1fr",
    },
    minHeight: 0,
    flex: 1,
  },
  side: {
    backgroundColor: "#f5f7fa",
    padding: 20,
    borderRightWidth: 1,
    borderRightStyle: "solid",
    borderRightColor: "#dce2ea",
    overflow: { default: "auto", "@media (max-width: 1050px)": "visible" },
  },
  sideHeading: { fontSize: 16, fontWeight: 650, margin: "0 0 12px" },
  region: {
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "#dce2ea",
    padding: "15px 0",
  },
  between: { display: "flex", justifyContent: "space-between" },
  small: { color: "#64748b" },
  blockRow: { display: "flex", backgroundColor: "#f8fafc", gap: 4 },
  blockMain: { flex: 1, textAlign: "left" },
  selectedButton: { backgroundColor: "#eff6ff", color: "#1d4ed8", borderColor: "#2563eb" },
  block: {
    margin: "10px 0",
    backgroundColor: "#fff",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#dce2ea",
    borderRadius: 4,
    overflow: "hidden",
  },
  selectedBlock: { borderColor: "#2563eb" },
  fields: { padding: "4px 16px 14px" },
  settings: { backgroundColor: "#fff", padding: 14, borderRadius: 4, marginBottom: 20 },
  summary: { cursor: "pointer", fontWeight: 600 },
  contentOnly: { gridTemplateColumns: "1fr" },
  hidden: { display: "none" },
  tabs: { display: "flex", gap: 6, backgroundColor: "#fff", padding: "8px 20px" },
  footer: {
    display: "flex",
    justifyContent: "space-between",
    flexWrap: "wrap",
    gap: 12,
    padding: "12px 20px",
    backgroundColor: "#fff",
    fontSize: 12,
    color: "#64748b",
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "#dce2ea",
  },
  metadata: { display: "grid", gridTemplateColumns: "1fr 1fr", gap: 14, margin: "24px 0" },
  metadataValue: { margin: 0, overflowWrap: "anywhere" },
  review: { marginTop: 16 },
  canvasWrap: {
    overflow: { default: "auto", "@media (max-width: 1050px)": "visible" },
    backgroundColor: "#f1f5f9",
    padding: "0 20px 30px",
  },
  tools: {
    height: 48,
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    fontSize: 12,
  },
  activeTool: { backgroundColor: "#eff6ff", color: "#1d4ed8", borderColor: "#2563eb" },
  canvas: {
    maxWidth: 920,
    minHeight: "calc(100% - 40px)",
    margin: "auto",
    backgroundColor: "#fff",
    boxShadow: "0 2px 10px #0f172a0d",
  },
  mobile: { maxWidth: 390 },
  preview: {
    outlineWidth: 2,
    outlineStyle: "solid",
    cursor: "pointer",
    outlineColor: { default: "transparent", ":hover": "#2563eb" },
    outlineOffset: -2,
  },
  previewSelected: { outlineColor: "#2563eb" },
  pathHelp: { fontSize: 12, lineHeight: 1.45, color: "#64748b" },
  pathPreview: { overflowWrap: "anywhere", color: "#2563eb" },
});
type Props = {
  page: Page;
  initialBlockId?: string;
  pages: Page[];
  templates: Template[];
  components: ComponentDef[];
  isAdmin: boolean;
  paths: string[];
  review?: Review;
  onSave: (p: Page) => Promise<Page>;
  onPublish: (p: Page) => Promise<Page>;
  onSubmit: (p: Page) => Promise<Page>;
  onDelete: (p: Page) => Promise<void>;
  onDirty: (dirty: boolean) => void;
};
export function PageEditor(p: Props) {
  const [draft, setDraft] = createSignal(structuredClone(p.page));
  const [parent, setParent] = createSignal(parentPath(p.page.slug));
  const [segment, setSegment] = createSignal(pathSegment(p.page.slug));
  const [aliasesText, setAliasesText] = createSignal((p.page.aliases ?? []).join("\n"));
  const [selected, setSelected] = createSignal<string | undefined>(
    p.initialBlockId ?? p.page.blocks[0]?.id,
  );
  const [experience, setExperience] = createSignal(true);
  const [mobile, setMobile] = createSignal(false);
  const [busy, setBusy] = createSignal("");
  const [error, setError] = createSignal("");
  const [dirty, setDirty] = createSignal(false);
  let publishDialog!: HTMLDialogElement;
  let dragged: string | undefined;
  const template = () => p.templates.find((t) => t.id === draft().template_id);
  const isRoot = () => p.page.slug === "/";
  const availableParents = () =>
    [...new Set([...pageParentPaths(p.pages), ...p.paths, parentPath(p.page.slug)])]
      .sort()
      .filter((path) => path !== p.page.slug && !path.startsWith(`${p.page.slug}/`));
  const validPath = () => isRoot() || /^[A-Za-z0-9_-]+$/.test(segment());
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
  async function publish(submit = false) {
    setBusy(submit ? "submit" : "publish");
    setError("");
    try {
      const saved = dirty() ? await p.onSave(draft()) : draft();
      setDraft(structuredClone(saved));
      setDirty(false);
      p.onDirty(false);
      setDraft(structuredClone(await (submit ? p.onSubmit(saved) : p.onPublish(saved))));
      if (!submit) publishDialog.close();
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
  function changeField(id: string, name: string, value: string) {
    update({
      ...draft(),
      blocks: draft().blocks.map((b) =>
        b.id === id ? { ...b, fields: { ...b.fields, [name]: value } } : b,
      ),
    });
  }
  return (
    <section {...stylex.attrs(styles.editor)}>
      <header {...stylex.attrs(styles.head)}>
        <div>
          <p {...stylex.attrs(common.eyebrow)}>Site / Page editor</p>
          <input
            {...stylex.attrs(common.control, styles.bareInput, styles.title)}
            aria-label="Page title"
            disabled={!!busy()}
            value={draft().title}
            onInput={(e) => update({ ...draft(), title: e.currentTarget.value })}
          />
          <code {...stylex.attrs(styles.slug)}>{draft().slug}</code>
        </div>
        <div {...stylex.attrs(styles.actions)}>
          <span
            {...stylex.attrs(
              common.badge,
              draft().published_revision === draft().revision && !dirty() && common.live,
            )}
          >
            {draft().published_revision === draft().revision && !dirty() ? "Published" : "Draft"}
          </span>
          <button
            {...stylex.attrs(common.button)}
            disabled={!!busy() || !validPath()}
            onClick={() => act("save", () => p.onSave(draft()))}
          >
            {busy() === "save" ? "Saving…" : "Save draft"}
          </button>
          <button
            {...stylex.attrs(common.button, !p.isAdmin && common.primary)}
            disabled={!!busy() || !validPath()}
            onClick={() => void publish(true)}
          >
            {busy() === "submit" ? "Submitting…" : "Submit for review"}
          </button>
          <Show when={p.isAdmin}>
            <button
              {...stylex.attrs(common.button, common.primary)}
              disabled={!!busy() || !validPath()}
              onClick={() => {
                setError("");
                publishDialog.showModal();
              }}
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
          </Show>
        </div>
      </header>
      <div {...stylex.attrs(styles.tabs)} role="group" aria-label="Editing mode">
        <button
          {...stylex.attrs(common.button, !experience() && styles.activeTool)}
          aria-pressed={!experience() ? "true" : "false"}
          onClick={() => setExperience(false)}
        >
          Content
        </button>
        <button
          {...stylex.attrs(common.button, experience() && styles.activeTool)}
          aria-pressed={experience() ? "true" : "false"}
          onClick={() => setExperience(true)}
        >
          Experience
        </button>
      </div>
      {error() && (
        <p {...stylex.attrs(common.error)} role="alert">
          {error()}
        </p>
      )}
      <div {...stylex.attrs(styles.grid, !experience() && styles.contentOnly)} inert={!!busy()}>
        <aside {...stylex.attrs(styles.side)}>
          <details {...stylex.attrs(styles.settings)}>
            <summary {...stylex.attrs(styles.summary)}>Page details · {template()?.name}</summary>
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
            <h3 {...stylex.attrs(styles.sideHeading)}>URL</h3>
            <Show
              when={!isRoot()}
              fallback={
                <p {...stylex.attrs(styles.pathHelp)}>
                  The root page stays at <code>/</code> and cannot be moved or renamed.
                </p>
              }
            >
              <label {...stylex.attrs(common.label)}>
                Parent
                <select
                  {...stylex.attrs(common.control)}
                  value={parent()}
                  onChange={(e) => {
                    setParent(e.currentTarget.value);
                    update({
                      ...draft(),
                      slug: joinPath(e.currentTarget.value, segment()),
                    });
                  }}
                >
                  <For each={availableParents()}>
                    {(path) => <option value={path}>{path}</option>}
                  </For>
                </select>
              </label>
              <label {...stylex.attrs(common.label)}>
                URL segment
                <input
                  {...stylex.attrs(common.control)}
                  required
                  pattern="[A-Za-z0-9_\-]+"
                  value={segment()}
                  onInput={(e) => {
                    setSegment(e.currentTarget.value);
                    update({
                      ...draft(),
                      slug: joinPath(parent(), e.currentTarget.value),
                    });
                  }}
                />
              </label>
              <p {...stylex.attrs(styles.pathHelp)}>
                Path preview: <code {...stylex.attrs(styles.pathPreview)}>{draft().slug}</code>
              </p>
              <Show when={!validPath()}>
                <p {...stylex.attrs(common.error)}>Use letters, numbers, hyphens or underscores.</p>
              </Show>
              <p {...stylex.attrs(styles.pathHelp)}>
                Saving a move or rename also moves every descendant draft URL. Published pages keep
                their current URLs until each changed page is published again.
              </p>
            </Show>
            <label {...stylex.attrs(common.label)}>
              Route aliases
              <textarea
                {...stylex.attrs(common.control)}
                rows={3}
                placeholder="/summer-sale"
                aria-describedby="route-alias-help"
                value={aliasesText()}
                onInput={(e) => {
                  setAliasesText(e.currentTarget.value);
                  update({
                    ...draft(),
                    aliases: e.currentTarget.value
                      .split("\n")
                      .map((path) => path.trim())
                      .filter(Boolean),
                  });
                }}
              />
            </label>
            <p id="route-alias-help" {...stylex.attrs(styles.pathHelp)}>
              One absolute path per line, using letters, numbers, hyphens or underscores. After
              publication, each alias permanently redirects (301) to this page's published path.
              Query parameters are preserved. The page path stays canonical and appears in the
              browser. Clear this field and publish to remove aliases.
            </p>
          </details>
          <h3 {...stylex.attrs(styles.sideHeading)}>Page structure</h3>
          <For each={template()?.regions}>
            {(region) => (
              <div {...stylex.attrs(styles.region)}>
                <div {...stylex.attrs(styles.between)}>
                  <strong>{region.name}</strong>
                  <small {...stylex.attrs(styles.small)}>
                    {blocks(region.name).length}/{region.max_components}
                  </small>
                </div>
                <For each={blocks(region.name).map((b) => b.id)}>
                  {(id) => {
                    const block = () => draft().blocks.find((b) => b.id === id)!;
                    const def = p.components.find((c) => c.id === block().component_id)!;
                    return (
                      <div
                        {...stylex.attrs(styles.block, selected() === id && styles.selectedBlock)}
                      >
                        <div {...stylex.attrs(styles.blockRow)}>
                          <button
                            {...stylex.attrs(
                              common.button,
                              styles.blockMain,
                              selected() === id && styles.selectedButton,
                            )}
                            draggable="true"
                            onDragStart={() => (dragged = id)}
                            onDragOver={(e) => e.preventDefault()}
                            onDrop={() => {
                              if (dragged && dragged !== id) {
                                const from = draft().blocks.findIndex((b) => b.id === dragged),
                                  to = draft().blocks.findIndex((b) => b.id === id),
                                  copy = [...draft().blocks];
                                copy.splice(to, 0, ...copy.splice(from, 1));
                                update({ ...draft(), blocks: copy });
                              }
                            }}
                            aria-expanded={selected() === id ? "true" : "false"}
                            onClick={() => setSelected(selected() === id ? undefined : id)}
                          >
                            <span>⠿</span>
                            {def?.name}
                          </button>
                          <button
                            {...stylex.attrs(common.button)}
                            type="button"
                            onClick={() => move(id, -1)}
                            aria-label={`Move ${def?.name} up`}
                          >
                            ↑
                          </button>
                          <button
                            {...stylex.attrs(common.button)}
                            type="button"
                            onClick={() => move(id, 1)}
                            aria-label={`Move ${def?.name} down`}
                          >
                            ↓
                          </button>
                        </div>
                        <Show when={selected() === id}>
                          <div {...stylex.attrs(styles.fields)}>
                            <For each={def?.fields}>
                              {(field) => (
                                <Show
                                  when={field.kind !== "richtext"}
                                  fallback={
                                    <div {...stylex.attrs(common.label)}>
                                      <span>
                                        {field.label}
                                        {field.required ? " *" : ""}
                                      </span>
                                      <RichTextEditor
                                        field={field}
                                        pages={p.pages}
                                        value={block().fields[field.name] || ""}
                                        onChange={(value) => changeField(id, field.name, value)}
                                      />
                                    </div>
                                  }
                                >
                                  <label {...stylex.attrs(common.label)}>
                                    {field.label}
                                    {field.required ? " *" : ""}
                                    {field.kind === "textarea" ? (
                                      <textarea
                                        {...stylex.attrs(common.control, common.textarea)}
                                        required={field.required}
                                        value={block().fields[field.name] || ""}
                                        onInput={(e) =>
                                          changeField(id, field.name, e.currentTarget.value)
                                        }
                                      />
                                    ) : (
                                      <input
                                        {...stylex.attrs(common.control)}
                                        type="text"
                                        required={field.required}
                                        value={block().fields[field.name] || ""}
                                        onInput={(e) =>
                                          changeField(id, field.name, e.currentTarget.value)
                                        }
                                      />
                                    )}
                                  </label>
                                </Show>
                              )}
                            </For>
                            <button
                              {...stylex.attrs(common.button, common.danger)}
                              onClick={() => remove(id)}
                            >
                              Remove component
                            </button>
                          </div>
                        </Show>
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
          <Show when={p.review}>
            {(review) => (
              <div
                {...stylex.attrs(
                  common.notice,
                  review().status === "changes_requested" && common.warning,
                  styles.review,
                )}
                role="status"
              >
                <strong>
                  {review().status === "submitted"
                    ? "In review"
                    : review().status === "approved"
                      ? "Approved and published"
                      : "Changes requested"}
                </strong>
                <p>Submitted revision {review().content.page.revision}</p>
                <Show when={review().feedback}>
                  <p>{review().feedback}</p>
                </Show>
                <Show when={dirty() || review().content.page.revision !== draft().revision}>
                  <p>Draft changed. Save your changes, then submit again for review.</p>
                </Show>
              </div>
            )}
          </Show>
        </aside>
        <main {...stylex.attrs(styles.canvasWrap, !experience() && styles.hidden)}>
          <div {...stylex.attrs(styles.tools)}>
            <span>Live preview</span>
            <div>
              <button
                {...stylex.attrs(common.button, !mobile() && styles.activeTool)}
                aria-pressed={!mobile() ? "true" : "false"}
                onClick={() => setMobile(false)}
              >
                Desktop
              </button>
              <button
                {...stylex.attrs(common.button, mobile() && styles.activeTool)}
                aria-pressed={mobile() ? "true" : "false"}
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
      </div>
      <footer {...stylex.attrs(styles.footer)}>
        <span role="status">
          {busy()
            ? "Working…"
            : dirty()
              ? "Unsaved changes"
              : `Saved draft · Revision ${draft().revision}`}
        </span>
        <span>Draft changes are private until published.</span>
      </footer>
      <dialog
        ref={(element) => {
          publishDialog = element;
        }}
        {...stylex.attrs(common.dialog)}
        aria-labelledby="publish-heading"
        onCancel={(e) => {
          if (busy()) e.preventDefault();
        }}
      >
        <h2 id="publish-heading" {...stylex.attrs(common.heading)}>
          Publish {draft().title}?
        </h2>
        <p {...stylex.attrs(common.muted)}>Publish this page immediately to your live site.</p>
        <dl {...stylex.attrs(styles.metadata)}>
          <dt>URL</dt>
          <dd {...stylex.attrs(styles.metadataValue)}>{draft().slug}</dd>
          <dt>Template</dt>
          <dd {...stylex.attrs(styles.metadataValue)}>{template()?.name}</dd>
          <dt>Saved draft revision</dt>
          <dd {...stylex.attrs(styles.metadataValue)}>{draft().revision}</dd>
          <dt>Currently live revision</dt>
          <dd {...stylex.attrs(styles.metadataValue)}>
            {draft().published_revision ?? "Not published"}
          </dd>
        </dl>
        <Show when={dirty()}>
          <p {...stylex.attrs(common.muted)}>
            Unsaved changes will be saved as a new revision before publishing.
          </p>
        </Show>
        <div {...stylex.attrs(common.notice)}>
          <strong>This page only</strong>
          <p>
            {draft().title} at {draft().slug} will go live. Child pages will not be published.
          </p>
          <p>The page, template and component definitions are published together.</p>
        </div>
        <Show when={error()}>
          <p {...stylex.attrs(common.error)} role="alert">
            {error()}
          </p>
        </Show>
        <div {...stylex.attrs(styles.actions, styles.review)}>
          <button
            {...stylex.attrs(common.button)}
            disabled={!!busy()}
            onClick={() => publishDialog.close()}
          >
            Cancel
          </button>
          <button
            {...stylex.attrs(common.button, common.primary)}
            disabled={!!busy()}
            onClick={() => void publish()}
          >
            {busy() === "publish" ? "Publishing…" : dirty() ? "Save and publish" : "Publish page"}
          </button>
        </div>
      </dialog>
    </section>
  );
}
