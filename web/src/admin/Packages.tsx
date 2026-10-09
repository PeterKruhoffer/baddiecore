import { createSignal, Show } from "solid-js";
import * as stylex from "@stylexjs/stylex";
import { api } from "../lib/api";
import { common } from "../common.stylex";
import type { PackageResult } from "../types";

const styles = stylex.create({
  compact: { padding: "5px 8px", whiteSpace: "nowrap" },
  file: { marginTop: 18 },
  summary: { margin: "14px 0 0", paddingLeft: 18, lineHeight: 1.7 },
  paths: { fontFamily: "monospace", overflowWrap: "anywhere" },
  actions: { display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 22 },
});

/** Download the page at `path` and all pages below it as a zip package. */
export function PackageExport(p: { path: string }) {
  const [busy, setBusy] = createSignal(false);
  async function download() {
    setBusy(true);
    try {
      const { blob, filename } = await api.exportPackage(p.path);
      const link = document.createElement("a");
      link.href = URL.createObjectURL(blob);
      link.download = filename;
      link.click();
      setTimeout(() => URL.revokeObjectURL(link.href));
    } catch (e) {
      alert(e instanceof Error ? e.message : "Could not export package");
    } finally {
      setBusy(false);
    }
  }
  return (
    <button
      {...stylex.attrs(common.button, styles.compact)}
      title={`Download ${p.path} and its child pages as a zip package`}
      aria-label={`Export ${p.path}`}
      disabled={busy()}
      onClick={() => void download()}
    >
      {busy() ? "Exporting…" : "Export"}
    </button>
  );
}

/** Upload a zip package and install its pages as drafts. */
export function PackageInstall(p: { onInstalled: () => Promise<void> }) {
  let dialog!: HTMLDialogElement;
  const [file, setFile] = createSignal<File>();
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");
  const [result, setResult] = createSignal<PackageResult>();
  function open() {
    setFile();
    setError("");
    setResult();
    dialog.showModal();
  }
  async function install() {
    const selected = file();
    if (!selected || busy()) return;
    setBusy(true);
    setError("");
    try {
      setResult(await api.installPackage(selected));
      await p.onInstalled();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not install package");
    } finally {
      setBusy(false);
    }
  }
  const pages = (label: string, paths: string[]) =>
    paths.length > 0 && (
      <li>
        {label} {paths.length}: <span {...stylex.attrs(styles.paths)}>{paths.join(", ")}</span>
      </li>
    );
  return (
    <>
      <button {...stylex.attrs(common.button)} onClick={open}>
        Import package
      </button>
      <dialog
        ref={(element) => {
          dialog = element;
        }}
        {...stylex.attrs(common.dialog)}
        aria-labelledby="package-heading"
        onCancel={(e) => {
          if (busy()) e.preventDefault();
        }}
      >
        <h2 id="package-heading" {...stylex.attrs(common.heading)}>
          Import a content package
        </h2>
        <Show
          when={result()}
          fallback={
            <>
              <p {...stylex.attrs(common.muted)}>
                Install pages exported from another Baddiecore site.
              </p>
              <div {...stylex.attrs(common.notice)}>
                Pages are installed as drafts; nothing goes live until you publish. Pages that
                already exist here are updated. Missing templates and components are added, and
                existing ones are kept as they are.
              </div>
              <label {...stylex.attrs(common.label, styles.file)}>
                Package (.zip)
                <input
                  {...stylex.attrs(common.control)}
                  type="file"
                  accept=".zip,application/zip"
                  onChange={(e) => setFile(e.currentTarget.files?.[0])}
                />
              </label>
            </>
          }
        >
          {(installed) => (
            <div role="status">
              <p>Package installed as drafts. Review the pages, then publish them.</p>
              <ul {...stylex.attrs(styles.summary)}>
                {pages("Created", installed().created)}
                {pages("Updated", installed().updated)}
                {pages("Unchanged", installed().unchanged)}
                <Show when={installed().templates_added + installed().components_added}>
                  <li>
                    Added {installed().templates_added} templates and {installed().components_added}{" "}
                    components
                  </li>
                </Show>
              </ul>
            </div>
          )}
        </Show>
        <Show when={error()}>
          <p {...stylex.attrs(common.error)} role="alert">
            {error()}
          </p>
        </Show>
        <div {...stylex.attrs(styles.actions)}>
          <Show
            when={!result()}
            fallback={
              <button
                {...stylex.attrs(common.button, common.primary)}
                onClick={() => dialog.close()}
              >
                Done
              </button>
            }
          >
            <button
              {...stylex.attrs(common.button)}
              disabled={busy()}
              onClick={() => dialog.close()}
            >
              Cancel
            </button>
            <button
              {...stylex.attrs(common.button, common.primary)}
              disabled={busy() || !file()}
              onClick={() => void install()}
            >
              {busy() ? "Installing…" : "Install package"}
            </button>
          </Show>
        </div>
      </dialog>
    </>
  );
}
