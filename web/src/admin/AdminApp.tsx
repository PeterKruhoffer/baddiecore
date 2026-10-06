import { createSignal, For, Show } from "solid-js";
import * as stylex from "@stylexjs/stylex";
import { createRequest } from "../lib/resource";
import { api, ApiError } from "../lib/api";
import type { ComponentDef, Page, Template } from "../types";
import { Login } from "./Login";
import { PageEditor } from "./PageEditor";
import { DefinitionEditor } from "./DefinitionEditor";
import { ContentSidebar } from "./ContentSidebar";
import { OrganizationEditor } from "./OrganizationEditor";
import { ReviewOverview } from "./ReviewOverview";
import { common } from "../common.stylex";
import {
  buildPageTree,
  joinPath,
  pageParentPaths,
  parentPath,
  type PageTreeNode,
} from "./pageTree";
const styles = stylex.create({
  shell: {
    display: { default: "grid", "@media (max-width: 720px)": "block" },
    gridTemplateColumns: {
      default: "250px 1fr",
      "@media (max-width: 1050px)": "210px 1fr",
    },
    gridTemplateRows: "60px minmax(0, 1fr)",
    height: "100vh",
  },
  topbar: {
    gridColumn: "1 / -1",
    backgroundColor: "#282d35",
    color: "#fff",
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    padding: "14px 22px",
    gap: 16,
  },
  role: {
    color: "#dce2ea",
    fontSize: 13,
    textTransform: "capitalize",
    display: { default: "block", "@media (max-width: 720px)": "none" },
  },
  navigationToggle: {
    display: { default: "none", "@media (max-width: 720px)": "block" },
    color: "#182235",
  },
  navigationClosed: { display: { default: "flex", "@media (max-width: 720px)": "none" } },
  sidebar: {
    backgroundColor: "#fff",
    color: "#182235",
    padding: "22px 14px",
    display: "flex",
    flexDirection: "column",
    overflowY: "auto",
    borderRightWidth: 1,
    borderRightStyle: "solid",
    borderRightColor: "#dce2ea",
    height: { default: null, "@media (max-width: 720px)": "auto" },
  },
  brand: {
    display: "flex",
    gap: 10,
    alignItems: "center",
    color: "inherit",
    textDecoration: "none",
    padding: 0,
    fontWeight: 600,
  },
  navButton: {
    width: "100%",
    borderWidth: 0,
    backgroundColor: { default: "transparent", ":hover": "#f1f5f9" },
    color: { default: "#334155", ":hover": "#1d4ed8" },
    textAlign: "left",
    display: "flex",
    justifyContent: "space-between",
  },
  active: { backgroundColor: "#eff6ff", color: "#1d4ed8" },
  sidebarFoot: {
    marginTop: "auto",
    display: "grid",
    gap: 8,
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "#dce2ea",
    paddingTop: 15,
  },
  footLink: { color: "#64748b", textDecoration: "none", padding: 7 },
  workspace: { minWidth: 0, overflow: "auto" },
  home: {
    padding: { default: "28px 32px", "@media (max-width: 720px)": "25px 16px" },
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
  pageHeading: { fontSize: 28, fontWeight: 650, margin: 0 },
  table: {
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#dce2ea",
    borderRadius: 4,
    overflow: "hidden",
    backgroundColor: "#fff",
  },
  tableHead: {
    fontSize: 11,
    textTransform: "uppercase",
    color: "#64748b",
    backgroundColor: "#f1f5f9",
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
    borderTopColor: "#e2e8f0",
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
  virtualFolder: { color: "#64748b", fontStyle: "italic" },
  addChild: { padding: "5px 8px", whiteSpace: "nowrap" },
  preview: { fontFamily: "monospace", color: "#2563eb", overflowWrap: "anywhere" },
  pill: {
    fontSize: 12,
    backgroundColor: "#fef3c7",
    color: "#92400e",
    width: "max-content",
    padding: "4px 8px",
    borderRadius: 20,
  },
  live: { backgroundColor: "#dcfce7", color: "#166534" },
  backdrop: {
    position: "fixed",
    inset: 0,
    backgroundColor: "#0f172a66",
    display: "grid",
    placeItems: "center",
    zIndex: 10,
  },
  modal: {
    backgroundColor: "#fff",
    width: "min(430px, 90vw)",
    borderRadius: 6,
    padding: 26,
  },
  modalHeading: { fontSize: 24, fontWeight: 650 },
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
  const data = () => (request.error() ? undefined : request.value());
  const refetch = request.refetch;
  type Section = "pages" | "templates" | "components" | "reviews" | "organization";
  const [section, setSection] = createSignal<Section>("pages");
  const [pageId, setPageId] = createSignal<string>();
  const [blockId, setBlockId] = createSignal<string>();
  const [definitionId, setDefinitionId] = createSignal<string>();
  const [navigation, setNavigation] = createSignal(0);
  const [dirty, setDirty] = createSignal(false);
  const [loggedOut, setLoggedOut] = createSignal(false);
  const [navigationOpen, setNavigationOpen] = createSignal(false);
  const guard = () => !dirty() || confirm("Discard your unsaved changes?");
  function choose(id?: string, selectedBlock?: string) {
    if (guard()) {
      setSection("pages");
      setPageId(id);
      setBlockId(selectedBlock);
      setDefinitionId();
      setNavigation(navigation() + 1);
      setDirty(false);
      setNavigationOpen(false);
    }
  }
  function changeSection(next: Section, selectedDefinition?: string) {
    if (!guard()) return;
    setSection(next);
    setPageId();
    setBlockId();
    setDefinitionId(selectedDefinition);
    setNavigation(navigation() + 1);
    setDirty(false);
    setNavigationOpen(false);
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
      fallback={<Login onSuccess={() => window.location.reload()} />}
    >
      <div {...stylex.attrs(styles.shell)}>
        <header {...stylex.attrs(styles.topbar)}>
          <a
            {...stylex.attrs(styles.brand)}
            href="/admin"
            onClick={(e) => {
              if (!guard()) e.preventDefault();
            }}
          >
            <span {...stylex.attrs(common.brandMark)}>B</span>
            <span>Baddiecore</span>
          </a>
          <button
            {...stylex.attrs(common.button, styles.navigationToggle)}
            aria-expanded={navigationOpen() ? "true" : "false"}
            aria-controls="workspace-navigation"
            onClick={() => setNavigationOpen(!navigationOpen())}
          >
            {navigationOpen() ? "Close navigation" : "Navigation"}
          </button>
          <span {...stylex.attrs(styles.role)}>
            {data()?.access.role === "admin" ? "Administrator" : data()?.access.role}
          </span>
        </header>
        <aside
          id="workspace-navigation"
          {...stylex.attrs(styles.sidebar, !navigationOpen() && styles.navigationClosed)}
        >
          <ContentSidebar
            pages={data()?.pages ?? []}
            components={data()?.components ?? []}
            templates={data()?.templates ?? []}
            canManageDefinitions={data()?.access.role === "admin"}
            selectedSection={section()}
            selectedPageId={pageId()}
            selectedBlockId={blockId()}
            selectedDefinitionId={definitionId()}
            onNavigatePage={choose}
            onNavigateDefinition={changeSection}
          >
            <button
              {...stylex.attrs(
                common.button,
                styles.navButton,
                section() === "reviews" && styles.active,
              )}
              onClick={() => changeSection("reviews")}
            >
              Reviews{" "}
              <span>
                {data()?.reviews.filter((r) => r.status === "submitted").length || 0} pending
              </span>
            </button>
            <Show when={data()?.access.role === "admin"}>
              <button
                {...stylex.attrs(
                  common.button,
                  styles.navButton,
                  section() === "organization" && styles.active,
                )}
                onClick={() => changeSection("organization")}
              >
                Organization
              </button>
            </Show>
          </ContentSidebar>
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
          <Show when={!request.error() && data()}>
            {(dataAccessor) => {
              const d = () => dataAccessor();
              return (
                <Show
                  when={section() !== "organization"}
                  fallback={
                    <Show when={d().access.role === "admin"}>
                      <OrganizationEditor onDirty={setDirty} onRefresh={refresh} />
                    </Show>
                  }
                >
                  <Show
                    when={section() !== "reviews"}
                    fallback={
                      <ReviewOverview
                        reviews={d().reviews}
                        role={d().access.role}
                        onRefresh={refresh}
                        onOpen={choose}
                      />
                    }
                  >
                    <Show
                      when={section() === "pages"}
                      fallback={
                        <Show when={d().access.role === "admin"}>
                          <For each={[navigation()]}>
                            {() => (
                              <DefinitionEditor
                                kind={section() as "templates" | "components"}
                                initialId={definitionId()}
                                templates={d().templates}
                                components={d().components}
                                onDirty={setDirty}
                                onSave={async (value, isNew) => {
                                  const result =
                                    section() === "templates"
                                      ? await (isNew
                                          ? api.createTemplate(value as Omit<Template, "id">)
                                          : api.updateTemplate(value as Template))
                                      : await (isNew
                                          ? api.createComponent(value as Omit<ComponentDef, "id">)
                                          : api.updateComponent(value as ComponentDef));
                                  await refresh();
                                  setDefinitionId(result.id);
                                  return result;
                                }}
                              />
                            )}
                          </For>
                        </Show>
                      }
                    >
                      <Show
                        when={page() && pageId()}
                        fallback={
                          <PageHome
                            pages={d().pages}
                            templates={d().templates}
                            paths={d().access.role === "editor" ? d().access.paths : ["/"]}
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
                                isAdmin={d().access.role === "admin"}
                                paths={d().access.role === "editor" ? d().access.paths : ["/"]}
                                review={d().reviews.find((r) => r.id === pageId())}
                                onSubmit={async (value) => {
                                  await api.submit(value);
                                  await refresh();
                                  return value;
                                }}
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
  paths: string[];
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
  const openCreate = (path = p.paths[0] || "/") => {
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
          disabled={!p.paths.length}
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
                <For
                  each={[
                    ...new Set([
                      ...pageParentPaths(p.pages),
                      ...p.paths,
                      ...p.paths.map(parentPath),
                    ]),
                  ].sort()}
                >
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
