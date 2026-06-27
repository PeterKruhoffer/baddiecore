import { createEffect, createMemo, createSignal, For, on, Show } from "solid-js";
import { ApiError, updateRenderingDatasource } from "./cmsApi";
import type { Rendering, RenderingField } from "./cmsTypes";

type SaveState = "idle" | "saving" | "saved" | "error" | "conflict";

function FieldControl(props: {
  field: RenderingField;
  value: string;
  disabled: boolean;
  onInput: (value: string) => void;
}) {
  const handleInput = (event: InputEvent & { currentTarget: HTMLInputElement | HTMLTextAreaElement }) =>
    props.onInput(event.currentTarget.value);

  return (
    <label>
      <span>{props.field.label}{props.field.required ? " *" : ""}</span>
      <Show when={props.field.type === "textarea"} fallback={
        <input
          value={props.value}
          required={props.field.required}
          disabled={props.disabled}
          onInput={handleInput}
        />
      }>
        <textarea
          rows="5"
          value={props.value}
          required={props.field.required}
          disabled={props.disabled}
          onInput={handleInput}
        />
      </Show>
    </label>
  );
}

function SaveActions(props: {
  state: SaveState;
  isDirty: boolean;
  onSave: () => void;
  onRefresh: () => void;
}) {
  return (
    <div class="save-actions">
      <button
        type="button"
        class="save-button"
        disabled={!props.isDirty || props.state === "saving" || props.state === "conflict"}
        onClick={props.onSave}
      >
        {props.state === "saving" ? "Saving…" : "Save changes"}
      </button>
      <Show when={props.state === "conflict"}>
        <button type="button" class="refresh-button" onClick={props.onRefresh}>Refresh layout</button>
      </Show>
    </div>
  );
}

function SaveFeedback(props: { state: SaveState; message: string; isDirty: boolean }) {
  return (
    <>
      <Show when={props.message}>
        <p classList={{
          "save-feedback": true,
          error: props.state === "error" || props.state === "conflict",
          success: props.state === "saved"
        }} role="status">
          {props.message}
        </p>
      </Show>
      <Show when={props.isDirty && props.state === "idle"}>
        <p class="dirty-note">Unsaved changes</p>
      </Show>
    </>
  );
}

function FieldEditor(props: { rendering: Rendering; refreshLayout: () => Promise<void> }) {
  const [fieldValues, setFieldValues] = createSignal<Record<string, string>>({});
  const [saveState, setSaveState] = createSignal<SaveState>("idle");
  const [saveMessage, setSaveMessage] = createSignal("");
  const isDirty = createMemo(() =>
    props.rendering.fields.some((field) => (fieldValues()[field.name] ?? "") !== field.value)
  );

  createEffect(on(() => props.rendering, (rendering) => {
    setFieldValues(Object.fromEntries(rendering.fields.map((field) => [field.name, field.value])));
    setSaveState("idle");
    setSaveMessage("");
  }));

  const updateField = (name: string, value: string) => {
    setFieldValues((current) => ({ ...current, [name]: value }));
    setSaveState("idle");
    setSaveMessage("");
  };

  const saveFields = async () => {
    if (!isDirty() || saveState() === "saving") return;

    setSaveState("saving");
    setSaveMessage("Saving changes…");
    try {
      await updateRenderingDatasource(props.rendering, fieldValues());
      await props.refreshLayout();
      setSaveState("saved");
      setSaveMessage("Changes saved. Preview refreshed.");
    } catch (error) {
      if (error instanceof ApiError && error.status === 409) {
        setSaveState("conflict");
        setSaveMessage(`This datasource changed${error.currentVersion ? ` (now version ${error.currentVersion})` : ""}. Refresh before saving again.`);
      } else {
        setSaveState("error");
        setSaveMessage(error instanceof Error ? error.message : "The changes could not be saved.");
      }
    }
  };

  const refreshConflict = async () => {
    await props.refreshLayout();
    setSaveState("idle");
    setSaveMessage("");
  };

  return (
    <>
      <small class="kicker">Selected rendering</small>
      <h2>{props.rendering.name}</h2>
      <code>{props.rendering.componentKey}</code>
      <small class="datasource-version">Datasource version {props.rendering.datasourceVersion}</small>
      <div class="fields">
        <For each={props.rendering.fields}>
          {(field) => (
            <FieldControl
              field={field}
              value={fieldValues()[field.name] ?? ""}
              disabled={saveState() === "saving"}
              onInput={(value) => updateField(field.name, value)}
            />
          )}
        </For>
      </div>
      <SaveActions
        state={saveState()}
        isDirty={isDirty()}
        onSave={() => void saveFields()}
        onRefresh={() => void refreshConflict()}
      />
      <SaveFeedback state={saveState()} message={saveMessage()} isDirty={isDirty()} />
    </>
  );
}

export function FieldPanel(props: { rendering?: Rendering; refreshLayout: () => Promise<void> }) {
  return (
    <aside class="field-panel">
      <Show
        when={props.rendering}
        fallback={<p class="notice">Select a rendering in the preview to inspect its fields.</p>}
      >
        {(rendering) => <FieldEditor rendering={rendering()} refreshLayout={props.refreshLayout} />}
      </Show>
    </aside>
  );
}
