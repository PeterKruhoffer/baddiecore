import { createSignal } from "solid-js";
import * as stylex from "@stylexjs/stylex";
import { api } from "../lib/api";
import { common } from "../common.stylex";
const styles = stylex.create({
  login: {
    height: "100vh",
    display: "grid",
    placeItems: "center",
    backgroundColor: "#fffdf8",
    backgroundImage: "radial-gradient(circle at 70% 20%, #e6dcf7, transparent 35%)",
  },
  card: {
    width: "min(390px, 90vw)",
    padding: 38,
    backgroundColor: "#fffdfbcc",
    borderRadius: 16,
  },
  heading: { font: "600 40px Georgia, serif" },
  full: { width: "100%" },
});
export function Login(p: { onSuccess: () => void }) {
  const [password, setPassword] = createSignal("");
  const [error, setError] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  async function submit(e: SubmitEvent) {
    e.preventDefault();
    setBusy(true);
    setError("");
    try {
      await api.login(password());
      p.onSuccess();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not sign in");
    } finally {
      setBusy(false);
    }
  }
  return (
    <main {...stylex.attrs(styles.login)}>
      <form {...stylex.attrs(styles.card)} onSubmit={submit}>
        <span {...stylex.attrs(common.brandMark)}>B</span>
        <p {...stylex.attrs(common.eyebrow)}>Baddiecore</p>
        <h1 {...stylex.attrs(styles.heading)}>Welcome back.</h1>
        <p {...stylex.attrs(common.muted)}>Sign in to shape your site.</p>
        <label {...stylex.attrs(common.label)}>
          Password
          <input
            {...stylex.attrs(common.control)}
            autofocus
            type="password"
            required
            value={password()}
            onInput={(e) => setPassword(e.currentTarget.value)}
          />
        </label>
        {error() && (
          <p {...stylex.attrs(common.error)} role="alert">
            {error()}
          </p>
        )}
        <button {...stylex.attrs(common.button, common.primary, styles.full)} disabled={busy()}>
          {busy() ? "Signing in…" : "Enter workspace"}
        </button>
      </form>
    </main>
  );
}
