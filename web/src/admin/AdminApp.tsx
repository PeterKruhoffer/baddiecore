import { createSignal, For, Show } from "solid-js";
import * as stylex from "@stylexjs/stylex";
import { createRequest } from "../lib/resource";
import { api, ApiError } from "../lib/api";
import type { ComponentDef, Page, Template } from "../types";
import { Login } from "./Login";
import { PageEditor } from "./PageEditor";
import { DefinitionEditor } from "./DefinitionEditor";
import { common } from "../common.stylex";
const styles = stylex.create({
  shell: {
    display: { default: "grid", "@media (max-width: 720px)": "block" },
    gridTemplateColumns: {
      default: "210px 1fr",
      "@media (max-width: 1050px)": "180px 1fr",
    },
    height: "100vh",
  },
  sidebar: {
    backgroundColor: "#282521",
    color: "#f7f1e8",
    padding: "21px 14px",
    display: "flex",
    flexDirection: "column",
    height: { default: null, "@media (max-width: 720px)": "auto" },
  },
  brand: {
    display: "flex",
    gap: 10,
    alignItems: "center",
    color: "inherit",
    textDecoration: "none",
    padding: "0 7px 27px",
    fontWeight: 600,
  },
  brandSmall: { display: "block", fontSize: 10, color: "#77716a" },
  nav: {
    display: "grid",
    gap: 4,
    gridTemplateColumns: {
      default: null,
      "@media (max-width: 720px)": "repeat(3, 1fr)",
    },
  },
  navButton: {
    borderWidth: 0,
    backgroundColor: { default: "transparent", ":hover": "#3a3530" },
    color: { default: "#c9c2b9", ":hover": "#fff" },
    textAlign: "left",
    display: "flex",
    justifyContent: "space-between",
  },
  active: { backgroundColor: "#3a3530", color: "#fff" },
  sidebarFoot: {
    marginTop: "auto",
    display: "grid",
    gap: 8,
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "#48423c",
    paddingTop: 15,
  },
  footLink: { color: "#c9c2b9", textDecoration: "none", padding: 7 },
  workspace: { minWidth: 0, overflow: "auto" },
  home: {
    padding: { default: "42px 48px", "@media (max-width: 720px)": "25px 16px" },
    maxWidth: 1200,
    margin: "auto",
  },
  homeHeader: {
    display: "flex",
    justifyContent: "space-between",
    marginBottom: 30,
  },
  pageHeading: { font: "600 42px Georgia, serif", margin: 0 },
  table: {
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#ddd6cb",
    borderRadius: 12,
    overflow: "hidden",
    backgroundColor: "#fffdf8",
  },
  tableRow: {
    display: "grid",
    gridTemplateColumns: {
      default: "2fr 1.2fr 1fr 1fr",
      "@media (max-width: 720px)": "1fr 1fr",
    },
    alignItems: "center",
    padding: "14px 18px",
    gap: 12,
  },
  tableHead: {
    fontSize: 11,
    textTransform: "uppercase",
    color: "#77716a",
    backgroundColor: "#eee9e0",
    display: { default: "grid", "@media (max-width: 720px)": "none" },
  },
  pageRow: {
    borderWidth: 0,
    borderRadius: 0,
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "#e9e3da",
    textAlign: "left",
  },
  pill: {
    fontSize: 12,
    backgroundColor: "#f3e4d9",
    color: "#7e4c2d",
    width: "max-content",
    padding: "4px 8px",
    borderRadius: 20,
  },
  live: { backgroundColor: "#e4eee2", color: "#346039" },
  backdrop: {
    position: "fixed",
    inset: 0,
    backgroundColor: "#27231f99",
    display: "grid",
    placeItems: "center",
    zIndex: 10,
  },
  modal: {
    backgroundColor: "#fffdf8",
    width: "min(430px, 90vw)",
    borderRadius: 14,
    padding: 26,
  },
  modalHeading: { font: "600 30px Georgia, serif" },
  footer: {
    display: "flex",
    justifyContent: "flex-end",
    gap: 8,
    marginTop: 22,
  },
  state: { padding: "15vh 10vw" },
});
export function AdminApp() {
  const request = createRequest(() => api.bootstrap());
  const data = request.value;
  const refetch = request.refetch;
  const [section, setSection] = createSignal<"pages" | "templates" | "components">("pages");
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
        !(request.error() instanceof ApiError && (request.error() as ApiError).status === 401)
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
      <div {...stylex.attrs(styles.shell)}>
        <aside {...stylex.attrs(styles.sidebar)}>
          <a
            {...stylex.attrs(styles.brand)}
            href="/admin"
            onClick={(e) => {
              if (!guard()) e.preventDefault();
            }}
          >
            <span {...stylex.attrs(common.brandMark)}>B</span>
            <span>
              Baddiecore
              <small {...stylex.attrs(styles.brandSmall)}>Workspace</small>
            </span>
          </a>
          <nav {...stylex.attrs(styles.nav)}>
            <button
              {...stylex.attrs(
                common.button,
                styles.navButton,
                section() === "pages" && styles.active,
              )}
              onClick={() => changeSection("pages")}
            >
              Pages <span>{data()?.pages.length || 0}</span>
            </button>
            <button
              {...stylex.attrs(
                common.button,
                styles.navButton,
                section() === "templates" && styles.active,
              )}
              onClick={() => changeSection("templates")}
            >
              Templates
            </button>
            <button
              {...stylex.attrs(
                common.button,
                styles.navButton,
                section() === "components" && styles.active,
              )}
              onClick={() => changeSection("components")}
            >
              Components
            </button>
          </nav>
          <div {...stylex.attrs(styles.sidebarFoot)}>
            <a {...stylex.attrs(styles.footLink)} href="/" target="_blank">
              View site ↗
            </a>
            <button
              {...stylex.attrs(common.button, styles.navButton)}
              onClick={async () => {
                if (!guard()) return;
                try {
                  const result = await api.logout();
                  setDirty(false);
                  setLoggedOut(true);
                  if (result?.redirect_url) window.location.assign(result.redirect_url);
                } catch {
                  await refetch();
                }
              }}
            >
              Sign out
            </button>
          </div>
        </aside>
        <div {...stylex.attrs(styles.workspace)}>
          <Show when={request.loading()}>
            <div>Opening workspace…</div>
          </Show>
          <Show
            when={
              request.error() &&
              !(request.error() instanceof ApiError && (request.error() as ApiError).status === 401)
            }
          >
            <div {...stylex.attrs(styles.state)} role="alert">
              <h1>Could not open workspace</h1>
              <p>
                {request.error() instanceof Error
                  ? (request.error() as Error).message
                  : "Request failed"}
              </p>
              <button {...stylex.attrs(common.button)} onClick={() => void refetch()}>
                Try again
              </button>
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
                              await (isNew
                                ? api.createTemplate(value as Omit<Template, "id">)
                                : api.updateTemplate(value as Template));
                            else
                              await (isNew
                                ? api.createComponent(value as Omit<ComponentDef, "id">)
                                : api.updateComponent(value as ComponentDef));
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
    <section {...stylex.attrs(styles.home)}>
      <header {...stylex.attrs(styles.homeHeader)}>
        <div>
          <p {...stylex.attrs(common.eyebrow)}>Content</p>
          <h1 {...stylex.attrs(styles.pageHeading)}>Pages</h1>
          <p {...stylex.attrs(common.muted)}>Draft, preview and publish the pages on your site.</p>
        </div>
        <button {...stylex.attrs(common.button, common.primary)} onClick={() => setCreating(true)}>
          New page
        </button>
      </header>
      <div {...stylex.attrs(styles.table)}>
        <div {...stylex.attrs(styles.tableRow, styles.tableHead)}>
          <span>Page</span>
          <span>Path</span>
          <span>Status</span>
          <span>Updated</span>
        </div>
        <For each={p.pages}>
          {(page) => (
            <button
              {...stylex.attrs(common.button, styles.tableRow, styles.pageRow)}
              onClick={() => p.onChoose(page.id)}
            >
              <strong>{page.title}</strong>
              <code>{page.slug}</code>
              <span
                {...stylex.attrs(
                  styles.pill,
                  page.published_revision === page.revision && styles.live,
                )}
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
        <div {...stylex.attrs(styles.backdrop)} onClick={() => setCreating(false)}>
          <form
            {...stylex.attrs(styles.modal)}
            onSubmit={submit}
            onClick={(e) => e.stopPropagation()}
          >
            <p {...stylex.attrs(common.eyebrow)}>New page</p>
            <h2 {...stylex.attrs(styles.modalHeading)}>Start with the basics</h2>
            <label {...stylex.attrs(common.label)}>
              Title
              <input {...stylex.attrs(common.control)} name="title" required autofocus />
            </label>
            <label {...stylex.attrs(common.label)}>
              Path
              <input
                {...stylex.attrs(common.control)}
                name="slug"
                required
                pattern="/.*"
                placeholder="/about"
              />
            </label>
            <label {...stylex.attrs(common.label)}>
              Template
              <select {...stylex.attrs(common.control)} name="template_id" required>
                <For each={p.templates}>{(t) => <option value={t.id}>{t.name}</option>}</For>
              </select>
            </label>
            {error() && <p {...stylex.attrs(common.error)}>{error()}</p>}
            <footer {...stylex.attrs(styles.footer)}>
              <button
                {...stylex.attrs(common.button)}
                type="button"
                onClick={() => setCreating(false)}
              >
                Cancel
              </button>
              <button {...stylex.attrs(common.button, common.primary)}>Create page</button>
            </footer>
          </form>
        </div>
      </Show>
    </section>
  );
}
