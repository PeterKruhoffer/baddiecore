import { createSignal, For, Show } from "solid-js";
import * as stylex from "@stylexjs/stylex";
import type { Review, Role } from "../types";
import { api } from "../lib/api";
import { common } from "../common.stylex";
import { BlockRenderer, blocksInTemplateOrder } from "../components/Renderer";

const styles = stylex.create({
  layout: { padding: { default: 24, "@media (max-width: 720px)": 16 } },
  header: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 16,
    flexWrap: "wrap",
  },
  workspace: {
    display: "grid",
    gridTemplateColumns: { default: "260px minmax(0, 1fr)", "@media (max-width: 1050px)": "1fr" },
    gap: 16,
  },
  queue: { display: "grid", alignContent: "start", gap: 8 },
  item: { display: "grid", gap: 8, textAlign: "left", padding: 16 },
  selected: { backgroundColor: "#eff6ff", borderColor: "#2563eb", color: "#1d4ed8" },
  detail: {
    display: "grid",
    gridTemplateColumns: { default: "minmax(0, 1fr) 280px", "@media (max-width: 1200px)": "1fr" },
    gap: 20,
  },
  decision: {
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "#dce2ea",
    paddingTop: 20,
  },
  meta: { fontSize: 12, color: "#64748b", overflowWrap: "anywhere" },
  card: {
    padding: 20,
    minWidth: 0,
    backgroundColor: "#fff",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#dce2ea",
    borderRadius: 4,
  },
  actions: { display: "flex", gap: 10, flexWrap: "wrap", marginTop: 14 },
  filters: { display: "flex", gap: 8, flexWrap: "wrap", marginBlock: 20 },
  heading: { fontSize: 26, fontWeight: 650, margin: "0 0 8px" },
  preview: {
    backgroundColor: "#fff",
    marginTop: 15,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#e2e8f0",
    overflow: "hidden",
  },
  data: { overflow: "auto", maxHeight: 400 },
});

export function ReviewOverview(p: {
  reviews: Review[];
  role: Role;
  onRefresh: () => Promise<void>;
  onOpen: (id: string) => void;
}) {
  const [error, setError] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [filter, setFilter] = createSignal<Review["status"] | "all">("submitted");
  const [selected, setSelected] = createSignal<string>();
  const filters = [
    { value: "submitted", label: "In review" },
    { value: "changes_requested", label: "Changes requested" },
    { value: "approved", label: "Approved" },
    { value: "all", label: "All" },
  ] as const;
  const visible = () => p.reviews.filter((r) => filter() === "all" || r.status === filter());
  const current = () => visible().find((r) => r.id === selected()) ?? visible()[0];
  async function decide(review: Review, approve: boolean, feedback: string) {
    setBusy(true);
    setError("");
    try {
      await api.review(review, approve, feedback);
      await p.onRefresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Review failed");
    } finally {
      setBusy(false);
    }
  }
  return (
    <section {...stylex.attrs(styles.layout)}>
      <header {...stylex.attrs(styles.header)}>
        <div>
          <h1 {...stylex.attrs(styles.heading)}>Reviews</h1>
          <p {...stylex.attrs(common.muted)}>
            Review the submitted snapshot. Approval publishes that exact revision immediately.
          </p>
        </div>
        <button
          {...stylex.attrs(common.button)}
          disabled={busy()}
          onClick={() => void p.onRefresh()}
        >
          Refresh reviews
        </button>
      </header>
      <div {...stylex.attrs(styles.filters)} role="group" aria-label="Review status">
        <For each={filters}>
          {(item) => (
            <button
              {...stylex.attrs(common.button, filter() === item.value && common.primary)}
              aria-pressed={filter() === item.value ? "true" : "false"}
              disabled={busy()}
              onClick={() => {
                setFilter(item.value);
                setError("");
              }}
            >
              {item.label} (
              {p.reviews.filter((r) => item.value === "all" || r.status === item.value).length})
            </button>
          )}
        </For>
      </div>
      <Show when={error()}>
        <p role="alert" {...stylex.attrs(common.error)}>
          {error()}
        </p>
      </Show>
      <Show when={!visible().length}>
        <p role="status">
          {p.reviews.length
            ? "No submissions in this status. Choose another status to see earlier decisions."
            : "No submissions yet. Open a page and choose Submit for review."}
        </p>
      </Show>
      <div {...stylex.attrs(styles.workspace)}>
        <nav {...stylex.attrs(styles.queue)} aria-label="Submissions">
          <For each={visible()}>
            {(review) => (
              <button
                {...stylex.attrs(
                  common.button,
                  styles.item,
                  current()?.id === review.id && styles.selected,
                )}
                aria-pressed={current()?.id === review.id ? "true" : "false"}
                disabled={busy()}
                onClick={() => {
                  setSelected(review.id);
                  setError("");
                }}
              >
                <strong>{review.content.page.title}</strong>
                <code>{review.content.page.slug}</code>
                <small {...stylex.attrs(styles.meta)}>
                  Revision {review.content.page.revision} · Submitted by {review.submitted_by}
                </small>
              </button>
            )}
          </For>
        </nav>
        <For each={current() ? [current()!] : []}>
          {(review) => (
            <article {...stylex.attrs(styles.card)}>
              <h2 {...stylex.attrs(common.heading)}>{review.content.page.title}</h2>
              <p>
                <code>{review.content.page.slug}</code> · Revision {review.content.page.revision} ·{" "}
                <span {...stylex.attrs(common.badge, review.status === "approved" && common.live)}>
                  {review.status === "submitted"
                    ? "In review"
                    : review.status === "approved"
                      ? "Approved and published"
                      : "Changes requested"}
                </span>
              </p>
              <p {...stylex.attrs(styles.meta)}>
                Submitted by {review.submitted_by}
                {review.reviewed_by ? ` · Reviewed by ${review.reviewed_by}` : ""}
              </p>
              <div {...stylex.attrs(styles.detail)}>
                <section aria-label="Submitted preview">
                  <h3>Submitted preview</h3>
                  <div
                    {...stylex.attrs(styles.preview)}
                    onClick={(e) => {
                      if ((e.target as HTMLElement).closest("a")) e.preventDefault();
                    }}
                  >
                    <For
                      each={blocksInTemplateOrder(
                        review.content.page.blocks,
                        review.content.template.regions,
                      )}
                    >
                      {(block) => {
                        const definition = review.content.components.find(
                          (c) => c.id === block.component_id,
                        );
                        return (
                          <Show when={definition}>
                            {(d) => <BlockRenderer block={block} definition={d()} />}
                          </Show>
                        );
                      }}
                    </For>
                  </div>
                  <details>
                    <summary>Exact submitted data</summary>
                    <pre {...stylex.attrs(styles.data)}>
                      {JSON.stringify(review.content, null, 2)}
                    </pre>
                  </details>
                </section>
                <aside {...stylex.attrs(styles.decision)} aria-label="Review decision">
                  <h3>Review decision</h3>
                  <Show when={review.feedback}>
                    <p {...stylex.attrs(common.notice, common.warning)}>
                      <strong>Feedback</strong>
                      <br />
                      {review.feedback}
                    </p>
                  </Show>
                  <Show when={p.role !== "editor" && review.status === "submitted"}>
                    <p {...stylex.attrs(common.notice)}>
                      Approving publishes this submitted revision immediately.
                    </p>
                    <form
                      onSubmit={(e) => {
                        e.preventDefault();
                        const feedback = String(
                          new FormData(e.currentTarget).get("feedback") || "",
                        );
                        const approve = (e.submitter as HTMLButtonElement)?.value === "approve";
                        if (!approve && !feedback.trim()) {
                          setError("Feedback is required when requesting changes.");
                          return;
                        }
                        void decide(review, approve, feedback);
                      }}
                    >
                      <label {...stylex.attrs(common.label)}>
                        Feedback
                        <textarea
                          name="feedback"
                          {...stylex.attrs(common.control, common.textarea)}
                          disabled={busy()}
                        />
                        <small {...stylex.attrs(common.muted)}>
                          Required when requesting changes.
                        </small>
                      </label>
                      <div {...stylex.attrs(styles.actions)}>
                        <button {...stylex.attrs(common.button)} value="changes" disabled={busy()}>
                          Request changes
                        </button>
                        <button
                          {...stylex.attrs(common.button, common.primary)}
                          value="approve"
                          disabled={busy()}
                        >
                          Approve and publish
                        </button>
                      </div>
                    </form>
                  </Show>
                  <p {...stylex.attrs(styles.meta)}>
                    Reviewing a submitted snapshot, not unsaved edits. If the page or its
                    definitions change after submission, it must be submitted again.
                  </p>
                  <button
                    {...stylex.attrs(common.button)}
                    disabled={busy()}
                    onClick={() => p.onOpen(review.id)}
                  >
                    Open current draft
                  </button>
                </aside>
              </div>
            </article>
          )}
        </For>
      </div>
    </section>
  );
}
