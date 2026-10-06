import { createSignal, For, Show } from "solid-js";
import * as stylex from "@stylexjs/stylex";
import type { ComponentDef, Field, FieldKind, Region, RendererName, Template } from "../types";
import { common } from "../common.stylex";
const styles = stylex.create({
  definition: {
    padding: { default: "24px 28px", "@media (max-width: 720px)": "25px 16px" },
  },
  header: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    flexWrap: "wrap",
    gap: 16,
    marginBottom: 24,
  },
  heading: { fontSize: 26, fontWeight: 650, margin: 0 },
  layout: {
    display: "grid",
    gridTemplateColumns: {
      default: "minmax(0, 1fr) 300px",
      "@media (max-width: 1100px)": "1fr",
    },
    gap: 20,
  },
  preview: {
    backgroundColor: "#fff",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#dce2ea",
    borderRadius: 4,
    padding: 20,
    alignSelf: "start",
  },
  regionPreview: {
    backgroundColor: "#eff6ff",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#bfdbfe",
    borderRadius: 4,
    padding: 14,
    margin: "14px 0",
  },
  previewType: {
    display: "block",
    backgroundColor: "#fff",
    padding: 8,
    marginTop: 6,
    borderRadius: 4,
  },
  small: { color: "#64748b", fontSize: 12, lineHeight: 1.6 },
  panel: {
    backgroundColor: "#fff",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#dce2ea",
    borderRadius: 4,
    padding: 24,
  },
  row: {
    display: "grid",
    gridTemplateColumns: {
      default: "1fr 1fr",
      "@media (max-width: 1200px)": "1fr",
    },
    gap: 12,
  },
  footer: {
    display: "flex",
    justifyContent: "flex-end",
    gap: 8,
    marginTop: 22,
  },
  fieldset: {
    borderWidth: 0,
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "#dce2ea",
    marginTop: 25,
  },
  legend: { fontSize: 18, fontWeight: 650 },
  rule: {
    backgroundColor: "#f8fafc",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#dce2ea",
    borderRadius: 4,
    padding: 16,
    margin: "14px 0",
  },
  checks: { display: "flex", flexWrap: "wrap", gap: "4px 16px" },
  check: { display: "flex", alignItems: "center", gap: 6 },
  autoWidth: { width: "auto" },
  fieldGrid: {
    display: "grid",
    gridTemplateColumns: {
      default: "1fr 1fr 130px 90px auto",
      "@media (max-width: 1200px)": "1fr",
    },
    alignItems: "end",
    gap: 9,
  },
});
type Props = {
  kind: "templates" | "components";
  initialId?: string;
  templates: Template[];
  components: ComponentDef[];
  onSave: (value: Template | ComponentDef, isNew: boolean) => Promise<Template | ComponentDef>;
  onDirty: (dirty: boolean) => void;
};
const blankTemplate = (): Template => ({
  id: "",
  name: "",
  description: "",
  regions: [{ name: "main", allowed_components: [], max_components: 20 }],
});
const blankComponent = (): ComponentDef => ({
  id: "",
  name: "",
  description: "",
  renderer: "text",
  fields: [{ name: "title", label: "Title", kind: "text", required: true }],
});
export function DefinitionEditor(p: Props) {
  const initial = (p.kind === "templates" ? p.templates : p.components).find(
    (item) => item.id === p.initialId,
  );
  const [selected, setSelected] = createSignal(initial?.id ?? "");
  const [dirty, setDirty] = createSignal(false);
  const [draft, setDraft] = createSignal<Template | ComponentDef>(
    initial
      ? structuredClone(initial)
      : p.kind === "templates"
        ? blankTemplate()
        : blankComponent(),
  );
  const [error, setError] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const list = () => (p.kind === "templates" ? p.templates : p.components);
  function markDirty(value: boolean) {
    setDirty(value);
    p.onDirty(value);
  }
  function choose(id: string) {
    if (dirty() && !confirm("Discard your unsaved changes?")) return;
    setSelected(id);
    setDraft(
      structuredClone(
        list().find((x) => x.id === id) ??
          (p.kind === "templates" ? blankTemplate() : blankComponent()),
      ),
    );
    markDirty(false);
  }
  function patch(values: Partial<Template & ComponentDef>) {
    setDraft({ ...draft(), ...values } as Template | ComponentDef);
    markDirty(true);
  }
  function change(value: Template | ComponentDef) {
    setDraft(value);
    markDirty(true);
  }
  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (busy()) return;
    setBusy(true);
    setError("");
    try {
      const saved = await p.onSave(draft(), !selected());
      setSelected(saved.id);
      setDraft(structuredClone(saved));
      markDirty(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not save");
    } finally {
      setBusy(false);
    }
  }
  return (
    <section {...stylex.attrs(styles.definition)}>
      <header {...stylex.attrs(styles.header)}>
        <div>
          <p {...stylex.attrs(common.eyebrow)}>Site model</p>
          <h1 {...stylex.attrs(styles.heading)}>
            {selected() ? draft().name : p.kind === "templates" ? "New template" : "New component"}
          </h1>
          <p {...stylex.attrs(common.muted)}>
            {p.kind === "templates"
              ? "Define where components can go on a page."
              : "Define a renderer and its content fields."}
          </p>
        </div>
        <div {...stylex.attrs(styles.checks)}>
          <button
            {...stylex.attrs(common.button)}
            disabled={busy()}
            onClick={() => choose(selected())}
          >
            Cancel changes
          </button>
          <button {...stylex.attrs(common.button)} disabled={busy()} onClick={() => choose("")}>
            New {p.kind === "templates" ? "template" : "component"}
          </button>
          <button
            {...stylex.attrs(common.button, common.primary)}
            form="definition-form"
            disabled={busy()}
          >
            {busy() ? "Saving…" : p.kind === "templates" ? "Save template" : "Save component"}
          </button>
        </div>
      </header>
      <div {...stylex.attrs(styles.layout)}>
        <form id="definition-form" {...stylex.attrs(styles.panel)} onSubmit={submit} inert={busy()}>
          <div {...stylex.attrs(styles.row)}>
            <label {...stylex.attrs(common.label)}>
              Name
              <input
                {...stylex.attrs(common.control)}
                required
                value={draft().name}
                onInput={(e) => patch({ name: e.currentTarget.value })}
              />
            </label>
            <label {...stylex.attrs(common.label)}>
              Description
              <input
                {...stylex.attrs(common.control)}
                value={draft().description}
                onInput={(e) => patch({ description: e.currentTarget.value })}
              />
            </label>
          </div>
          <Show
            when={p.kind === "templates"}
            fallback={<ComponentFields value={draft() as ComponentDef} onChange={change} />}
          >
            {
              <TemplateRegions
                value={draft() as Template}
                components={p.components}
                onChange={change}
              />
            }
          </Show>
          {error() && (
            <p {...stylex.attrs(common.error)} role="alert">
              {error()}
            </p>
          )}
          <footer {...stylex.attrs(styles.footer)}>
            <span {...stylex.attrs(styles.small)} role="status">
              {busy()
                ? "Saving…"
                : dirty()
                  ? "Unsaved changes"
                  : selected()
                    ? "Saved"
                    : "New definition"}
            </span>
            <button {...stylex.attrs(common.button, common.primary)}>
              Save {selected() ? "changes" : p.kind === "templates" ? "template" : "component"}
            </button>
          </footer>
        </form>
        <aside
          {...stylex.attrs(styles.preview)}
          aria-label={p.kind === "templates" ? "Structure preview" : "Component summary"}
        >
          <Show
            when={p.kind === "templates"}
            fallback={
              <>
                <h3>Component fields</h3>
                <p {...stylex.attrs(styles.small)}>
                  Renderer: {(draft() as ComponentDef).renderer}
                </p>
                <For each={(draft() as ComponentDef).fields}>
                  {(field) => (
                    <p {...stylex.attrs(styles.previewType)}>
                      {field.label || field.name} · {field.kind}
                      {field.required ? " · Required" : ""}
                    </p>
                  )}
                </For>
                <p {...stylex.attrs(common.notice)}>
                  Templates decide which regions allow this component.
                </p>
              </>
            }
          >
            <h3>Structure preview</h3>
            <p {...stylex.attrs(styles.small)}>
              A region map, not the page design. These are allowed types, not default components.
            </p>
            <For each={(draft() as Template).regions}>
              {(region) => (
                <section {...stylex.attrs(styles.regionPreview)}>
                  <strong>{region.name || "Unnamed region"}</strong>
                  <p {...stylex.attrs(styles.small)}>Up to {region.max_components} components</p>
                  <For each={p.components.filter((c) => region.allowed_components.includes(c.id))}>
                    {(component) => (
                      <span {...stylex.attrs(styles.previewType)}>{component.name}</span>
                    )}
                  </For>
                  <Show when={!region.allowed_components.length}>
                    <p {...stylex.attrs(styles.small)}>No components allowed</p>
                  </Show>
                </section>
              )}
            </For>
            <div {...stylex.attrs(common.notice)}>
              <strong>Fields live on components</strong>
              <p>Edit component definitions to define their fields.</p>
            </div>
          </Show>
        </aside>
      </div>
    </section>
  );
}
function TemplateRegions(p: {
  value: Template;
  components: ComponentDef[];
  onChange: (v: Template) => void;
}) {
  function set(regions: Region[]) {
    p.onChange({ ...p.value, regions });
  }
  return (
    <fieldset {...stylex.attrs(styles.fieldset)}>
      <legend {...stylex.attrs(styles.legend)}>Regions</legend>
      <For each={p.value.regions.map((_, index) => index)}>
        {(index) => {
          const i = () => index;
          const region = () => p.value.regions[index];
          return (
            <div {...stylex.attrs(styles.rule)}>
              <div {...stylex.attrs(styles.row)}>
                <label {...stylex.attrs(common.label)}>
                  Region name
                  <input
                    {...stylex.attrs(common.control)}
                    required
                    value={region().name}
                    onInput={(e) =>
                      set(
                        p.value.regions.map((r, n) =>
                          n === i() ? { ...r, name: e.currentTarget.value } : r,
                        ),
                      )
                    }
                  />
                </label>
                <label {...stylex.attrs(common.label)}>
                  Maximum components
                  <input
                    {...stylex.attrs(common.control)}
                    required
                    min="1"
                    type="number"
                    value={region().max_components}
                    onInput={(e) =>
                      set(
                        p.value.regions.map((r, n) =>
                          n === i()
                            ? {
                                ...r,
                                max_components: Number(e.currentTarget.value),
                              }
                            : r,
                        ),
                      )
                    }
                  />
                </label>
              </div>
              <p {...stylex.attrs(styles.small)}>Editors can add only the selected components.</p>
              <span {...stylex.attrs(common.label)}>Allowed components</span>
              <div {...stylex.attrs(styles.checks)}>
                <For each={p.components}>
                  {(c) => (
                    <label {...stylex.attrs(common.label, styles.check)}>
                      <input
                        {...stylex.attrs(common.control, styles.autoWidth)}
                        type="checkbox"
                        checked={region().allowed_components.includes(c.id)}
                        onChange={(e) =>
                          set(
                            p.value.regions.map((r, n) =>
                              n === i()
                                ? {
                                    ...r,
                                    allowed_components: e.currentTarget.checked
                                      ? [...r.allowed_components, c.id]
                                      : r.allowed_components.filter((id) => id !== c.id),
                                  }
                                : r,
                            ),
                          )
                        }
                      />
                      {c.name}
                    </label>
                  )}
                </For>
              </div>
              <button
                type="button"
                {...stylex.attrs(common.button, common.danger)}
                onClick={() => set(p.value.regions.filter((_, n) => n !== i()))}
              >
                Remove region
              </button>
            </div>
          );
        }}
      </For>
      <p {...stylex.attrs(common.notice)}>
        An empty allowed-components list permits no components.
      </p>
      <button
        {...stylex.attrs(common.button)}
        type="button"
        onClick={() =>
          set([
            ...p.value.regions,
            {
              name: `region_${p.value.regions.length + 1}`,
              allowed_components: [],
              max_components: 10,
            },
          ])
        }
      >
        Add region
      </button>
    </fieldset>
  );
}
function ComponentFields(p: { value: ComponentDef; onChange: (v: ComponentDef) => void }) {
  function set(fields: Field[]) {
    p.onChange({ ...p.value, fields });
  }
  return (
    <>
      <label {...stylex.attrs(common.label)}>
        Renderer
        <select
          {...stylex.attrs(common.control)}
          value={p.value.renderer}
          onChange={(e) =>
            p.onChange({
              ...p.value,
              renderer: e.currentTarget.value as RendererName,
            })
          }
        >
          <For each={["hero", "text", "callout", "cards", "external"] as RendererName[]}>
            {(r) => <option>{r}</option>}
          </For>
        </select>
      </label>
      <Show when={p.value.renderer === "external"}>
        <p {...stylex.attrs(styles.small)}>
          The connected app renders component ID {p.value.id || "assigned on save"}. Editors see a
          content preview here. Allow this component in a template region to use it on pages.
        </p>
      </Show>
      <fieldset {...stylex.attrs(styles.fieldset)}>
        <legend {...stylex.attrs(styles.legend)}>Fields</legend>
        <For each={p.value.fields.map((_, index) => index)}>
          {(index) => {
            const i = () => index;
            const field = () => p.value.fields[index];
            return (
              <div {...stylex.attrs(styles.rule, styles.fieldGrid)}>
                <label {...stylex.attrs(common.label)}>
                  Key
                  <input
                    {...stylex.attrs(common.control)}
                    required
                    pattern="[a-z][a-z0-9_]*"
                    value={field().name}
                    onInput={(e) =>
                      set(
                        p.value.fields.map((f, n) =>
                          n === i() ? { ...f, name: e.currentTarget.value } : f,
                        ),
                      )
                    }
                  />
                </label>
                <label {...stylex.attrs(common.label)}>
                  Label
                  <input
                    {...stylex.attrs(common.control)}
                    required
                    value={field().label}
                    onInput={(e) =>
                      set(
                        p.value.fields.map((f, n) =>
                          n === i() ? { ...f, label: e.currentTarget.value } : f,
                        ),
                      )
                    }
                  />
                </label>
                <label {...stylex.attrs(common.label)}>
                  Type
                  <select
                    {...stylex.attrs(common.control)}
                    value={field().kind}
                    onChange={(e) =>
                      set(
                        p.value.fields.map((f, n) =>
                          n === i() ? { ...f, kind: e.currentTarget.value as FieldKind } : f,
                        ),
                      )
                    }
                  >
                    <option>text</option>
                    <option>textarea</option>
                    <option>url</option>
                  </select>
                </label>
                <label {...stylex.attrs(common.label, styles.check)}>
                  <input
                    {...stylex.attrs(common.control, styles.autoWidth)}
                    type="checkbox"
                    checked={field().required}
                    onChange={(e) =>
                      set(
                        p.value.fields.map((f, n) =>
                          n === i() ? { ...f, required: e.currentTarget.checked } : f,
                        ),
                      )
                    }
                  />{" "}
                  Required
                </label>
                <button
                  type="button"
                  {...stylex.attrs(common.button, common.danger)}
                  onClick={() => set(p.value.fields.filter((_, n) => n !== i()))}
                >
                  Remove
                </button>
              </div>
            );
          }}
        </For>
        <button
          {...stylex.attrs(common.button)}
          type="button"
          onClick={() =>
            set([
              ...p.value.fields,
              { name: "field", label: "Field", kind: "text", required: false },
            ])
          }
        >
          Add field
        </button>
      </fieldset>
    </>
  );
}
