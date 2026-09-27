import { createSignal, For, Show } from "solid-js";
import type {
  ComponentDef,
  Field,
  FieldKind,
  Region,
  RendererName,
  Template,
} from "../types";
type Props = {
  kind: "templates" | "components";
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
  const [selected, setSelected] = createSignal("");
  const [dirty, setDirty] = createSignal(false);
  const [draft, setDraft] = createSignal<Template | ComponentDef>(
    p.kind === "templates" ? blankTemplate() : blankComponent(),
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
    <section class="definition">
      <header>
        <div>
          <p class="eyebrow">Site model</p>
          <h1>{p.kind === "templates" ? "Templates" : "Components"}</h1>
        </div>
        <button onClick={() => choose("")}>
          New {p.kind === "templates" ? "template" : "component"}
        </button>
      </header>
      <div class="definition-layout">
        <nav class="definition-list">
          <For each={list()}>
            {(item) => (
              <button
                class={selected() === item.id ? "active" : ""}
                onClick={() => choose(item.id)}
              >
                <strong>{item.name}</strong>
                <small>{item.description || "No description"}</small>
              </button>
            )}
          </For>
        </nav>
        <form class="form-panel" onSubmit={submit}>
          <div class="field-row">
            <label>
              Name
              <input
                required
                value={draft().name}
                onInput={(e) => patch({ name: e.currentTarget.value })}
              />
            </label>
            <label>
              Description
              <input
                value={draft().description}
                onInput={(e) => patch({ description: e.currentTarget.value })}
              />
            </label>
          </div>
          <Show
            when={p.kind === "templates"}
            fallback={
              <ComponentFields
                value={draft() as ComponentDef}
                onChange={change}
              />
            }
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
            <p class="error" role="alert">
              {error()}
            </p>
          )}
          <footer>
            <button class="primary">
              Save{" "}
              {selected()
                ? "changes"
                : p.kind === "templates"
                  ? "template"
                  : "component"}
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
    <fieldset>
      <legend>Regions</legend>
      <For each={p.value.regions}>
        {(region, i) => (
          <div class="rule-card">
            <div class="field-row">
              <label>
                Region name
                <input
                  required
                  value={region.name}
                  onInput={(e) =>
                    set(
                      p.value.regions.map((r, n) =>
                        n === i() ? { ...r, name: e.currentTarget.value } : r,
                      ),
                    )
                  }
                />
              </label>
              <label>
                Maximum
                <input
                  required
                  min="1"
                  type="number"
                  value={region.max_components}
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
            <span class="label">Allowed components</span>
            <div class="checks">
              <For each={p.components}>
                {(c) => (
                  <label>
                    <input
                      type="checkbox"
                      checked={region.allowed_components.includes(c.id)}
                      onChange={(e) =>
                        set(
                          p.value.regions.map((r, n) =>
                            n === i()
                              ? {
                                  ...r,
                                  allowed_components: e.currentTarget.checked
                                    ? [...r.allowed_components, c.id]
                                    : r.allowed_components.filter(
                                        (id) => id !== c.id,
                                      ),
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
              class="danger-link"
              onClick={() => set(p.value.regions.filter((_, n) => n !== i()))}
            >
              Remove region
            </button>
          </div>
        )}
      </For>
      <button
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
function ComponentFields(p: {
  value: ComponentDef;
  onChange: (v: ComponentDef) => void;
}) {
  function set(fields: Field[]) {
    p.onChange({ ...p.value, fields });
  }
  return (
    <>
      <label>
        Renderer
        <select
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
      <fieldset>
        <legend>Fields</legend>
        <For each={p.value.fields}>
          {(field, i) => (
            <div class="rule-card field-grid">
              <label>
                Key
                <input
                  required
                  pattern="[a-z][a-z0-9_]*"
                  value={field.name}
                  onInput={(e) =>
                    set(
                      p.value.fields.map((f, n) =>
                        n === i() ? { ...f, name: e.currentTarget.value } : f,
                      ),
                    )
                  }
                />
              </label>
              <label>
                Label
                <input
                  required
                  value={field.label}
                  onInput={(e) =>
                    set(
                      p.value.fields.map((f, n) =>
                        n === i() ? { ...f, label: e.currentTarget.value } : f,
                      ),
                    )
                  }
                />
              </label>
              <label>
                Type
                <select
                  value={field.kind}
                  onChange={(e) =>
                    set(
                      p.value.fields.map((f, n) =>
                        n === i()
                          ? { ...f, kind: e.currentTarget.value as FieldKind }
                          : f,
                      ),
                    )
                  }
                >
                  <option>text</option>
                  <option>textarea</option>
                  <option>url</option>
                </select>
              </label>
              <label class="check">
                <input
                  type="checkbox"
                  checked={field.required}
                  onChange={(e) =>
                    set(
                      p.value.fields.map((f, n) =>
                        n === i()
                          ? { ...f, required: e.currentTarget.checked }
                          : f,
                      ),
                    )
                  }
                />{" "}
                Required
              </label>
              <button
                type="button"
                class="danger-link"
                onClick={() => set(p.value.fields.filter((_, n) => n !== i()))}
              >
                Remove
              </button>
            </div>
          )}
        </For>
        <button
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
