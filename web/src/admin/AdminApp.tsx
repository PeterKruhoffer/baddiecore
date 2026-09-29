import { createSignal, For, Show } from "solid-js";
import * as stylex from "@stylexjs/stylex";
import { createRequest } from "../lib/resource";
import { api, ApiError } from "../lib/api";
import type { ComponentDef, Page, Template } from "../types";
import { Login } from "./Login";
import { PageEditor } from "./PageEditor";
import { DefinitionEditor } from "./DefinitionEditor";
import { ContentSidebar } from "./ContentSidebar";
import { common } from "../common.stylex";
import { buildPageTree, joinPath, pageParentPaths, type PageTreeNode } from "./pageTree";
const styles = stylex.create({
  shell: {
    display: { default: "grid", "@media (max-width: 720px)": "block" },
    gridTemplateColumns: {
      default: "250px 1fr",
      "@media (max-width: 1050px)": "210px 1fr",
    },
    height: "100vh",
  },
  sidebar: {
    backgroundColor: "#282521",
    color: "#f7f1e8",
    padding: "21px 14px",
    display: "flex",
    flexDirection: "column",
    overflowY: "auto",
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
    alignItems: "flex-start",
    gap: 16,
    marginBottom: 30,
  },
  newPage: { flexShrink: 0, whiteSpace: "nowrap" },
  pageHeading: { font: "600 42px Georgia, serif", margin: 0 },
  table: {
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#ddd6cb",
    borderRadius: 12,
    overflow: "hidden",
    backgroundColor: "#fffdf8",
  },
  tableHead: {
    fontSize: 11,
    textTransform: "uppercase",
    color: "#77716a",
    backgroundColor: "#eee9e0",
    display: { default: "grid", "@media (max-width: 720px)": "none" },
  },
  treeRow: {
    display: "grid",
    gridTemplateColumns: {
      default: "minmax(220px, 2fr) 1fr 1fr auto",
      "@media (max-width: 720px)": "minmax(0, 1fr) auto",
    },
    alignItems: "center",
    gap: 12,
    padding: "10px 18px",
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "#e9e3da",
  },
  treeList: { margin: 0, padding: 0, listStyleType: "none" },
  treeName: { display: "flex", alignItems: "center", gap: 6, minWidth: 0 },
  treeToggle: {
    width: 28,
    padding: 3,
    borderWidth: 0,
    backgroundColor: "transparent",
  },
  treePage: {
    padding: 4,
    borderWidth: 0,
    backgroundColor: "transparent",
    textAlign: "left",
    overflowWrap: "anywhere",
  },
  virtualFolder: { color: "#77716a", fontStyle: "italic" },
  addChild: { padding: "5px 8px", whiteSpace: "nowrap" },
  preview: { fontFamily: "monospace", color: "#6650a5", overflowWrap: "anywhere" },
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
  const [blockId, setBlockId] = createSignal<string>();
  const [definitionId, setDefinitionId] = createSignal<string>();
  const [navigation, setNavigation] = createSignal(0);
  const [dirty, setDirty] = createSignal(false);
  const [loggedOut, setLoggedOut] = createSignal(false);
  const guard = () => !dirty() || confirm("Discard your unsaved changes?");
  function choose(id?: string, selectedBlock?: string) {
    if (guard()) {
      setSection("pages");
      setPageId(id);
      setBlockId(selectedBlock);
      setDefinitionId();
      setNavigation(navigation() + 1);
      setDirty(false);
    }
  }
  function changeSection(next: "pages" | "templates" | "components", selectedDefinition?: string) {
    if (!guard()) return;
    setSection(next);
    setPageId();
    setBlockId();
    setDefinitionId(selectedDefinition);
    setNavigation(navigation() + 1);
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
          <ContentSidebar
            pages={data()?.pages ?? []}
            components={data()?.components ?? []}
            templates={data()?.templates ?? []}
            canManageDefinitions={true}
            selectedSection={section()}
            selectedPageId={pageId()}
            selectedBlockId={blockId()}
            selectedDefinitionId={definitionId()}
            onNavigatePage={choose}
            onNavigateDefinition={changeSection}
          />
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
                    <For each={[navigation()]}>
                      {() => (
                        <DefinitionEditor
                          kind={section() as "templates" | "components"}
                          initialId={definitionId()}
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
                      <For each={[`${id()}:${navigation()}`]}>
                        {() => (
                          <PageEditor
                            page={page()!}
                            initialBlockId={blockId()}
                            pages={d().pages}
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
  const [parent, setParent] = createSignal("/");
  const [segment, setSegment] = createSignal("");
  const [error, setError] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [expanded, setExpanded] = createSignal(new Set(pageParentPaths(p.pages)));
  const tree = () => buildPageTree(p.pages);
  const openCreate = (path = "/") => {
    setParent(path);
    setSegment("");
    setError("");
    setCreating(true);
  };
  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (busy()) return;
    setBusy(true);
    setError("");
    const form = new FormData(e.currentTarget as HTMLFormElement);
    try {
      await p.onCreate({
        title: String(form.get("title")),
        slug: joinPath(String(form.get("parent")), String(form.get("segment"))),
        template_id: String(form.get("template_id")),
      });
      setCreating(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not create page");
    } finally {
      setBusy(false);
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
        <button
          {...stylex.attrs(common.button, common.primary, styles.newPage)}
          onClick={() => openCreate()}
        >
          New page
        </button>
      </header>
      <div {...stylex.attrs(styles.table)}>
        <div {...stylex.attrs(styles.treeRow, styles.tableHead)}>
          <span>Page</span>
          <span>Status</span>
          <span>Revision</span>
          <span>Actions</span>
        </div>
        <ul {...stylex.attrs(styles.treeList)} aria-label="Page tree">
          <PageTreeRows
            node={tree()}
            level={0}
            expanded={expanded()}
            onToggle={(path) => {
              const next = new Set(expanded());
              if (next.has(path)) next.delete(path);
              else next.add(path);
              setExpanded(next);
            }}
            onChoose={p.onChoose}
            onAdd={openCreate}
          />
        </ul>
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
              Parent
              <select
                {...stylex.attrs(common.control)}
                name="parent"
                value={parent()}
                onChange={(e) => setParent(e.currentTarget.value)}
              >
                <For each={pageParentPaths(p.pages)}>
                  {(path) => <option value={path}>{path}</option>}
                </For>
              </select>
            </label>
            <label {...stylex.attrs(common.label)}>
              URL segment
              <input
                {...stylex.attrs(common.control)}
                name="segment"
                required={parent() !== "/" || p.pages.some((page) => page.slug === "/")}
                pattern="[A-Za-z0-9_\-]+"
                placeholder="about"
                value={segment()}
                onInput={(e) => setSegment(e.currentTarget.value)}
              />
            </label>
            <p>
              Path preview:{" "}
              <code {...stylex.attrs(styles.preview)}>{joinPath(parent(), segment())}</code>
            </p>
            <Show when={!p.pages.some((page) => page.slug === "/")}>
              <p {...stylex.attrs(common.muted)}>
                For a homepage, choose / and leave the segment empty.
              </p>
            </Show>
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
              <button {...stylex.attrs(common.button, common.primary)} disabled={busy()}>
                {busy() ? "Creating…" : "Create page"}
              </button>
            </footer>
          </form>
        </div>
      </Show>
    </section>
  );
}

function PageTreeRows(p: {
  node: PageTreeNode;
  level: number;
  expanded: Set<string>;
  onToggle: (path: string) => void;
  onChoose: (id: string) => void;
  onAdd: (path: string) => void;
}) {
  const open = () => p.expanded.has(p.node.path);
  return (
    <li>
      <div {...stylex.attrs(styles.treeRow)}>
        <div {...stylex.attrs(styles.treeName)} style={{ "padding-left": `${p.level * 18}px` }}>
          <Show
            when={p.node.children.length}
            fallback={<span {...stylex.attrs(styles.treeToggle)} aria-hidden="true" />}
          >
            <button
              {...stylex.attrs(common.button, styles.treeToggle)}
              aria-label={`${open() ? "Collapse" : "Expand"} ${p.node.path}`}
              aria-expanded={open() ? "true" : "false"}
              onClick={() => p.onToggle(p.node.path)}
            >
              {open() ? "▾" : "▸"}
            </button>
          </Show>
          <Show
            when={p.node.page}
            fallback={<span {...stylex.attrs(styles.virtualFolder)}>{p.node.name}</span>}
          >
            {(page) => (
              <button {...stylex.attrs(styles.treePage)} onClick={() => p.onChoose(page().id)}>
                <strong>{page().title}</strong> <code>{page().slug}</code>
              </button>
            )}
          </Show>
        </div>
        <Show when={p.node.page} fallback={<span>Folder</span>}>
          {(page) => (
            <span
              {...stylex.attrs(
                styles.pill,
                page().published_revision === page().revision && styles.live,
              )}
            >
              {page().published_revision === null
                ? "Unpublished"
                : page().published_revision === page().revision
                  ? "Published"
                  : "Draft changes"}
            </span>
          )}
        </Show>
        <span>{p.node.page ? `Revision ${p.node.page.revision}` : "—"}</span>
        <button
          {...stylex.attrs(common.button, styles.addChild)}
          onClick={() => p.onAdd(p.node.path)}
        >
          + Child
        </button>
      </div>
      <Show when={open() && p.node.children.length}>
        <ul {...stylex.attrs(styles.treeList)}>
          <For each={p.node.children}>
            {(child) => <PageTreeRows {...p} node={child} level={p.level + 1} />}
          </For>
        </ul>
      </Show>
    </li>
  );
}
