import { createSignal, For, Show } from "solid-js";
import { createRequest } from "../lib/resource";
import { api, ApiError } from "../lib/api";
import type { ComponentDef, Page, Template } from "../types";
import { Login } from "./Login";
import { PageEditor } from "./PageEditor";
import { DefinitionEditor } from "./DefinitionEditor";
export function AdminApp() {
  const request = createRequest(() => api.bootstrap());
  const data = request.value;
  const refetch = request.refetch;
  const [section, setSection] = createSignal<
    "pages" | "templates" | "components"
  >("pages");
  const [pageId, setPageId] = createSignal<string>();
  const [dirty, setDirty] = createSignal(false);
  const [loggedOut, setLoggedOut] = createSignal(false);
  const guard = () => !dirty() || confirm("Discard your unsaved changes?");
  function choose(id?: string) {
    if (guard()) {
      setPageId(id);
      setDirty(false);
    }
  }
  function changeSection(next: "pages" | "templates" | "components") {
    if (!guard()) return;
    setSection(next);
    setPageId();
    setDirty(false);
  }
  async function refresh() {
    await refetch();
  }
  const page = () => data()?.pages.find((p) => p.id === pageId());
  if (typeof window !== "undefined")
    window.onbeforeunload = () => (dirty() ? "Unsaved changes" : undefined);
  return (
    <Show
      when={
        !loggedOut() &&
        !(
          request.error() instanceof ApiError &&
          (request.error() as ApiError).status === 401
        )
      }
      fallback={
        <Login
          onSuccess={() => {
            setLoggedOut(false);
            void refetch();
          }}
        />
      }
    >
      <div class="admin-shell">
        <aside class="sidebar">
          <a
            class="brand"
            href="/admin"
            onClick={(e) => {
              if (!guard()) e.preventDefault();
            }}
          >
            <span class="brand-mark">B</span>
            <span>
              Baddiecore<small>Workspace</small>
            </span>
          </a>
          <nav>
            <button
              class={section() === "pages" ? "active" : ""}
              onClick={() => changeSection("pages")}
            >
              Pages <span>{data()?.pages.length || 0}</span>
            </button>
            <button
              class={section() === "templates" ? "active" : ""}
              onClick={() => changeSection("templates")}
            >
              Templates
            </button>
            <button
              class={section() === "components" ? "active" : ""}
              onClick={() => changeSection("components")}
            >
              Components
            </button>
          </nav>
          <div class="sidebar-foot">
            <a href="/" target="_blank">
              View site ↗
            </a>
            <button
              onClick={async () => {
                if (!guard()) return;
                try {
                  await api.logout();
                  setDirty(false);
                  setLoggedOut(true);
                } catch {
                  await refetch();
                }
              }}
            >
              Sign out
            </button>
          </div>
        </aside>
        <div class="workspace">
          <Show when={request.loading()}>
            <div class="loading">Opening workspace…</div>
          </Show>
          <Show
            when={
              request.error() &&
              !(
                request.error() instanceof ApiError &&
                (request.error() as ApiError).status === 401
              )
            }
          >
            <div class="public-state" role="alert">
              <h1>Could not open workspace</h1>
              <p>
                {request.error() instanceof Error
                  ? (request.error() as Error).message
                  : "Request failed"}
              </p>
              <button onClick={() => void refetch()}>Try again</button>
            </div>
          </Show>
          <Show when={data()}>
            {(dataAccessor) => {
              const d = () => dataAccessor();
              return (
                <Show
                  when={section() === "pages"}
                  fallback={
                    <For each={[section()]}>
                      {(kind) => (
                        <DefinitionEditor
                          kind={kind as "templates" | "components"}
                          templates={d().templates}
                          components={d().components}
                          onDirty={setDirty}
                          onSave={async (value, isNew) => {
                            if (section() === "templates")
                              isNew
                                ? await api.createTemplate(
                                    value as Omit<Template, "id">,
                                  )
                                : await api.updateTemplate(value as Template);
                            else
                              isNew
                                ? await api.createComponent(
                                    value as Omit<ComponentDef, "id">,
                                  )
                                : await api.updateComponent(
                                    value as ComponentDef,
                                  );
                            await refresh();
                          }}
                        />
                      )}
                    </For>
                  }
                >
                  <Show
                    when={pageId()}
                    fallback={
                      <PageHome
                        pages={d().pages}
                        templates={d().templates}
                        onChoose={choose}
                        onCreate={async (value) => {
                          const created = await api.createPage(value);
                          await refresh();
                          setPageId(created.id);
                        }}
                      />
                    }
                  >
                    {(id) => (
                      <For each={[id()]}>
                        {() => (
                          <PageEditor
                            page={page()!}
                            templates={d().templates}
                            components={d().components}
                            onDirty={setDirty}
                            onSave={async (value) => {
                              const result = await api.savePage(value);
                              await refresh();
                              return result;
                            }}
                            onPublish={async (value) => {
                              const result = await api.publish(value);
                              await refresh();
                              return result;
                            }}
                            onDelete={async (value) => {
                              await api.deletePage(value.id);
                              setPageId();
                              setDirty(false);
                              await refresh();
                            }}
                          />
                        )}
                      </For>
                    )}
                  </Show>
                </Show>
              );
            }}
          </Show>
        </div>
      </div>
    </Show>
  );
}
function PageHome(p: {
  pages: Page[];
  templates: Template[];
  onChoose: (id: string) => void;
  onCreate: (v: Pick<Page, "title" | "slug" | "template_id">) => Promise<void>;
}) {
  const [creating, setCreating] = createSignal(false);
  const [error, setError] = createSignal("");
  async function submit(e: SubmitEvent) {
    e.preventDefault();
    const form = new FormData(e.currentTarget as HTMLFormElement);
    try {
      await p.onCreate({
        title: String(form.get("title")),
        slug: String(form.get("slug")),
        template_id: String(form.get("template_id")),
      });
      setCreating(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not create page");
    }
  }
  return (
    <section class="page-home">
      <header>
        <div>
          <p class="eyebrow">Content</p>
          <h1>Pages</h1>
          <p class="muted">
            Draft, preview and publish the pages on your site.
          </p>
        </div>
        <button class="primary" onClick={() => setCreating(true)}>
          New page
        </button>
      </header>
      <div class="page-table">
        <div class="table-head">
          <span>Page</span>
          <span>Path</span>
          <span>Status</span>
          <span>Updated</span>
        </div>
        <For each={p.pages}>
          {(page) => (
            <button onClick={() => p.onChoose(page.id)}>
              <strong>{page.title}</strong>
              <code>{page.slug}</code>
              <span
                class={`pill ${page.published_revision === page.revision ? "live" : ""}`}
              >
                {page.published_revision === null
                  ? "Unpublished"
                  : page.published_revision === page.revision
                    ? "Published"
                    : "Draft changes"}
              </span>
              <span>Revision {page.revision} →</span>
            </button>
          )}
        </For>
      </div>
      <Show when={creating()}>
        <div class="modal-backdrop" onClick={() => setCreating(false)}>
          <form
            class="modal"
            onSubmit={submit}
            onClick={(e) => e.stopPropagation()}
          >
            <p class="eyebrow">New page</p>
            <h2>Start with the basics</h2>
            <label>
              Title
              <input name="title" required autofocus />
            </label>
            <label>
              Path
              <input name="slug" required pattern="/.*" placeholder="/about" />
            </label>
            <label>
              Template
              <select name="template_id" required>
                <For each={p.templates}>
                  {(t) => <option value={t.id}>{t.name}</option>}
                </For>
              </select>
            </label>
            {error() && <p class="error">{error()}</p>}
            <footer>
              <button type="button" onClick={() => setCreating(false)}>
                Cancel
              </button>
              <button class="primary">Create page</button>
            </footer>
          </form>
        </div>
      </Show>
    </section>
  );
}
