import { createSignal } from "solid-js";
import * as stylex from "@stylexjs/stylex";
import { api } from "../lib/api";
import { common } from "../common.stylex";

const styles = stylex.create({
  layout: { padding: 28, maxWidth: 560, margin: "auto" },
  status: { color: "#166534" },
});

export function AccountEditor(p: { id: string }) {
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");
  const [done, setDone] = createSignal(false);
  async function submit(e: SubmitEvent) {
    e.preventDefault();
    const form = e.currentTarget as HTMLFormElement;
    const data = new FormData(form);
    const password = String(data.get("password"));
    setError("");
    setDone(false);
    if (password !== String(data.get("confirm"))) {
      setError("The new passwords do not match.");
      return;
    }
    setBusy(true);
    try {
      await api.changePassword(String(data.get("current")), password);
      form.reset();
      setDone(true);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not change password");
    } finally {
      setBusy(false);
    }
  }
  return (
    <section {...stylex.attrs(styles.layout)}>
      <h1>Account</h1>
      <p>
        Signed in as <strong>{p.id}</strong>. Changing your password signs you out on other devices.
      </p>
      <form onSubmit={submit}>
        <input type="hidden" name="username" autocomplete="username" value={p.id} />
        <label {...stylex.attrs(common.label)}>
          Current password
          <input
            {...stylex.attrs(common.control)}
            name="current"
            type="password"
            autocomplete="current-password"
            required
          />
        </label>
        <label {...stylex.attrs(common.label)}>
          New password
          <input
            {...stylex.attrs(common.control)}
            name="password"
            type="password"
            autocomplete="new-password"
            minlength={8}
            required
          />
        </label>
        <label {...stylex.attrs(common.label)}>
          Repeat new password
          <input
            {...stylex.attrs(common.control)}
            name="confirm"
            type="password"
            autocomplete="new-password"
            minlength={8}
            required
          />
        </label>
        {error() && (
          <p {...stylex.attrs(common.error)} role="alert">
            {error()}
          </p>
        )}
        {done() && (
          <p {...stylex.attrs(styles.status)} role="status">
            Password changed.
          </p>
        )}
        <button {...stylex.attrs(common.button, common.primary)} disabled={busy()}>
          {busy() ? "Changing…" : "Change password"}
        </button>
      </form>
    </section>
  );
}
