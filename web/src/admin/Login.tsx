import { createSignal, Show } from "solid-js";
import * as stylex from "@stylexjs/stylex";
import { api } from "../lib/api";
import { createRequest } from "../lib/resource";
import { common } from "../common.stylex";
const styles = stylex.create({
  login: {
    height: "100vh",
    display: "grid",
    placeItems: "center",
    backgroundColor: "#f5f7fa",
  },
  card: {
    width: "min(390px, 90vw)",
    padding: 38,
    backgroundColor: "#fff",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#dce2ea",
    borderRadius: 6,
  },
  heading: { fontSize: 30, fontWeight: 650 },
  full: {
    width: "100%",
    opacity: { default: 1, ":disabled": 0.5 },
    cursor: { default: "pointer", ":disabled": "not-allowed" },
  },
});
export function Login(p: { onSuccess: () => void }) {
  const config = createRequest(api.authConfig);
  const [username, setUsername] = createSignal("");
  const [password, setPassword] = createSignal("");
  const [error, setError] = createSignal(
    new URLSearchParams(window.location.search).has("auth_error")
      ? "Sign-in failed. Try again or ask your administrator to check your access."
      : "",
  );
  const [busy, setBusy] = createSignal(false);
  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (config.value()?.method === "redirect") {
      window.location.assign("/api/login");
      return;
    }
    if (config.value()?.method !== "password") return;
    setBusy(true);
    setError("");
    try {
      await api.login(username(), password());
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
        <Show when={config.value()?.method === "password"}>
          <label {...stylex.attrs(common.label)}>
            Username
            <input
              {...stylex.attrs(common.control)}
              autofocus
              autocomplete="username"
              required
              value={username()}
              onInput={(e) => setUsername(e.currentTarget.value)}
            />
          </label>
          <label {...stylex.attrs(common.label)}>
            Password
            <input
              {...stylex.attrs(common.control)}
              type="password"
              autocomplete="current-password"
              required
              value={password()}
              onInput={(e) => setPassword(e.currentTarget.value)}
            />
          </label>
        </Show>
        <Show when={config.error()}>
          <p {...stylex.attrs(common.error)} role="alert">
            Could not load sign-in options.
          </p>
          <button
            {...stylex.attrs(common.button)}
            type="button"
            onClick={() => void config.refetch()}
          >
            Retry
          </button>
        </Show>
        {error() && (
          <p {...stylex.attrs(common.error)} role="alert">
            {error()}
          </p>
        )}
        <button
          {...stylex.attrs(common.button, common.primary, styles.full)}
          disabled={busy() || config.loading() || !config.value()}
        >
          {config.loading()
            ? "Loading sign-in…"
            : config.error()
              ? "Sign-in unavailable"
              : busy()
                ? "Signing in…"
                : config.value()?.method === "redirect"
                  ? config.value()?.label || "Sign in"
                  : "Enter workspace"}
        </button>
      </form>
    </main>
  );
}
