import { createSignal, For, Show } from "solid-js";
import * as stylex from "@stylexjs/stylex";
import type { ComponentDef, Field, FieldKind, Region, RendererName, Template } from "../types";
import { common } from "../common.stylex";
const styles = stylex.create({
  definition: {
    padding: { default: "42px 48px", "@media (max-width: 720px)": "25px 16px" },
    maxWidth: 1200,
    margin: "auto",
  },
  header: {
    display: "flex",
    justifyContent: "space-between",
    marginBottom: 30,
  },
  heading: { font: "600 42px Georgia, serif", margin: 0 },
  layout: {
    display: "grid",
    gridTemplateColumns: {
      default: "240px 1fr",
      "@media (max-width: 720px)": "1fr",
    },
    gap: 20,
  },
  list: { display: "grid", alignContent: "start", gap: 7 },
  listButton: { textAlign: "left", display: "grid" },
  active: { backgroundColor: "#eee9fa" },
  small: { color: "#77716a" },
  panel: {
    backgroundColor: "#fffdf8",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#ddd6cb",
    borderRadius: 12,
    padding: 24,
  },
  row: {
    display: "grid",
    gridTemplateColumns: {
      default: "1fr 1fr",
      "@media (max-width: 720px)": "1fr",
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
    borderTopColor: "#ddd6cb",
    marginTop: 25,
  },
  legend: { font: "600 19px Georgia, serif" },
  rule: { backgroundColor: "#f5f1ea", borderRadius: 9, padding: 13, margin: 9 },
  checks: { display: "flex", flexWrap: "wrap", gap: "4px 16px" },
  check: { display: "flex", alignItems: "center", gap: 6 },
  autoWidth: { width: "auto" },
  fieldGrid: {
    display: "grid",
    gridTemplateColumns: {
      default: "1fr 1fr 130px 90px auto",
      "@media (max-width: 720px)": "1fr",
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
  onSave: (value: Template | ComponentDef, isNew: boolean) => Promise<void>;
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
    setError("");
    try {
      await p.onSave(draft(), !selected());
      setSelected("");
      setDraft(p.kind === "templates" ? blankTemplate() : blankComponent());
      markDirty(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not save");
    }
  }
  return (
    <section {...stylex.attrs(styles.definition)}>
      <header {...stylex.attrs(styles.header)}>
        <div>
          <p {...stylex.attrs(common.eyebrow)}>Site model</p>
          <h1 {...stylex.attrs(styles.heading)}>
            {p.kind === "templates" ? "Templates" : "Components"}
          </h1>
        </div>
        <button {...stylex.attrs(common.button)} onClick={() => choose("")}>
          New {p.kind === "templates" ? "template" : "component"}
        </button>
      </header>
      <div {...stylex.attrs(styles.layout)}>
        <nav {...stylex.attrs(styles.list)}>
          <For each={list()}>
            {(item) => (
              <button
                {...stylex.attrs(
                  common.button,
                  styles.listButton,
                  selected() === item.id && styles.active,
                )}
                onClick={() => choose(item.id)}
              >
                <strong>{item.name}</strong>
                <small {...stylex.attrs(styles.small)}>
                  {item.description || "No description"}
                </small>
              </button>
            )}
          </For>
        </nav>
        <form {...stylex.attrs(styles.panel)} onSubmit={submit}>
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
            <button {...stylex.attrs(common.button, common.primary)}>
              Save {selected() ? "changes" : p.kind === "templates" ? "template" : "component"}
            </button>
          </footer>
        </form>
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
                  Maximum
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
          <For each={["hero", "text", "callout", "cards"] as RendererName[]}>
            {(r) => <option>{r}</option>}
          </For>
        </select>
      </label>
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
