import { createSignal, For, Show } from "solid-js";
import * as stylex from "@stylexjs/stylex";
import { api, findMedia, mediaUrl } from "../lib/api";
import { common } from "../common.stylex";
import type { Field, Media } from "../types";

const styles = stylex.create({
  page: {
    padding: { default: "28px 32px", "@media (max-width: 720px)": "25px 16px" },
    maxWidth: 1200,
    margin: "auto",
    minHeight: "100%",
  },
  dropping: { outlineWidth: 2, outlineStyle: "dashed", outlineColor: "#2563eb", outlineOffset: -8 },
  header: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "flex-start",
    gap: 16,
    marginBottom: 24,
  },
  grid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(180px, 1fr))",
    gap: 14,
    margin: 0,
    padding: 0,
    listStyleType: "none",
  },
  card: {
    backgroundColor: "#fff",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#dce2ea",
    borderRadius: 4,
    padding: 10,
    display: "grid",
    gap: 6,
    minWidth: 0,
  },
  thumb: {
    width: "100%",
    aspectRatio: "4 / 3",
    objectFit: "contain",
    backgroundColor: "#f1f5f9",
    borderRadius: 4,
    display: "block",
  },
  name: { fontSize: 13, fontWeight: 600, overflowWrap: "anywhere" },
  meta: { fontSize: 12 },
  fileInput: { display: "none" },
  lines: { whiteSpace: "pre-line" },
  choice: {
    textAlign: "left",
    display: "grid",
    gap: 6,
    width: "100%",
    padding: 8,
    borderColor: { default: "#dce2ea", ":hover": "#2563eb" },
  },
  chosen: { borderColor: "#2563eb", backgroundColor: "#eff6ff" },
  wideDialog: { width: "min(820px, calc(100vw - 32px))" },
  pickerGrid: { maxHeight: "55vh", overflowY: "auto", marginBlock: 16 },
  selected: { display: "flex", gap: 12, alignItems: "center" },
  selectedThumb: { width: 96, flexShrink: 0 },
  actions: { display: "flex", gap: 8, flexWrap: "wrap", justifyContent: "flex-end" },
});

const kilobytes = (size: number) =>
  size < 1024 * 1024 ? `${Math.ceil(size / 1024)} KB` : `${(size / 1024 / 1024).toFixed(1)} MB`;

/** Uploads each file in its own request and reports every failure by filename. */
async function uploadAll(files: File[]) {
  const uploaded: Media[] = [];
  const errors: string[] = [];
  for (const file of files) {
    try {
      uploaded.push(await api.uploadMedia(file));
    } catch (e) {
      errors.push(`${file.name}: ${e instanceof Error ? e.message : "upload failed"}`);
    }
  }
  return { uploaded, error: errors.join("\n") };
}

function UploadButton(p: { busy: boolean; onFiles: (files: File[]) => void }) {
  let input!: HTMLInputElement;
  return (
    <>
      <input
        ref={(element) => {
          input = element;
        }}
        {...stylex.attrs(styles.fileInput)}
        type="file"
        multiple
        accept="image/png,image/jpeg,image/gif,image/webp,image/avif"
        aria-label="Image files"
        onChange={(e) => {
          p.onFiles([...(e.currentTarget.files ?? [])]);
          e.currentTarget.value = "";
        }}
      />
      <button
        {...stylex.attrs(common.button, common.primary)}
        type="button"
        disabled={p.busy}
        onClick={() => input.click()}
      >
        {p.busy ? "Uploading…" : "Upload images"}
      </button>
    </>
  );
}

/** The library screen: every member uploads and describes media; admins delete it. */
export function MediaLibrary(p: {
  media: Media[];
  isAdmin: boolean;
  onRefresh: () => Promise<void>;
}) {
  const [busy, setBusy] = createSignal(false);
  const [dropping, setDropping] = createSignal(false);
  const [error, setError] = createSignal("");
  async function upload(files: File[]) {
    if (!files.length || busy()) return;
    setBusy(true);
    setError("");
    const result = await uploadAll(files);
    setError(result.error);
    await p.onRefresh();
    setBusy(false);
  }
  async function act(fn: () => Promise<unknown>) {
    setError("");
    try {
      await fn();
      await p.onRefresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Request failed");
    }
  }
  return (
    <section
      {...stylex.attrs(styles.page, dropping() && styles.dropping)}
      onDragOver={(e) => {
        e.preventDefault();
        setDropping(true);
      }}
      onDragLeave={() => setDropping(false)}
      onDrop={(e) => {
        e.preventDefault();
        setDropping(false);
        void upload([...(e.dataTransfer?.files ?? [])]);
      }}
    >
      <header {...stylex.attrs(styles.header)}>
        <div>
          <p {...stylex.attrs(common.eyebrow)}>Content</p>
          <h1 {...stylex.attrs(common.heading)}>Media</h1>
          <p {...stylex.attrs(common.muted)}>
            Upload PNG, JPEG, GIF, WebP or AVIF images up to 10 MB, or drop them here.
          </p>
        </div>
        <UploadButton busy={busy()} onFiles={(files) => void upload(files)} />
      </header>
      <Show when={error()}>
        <p {...stylex.attrs(common.error, styles.lines)} role="alert">
          {error()}
        </p>
      </Show>
      <Show when={p.media.length} fallback={<p {...stylex.attrs(common.muted)}>No media yet.</p>}>
        <ul {...stylex.attrs(styles.grid)} aria-label="Media library">
          <For each={p.media}>
            {(item) => (
              <li {...stylex.attrs(styles.card)}>
                <img {...stylex.attrs(styles.thumb)} src={mediaUrl(item.id)} alt={item.alt} />
                <span {...stylex.attrs(styles.name)}>{item.filename}</span>
                <span {...stylex.attrs(common.muted, styles.meta)}>{kilobytes(item.size)}</span>
                <label {...stylex.attrs(common.label)}>
                  Alt text
                  <input
                    {...stylex.attrs(common.control)}
                    value={item.alt}
                    placeholder="Describe the image"
                    onChange={(e) => {
                      const alt = e.currentTarget.value;
                      void act(() => api.updateMedia(item.id, alt));
                    }}
                  />
                </label>
                <Show when={p.isAdmin}>
                  <button
                    {...stylex.attrs(common.button, common.danger)}
                    type="button"
                    onClick={() => {
                      if (confirm(`Delete ${item.filename}?`))
                        void act(() => api.deleteMedia(item.id));
                    }}
                  >
                    Delete
                  </button>
                </Show>
              </li>
            )}
          </For>
        </ul>
      </Show>
    </section>
  );
}

/** A media field: shows the chosen image and opens a picker that can also upload. */
export function MediaField(p: {
  field: Field;
  value: string;
  media: Media[];
  onChange: (value: string) => void;
  onUploaded: () => Promise<void>;
}) {
  let dialog!: HTMLDialogElement;
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");
  const selected = () => findMedia(p.media, p.value);
  const headingId = `media-picker-${p.field.name}`;
  function choose(item: Media) {
    p.onChange(item.id);
    dialog.close();
  }
  async function upload(files: File[]) {
    if (!files.length || busy()) return;
    setBusy(true);
    setError("");
    const result = await uploadAll(files);
    setError(result.error);
    await p.onUploaded();
    setBusy(false);
    if (result.uploaded.length === 1 && !result.error) choose(result.uploaded[0]);
  }
  return (
    <div {...stylex.attrs(common.label)} role="group" aria-label={p.field.label}>
      <span>
        {p.field.label}
        {p.field.required ? " *" : ""}
      </span>
      <Show
        when={selected()}
        fallback={<span {...stylex.attrs(common.muted)}>No image selected</span>}
      >
        {(item) => (
          <div {...stylex.attrs(styles.selected)}>
            <img
              {...stylex.attrs(styles.thumb, styles.selectedThumb)}
              src={mediaUrl(item().id)}
              alt={item().alt}
            />
            <span {...stylex.attrs(styles.name)}>{item().filename}</span>
          </div>
        )}
      </Show>
      <div {...stylex.attrs(styles.actions)}>
        <button
          {...stylex.attrs(common.button)}
          type="button"
          onClick={() => {
            setError("");
            dialog.showModal();
          }}
        >
          Choose media
        </button>
        <Show when={p.value}>
          <button
            {...stylex.attrs(common.button, common.danger)}
            type="button"
            onClick={() => p.onChange("")}
          >
            Remove
          </button>
        </Show>
      </div>
      <dialog
        ref={(element) => {
          dialog = element;
        }}
        {...stylex.attrs(common.dialog, styles.wideDialog)}
        aria-labelledby={headingId}
      >
        <div {...stylex.attrs(styles.header)}>
          <h2 id={headingId} {...stylex.attrs(common.heading)}>
            Choose {p.field.label.toLowerCase()}
          </h2>
          <UploadButton busy={busy()} onFiles={(files) => void upload(files)} />
        </div>
        <Show when={error()}>
          <p {...stylex.attrs(common.error, styles.lines)} role="alert">
            {error()}
          </p>
        </Show>
        <Show
          when={p.media.length}
          fallback={<p {...stylex.attrs(common.muted)}>No media yet. Upload an image.</p>}
        >
          <ul {...stylex.attrs(styles.grid, styles.pickerGrid)} aria-label="Media library">
            <For each={p.media}>
              {(item) => (
                <li>
                  <button
                    {...stylex.attrs(
                      common.button,
                      styles.choice,
                      item.id === p.value && styles.chosen,
                    )}
                    type="button"
                    aria-pressed={item.id === p.value ? "true" : "false"}
                    onClick={() => choose(item)}
                  >
                    <img {...stylex.attrs(styles.thumb)} src={mediaUrl(item.id)} alt="" />
                    <span {...stylex.attrs(styles.name)}>{item.filename}</span>
                  </button>
                </li>
              )}
            </For>
          </ul>
        </Show>
        <div {...stylex.attrs(styles.actions)}>
          <button {...stylex.attrs(common.button)} type="button" onClick={() => dialog.close()}>
            Cancel
          </button>
        </div>
      </dialog>
    </div>
  );
}
