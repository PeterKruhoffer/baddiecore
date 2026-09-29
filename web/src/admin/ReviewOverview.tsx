import { createSignal, For, Show } from "solid-js";
import * as stylex from "@stylexjs/stylex";
import type { Review, Role } from "../types";
import { api } from "../lib/api";
import { common } from "../common.stylex";
import { BlockRenderer, blocksInTemplateOrder } from "../components/Renderer";

const styles = stylex.create({
  layout: { padding: 28, maxWidth: 1100, margin: "auto" },
  card: {
    padding: 20,
    marginBottom: 18,
    backgroundColor: "#fffdf8",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#ddd6cb",
    borderRadius: 12,
  },
  actions: { display: "flex", gap: 10, flexWrap: "wrap", marginTop: 14 },
  filters: { display: "flex", gap: 8, flexWrap: "wrap", marginBlock: 20 },
  heading: { font: "600 36px Georgia, serif" },
  preview: { backgroundColor: "#fff", marginTop: 15 },
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
  const filters = [
    { value: "submitted", label: "In review" },
    { value: "changes_requested", label: "Changes requested" },
    { value: "approved", label: "Approved" },
    { value: "all", label: "All" },
  ] as const;
  const visible = () => p.reviews.filter((r) => filter() === "all" || r.status === filter());
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
      <h1 {...stylex.attrs(styles.heading)}>Reviews</h1>
      <p>
        Review the submitted snapshot below. Approval publishes that exact revision. Later edits or
        schema changes require a new submission.
      </p>
      <button {...stylex.attrs(common.button)} disabled={busy()} onClick={() => void p.onRefresh()}>
        Refresh reviews
      </button>
      <div {...stylex.attrs(styles.filters)} role="group" aria-label="Review status">
        <For each={filters}>
          {(item) => (
            <button
              {...stylex.attrs(common.button, filter() === item.value && common.primary)}
              aria-pressed={filter() === item.value ? "true" : "false"}
              disabled={busy()}
              onClick={() => setFilter(item.value)}
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
      <For each={visible()}>
        {(review) => (
          <article {...stylex.attrs(styles.card)}>
            <h2>{review.content.page.title}</h2>
            <p>
              <code>{review.content.page.slug}</code> · Revision {review.content.page.revision} ·{" "}
              {review.status.replaceAll("_", " ")}
            </p>
            <p>
              Submitted by {review.submitted_by}
              {review.reviewed_by ? ` · Reviewed by ${review.reviewed_by}` : ""}
            </p>
            <Show when={review.feedback}>
              <p>
                <strong>Feedback</strong> {review.feedback}
              </p>
            </Show>
            <button
              {...stylex.attrs(common.button)}
              disabled={busy()}
              onClick={() => p.onOpen(review.id)}
            >
              Open current draft
            </button>
            <details>
              <summary>Submitted content and schema</summary>
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
                <pre {...stylex.attrs(styles.data)}>{JSON.stringify(review.content, null, 2)}</pre>
              </details>
            </details>
            <Show when={p.role !== "editor" && review.status === "submitted"}>
              <form
                onSubmit={(e) => {
                  e.preventDefault();
                  const feedback = String(new FormData(e.currentTarget).get("feedback") || "");
                  const approve = (e.submitter as HTMLButtonElement)?.value === "approve";
                  void decide(review, approve, feedback);
                }}
              >
                <label {...stylex.attrs(common.label)}>
                  Feedback, required to request changes
                  <textarea
                    name="feedback"
                    {...stylex.attrs(common.control, common.textarea)}
                    disabled={busy()}
                  />
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
          </article>
        )}
      </For>
    </section>
  );
}
