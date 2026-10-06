import { createSignal, For, Show } from "solid-js";
import type { JSX } from "@solidjs/web";
import * as stylex from "@stylexjs/stylex";
import type { ComponentDef, Page, Template } from "../types";
import { buildPageTree, type PageTreeNode } from "./pageTree";
import { searchContent, type ContentSearchResult } from "./contentSearch";

export interface ContentSidebarProps {
  pages: readonly Page[];
  components: readonly ComponentDef[];
  templates: readonly Template[];
  canManageDefinitions: boolean;
  selectedSection: string;
  selectedPageId?: string;
  selectedBlockId?: string;
  selectedDefinitionId?: string;
  /** Parent owns the dirty guard and switches to the pages section. No ID opens all pages. */
  onNavigatePage: (pageId?: string, blockId?: string) => void;
  /** Parent owns the dirty guard. No ID opens the definition list. */
  onNavigateDefinition: (kind: "templates" | "components", definitionId?: string) => void;
  /** Parent-owned Review and People navigation. */
  children?: JSX.Element;
}

const styles = stylex.create({
  nav: { backgroundColor: "#fff", color: "#182235", minWidth: 0, fontSize: 13 },
  label: { display: "block", color: "#64748b", fontSize: 11, marginBottom: 6 },
  search: {
    boxSizing: "border-box",
    width: "100%",
    minWidth: 0,
    font: "inherit",
    padding: "9px 8px",
    backgroundColor: "#fff",
    color: "#182235",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#dce2ea",
    borderRadius: 4,
    outlineColor: "#2563eb",
    outlineOffset: 2,
  },
  section: { marginTop: 20 },
  heading: {
    margin: "0 0 6px",
    padding: "0 8px",
    color: "#64748b",
    fontSize: 11,
    fontWeight: 600,
    textTransform: "uppercase",
    letterSpacing: "0.1em",
  },
  list: { listStyleType: "none", padding: 0, margin: 0 },
  nested: {
    paddingLeft: 10,
    borderLeftWidth: 1,
    borderLeftStyle: "solid",
    borderLeftColor: "#e2e8f0",
    marginLeft: 8,
  },
  row: { display: "flex", alignItems: "center", minWidth: 0 },
  button: {
    font: "inherit",
    borderWidth: 0,
    borderRadius: 4,
    padding: "8px",
    width: "100%",
    minWidth: 0,
    textAlign: "left",
    cursor: "pointer",
    overflowWrap: "anywhere",
    backgroundColor: { default: "transparent", ":hover": "#f1f5f9" },
    color: { default: "#334155", ":hover": "#1d4ed8" },
    outlineColor: "#2563eb",
    outlineOffset: -2,
  },
  active: { backgroundColor: "#eff6ff", color: "#1d4ed8", fontWeight: 600 },
  toggle: { width: 26, flexShrink: 0, padding: "8px 3px", textAlign: "center" },
  folder: { color: "#64748b", fontStyle: "italic" },
  detail: { display: "block", color: "#64748b", fontSize: 11, lineHeight: 1.5, marginTop: 3 },
  hint: { padding: "4px 8px", fontSize: 12, color: "#64748b", overflowWrap: "anywhere" },
  extra: {
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "#dce2ea",
    marginTop: 20,
    paddingTop: 12,
  },
});

export function ContentSidebar(props: ContentSidebarProps) {
  const [query, setQuery] = createSignal("");
  const [collapsedPaths, setCollapsedPaths] = createSignal(new Set<string>());
  const tree = () => buildPageTree([...props.pages]);
  const results = () =>
    searchContent(
      query(),
      props.pages,
      props.components,
      props.templates,
      props.canManageDefinitions,
    );
  const pageSelected = (id?: string) =>
    props.selectedSection === "pages" && props.selectedPageId === id;
  const definitionSelected = (kind: string, id?: string) =>
    props.selectedSection === kind && props.selectedDefinitionId === id;
  function openResult(result: ContentSearchResult) {
    if (result.kind === "page") props.onNavigatePage(result.pageId);
    else if (result.kind === "block") props.onNavigatePage(result.pageId, result.blockId);
    else props.onNavigateDefinition(result.kind, result.definitionId);
  }
  function resultSelected(result: ContentSearchResult) {
    if (result.kind === "page") return pageSelected(result.pageId) && !props.selectedBlockId;
    if (result.kind === "block")
      return pageSelected(result.pageId) && props.selectedBlockId === result.blockId;
    return definitionSelected(result.kind, result.definitionId);
  }
  function PageNode(p: { node: PageTreeNode }) {
    const expanded = () => !collapsedPaths().has(p.node.path);
    function toggle() {
      const next = new Set(collapsedPaths());
      if (expanded()) next.add(p.node.path);
      else next.delete(p.node.path);
      setCollapsedPaths(next);
    }
    return (
      <li>
        <div {...stylex.attrs(styles.row)}>
          <Show when={p.node.children.length > 0}>
            <button
              type="button"
              {...stylex.attrs(styles.button, styles.toggle)}
              aria-label={`${expanded() ? "Collapse" : "Expand"} ${p.node.name}`}
              aria-expanded={expanded() ? "true" : "false"}
              onClick={toggle}
            >
              {expanded() ? "▾" : "▸"}
            </button>
          </Show>
          <Show
            when={p.node.page}
            fallback={
              <button
                type="button"
                {...stylex.attrs(styles.button, styles.folder)}
                aria-expanded={expanded() ? "true" : "false"}
                title={`Folder ${p.node.path}`}
                onClick={toggle}
              >
                {p.node.name}/
              </button>
            }
          >
            {(page) => (
              <button
                type="button"
                {...stylex.attrs(styles.button, pageSelected(page().id) && styles.active)}
                aria-current={pageSelected(page().id) ? "page" : undefined}
                title={page().slug}
                onClick={() => props.onNavigatePage(page().id)}
              >
                {page().title || p.node.name}
              </button>
            )}
          </Show>
        </div>
        <Show when={expanded() && p.node.children.length > 0}>
          <ul {...stylex.attrs(styles.list, styles.nested)}>
            <For each={p.node.children}>{(node) => <PageNode node={node} />}</For>
          </ul>
        </Show>
      </li>
    );
  }
  function Definitions(p: {
    kind: "templates" | "components";
    label: string;
    items: readonly (Template | ComponentDef)[];
  }) {
    const [expanded, setExpanded] = createSignal(false);
    return (
      <li>
        <div {...stylex.attrs(styles.row)}>
          <button
            type="button"
            {...stylex.attrs(styles.button, styles.toggle)}
            aria-label={`${expanded() ? "Collapse" : "Expand"} ${p.label}`}
            aria-expanded={expanded() ? "true" : "false"}
            onClick={() => setExpanded(!expanded())}
          >
            {expanded() ? "▾" : "▸"}
          </button>
          <button
            type="button"
            {...stylex.attrs(styles.button, definitionSelected(p.kind) && styles.active)}
            aria-current={definitionSelected(p.kind) ? "page" : undefined}
            onClick={() => props.onNavigateDefinition(p.kind)}
          >
            {p.label}
          </button>
        </div>
        <Show when={expanded()}>
          <ul {...stylex.attrs(styles.list, styles.nested)}>
            <For each={p.items}>
              {(item) => (
                <li>
                  <button
                    type="button"
                    {...stylex.attrs(
                      styles.button,
                      definitionSelected(p.kind, item.id) && styles.active,
                    )}
                    aria-current={definitionSelected(p.kind, item.id) ? "page" : undefined}
                    onClick={() => props.onNavigateDefinition(p.kind, item.id)}
                  >
                    {item.name}
                  </button>
                </li>
              )}
            </For>
          </ul>
          <Show when={!p.items.length}>
            <p {...stylex.attrs(styles.hint)}>No {p.label.toLowerCase()} yet.</p>
          </Show>
        </Show>
      </li>
    );
  }
  return (
    <nav {...stylex.attrs(styles.nav)} aria-label="Content navigation">
      <label {...stylex.attrs(styles.label)}>
        Search pages and content
        <input
          {...stylex.attrs(styles.search)}
          type="search"
          placeholder="Search content…"
          value={query()}
          onInput={(event) => setQuery(event.currentTarget.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") setQuery("");
          }}
        />
      </label>
      <Show
        when={query().trim()}
        fallback={
          <>
            <section {...stylex.attrs(styles.section)} aria-label="Site">
              <h2 {...stylex.attrs(styles.heading)}>Site</h2>
              <button
                type="button"
                {...stylex.attrs(styles.button, pageSelected() && styles.active)}
                aria-current={pageSelected() ? "page" : undefined}
                onClick={() => props.onNavigatePage()}
              >
                All pages <span>({props.pages.length})</span>
              </button>
              <ul {...stylex.attrs(styles.list)}>
                <Show when={tree().page}>
                  {(page) => (
                    <li>
                      <button
                        type="button"
                        {...stylex.attrs(styles.button, pageSelected(page().id) && styles.active)}
                        aria-current={pageSelected(page().id) ? "page" : undefined}
                        title="/"
                        onClick={() => props.onNavigatePage(page().id)}
                      >
                        {page().title || "Home"}
                      </button>
                    </li>
                  )}
                </Show>
                <For each={tree().children}>{(node) => <PageNode node={node} />}</For>
              </ul>
              <Show when={!props.pages.length}>
                <p {...stylex.attrs(styles.hint)}>No pages available.</p>
              </Show>
            </section>
            <Show when={props.canManageDefinitions}>
              <section {...stylex.attrs(styles.section)} aria-label="System">
                <h2 {...stylex.attrs(styles.heading)}>System</h2>
                <ul {...stylex.attrs(styles.list)}>
                  <Definitions kind="templates" label="Templates" items={props.templates} />
                  <Definitions kind="components" label="Components" items={props.components} />
                </ul>
              </section>
            </Show>
          </>
        }
      >
        <section {...stylex.attrs(styles.section)} aria-label="Search results">
          <p {...stylex.attrs(styles.hint)} role="status">
            {results().length
              ? `${results().length} ${results().length === 1 ? "result" : "results"}`
              : "No results. Try a page title, path, or content text."}
          </p>
          <ul {...stylex.attrs(styles.list)}>
            <For each={results()}>
              {(result) => (
                <li>
                  <button
                    type="button"
                    {...stylex.attrs(styles.button, resultSelected(result) && styles.active)}
                    aria-current={resultSelected(result) ? "page" : undefined}
                    onClick={() => openResult(result)}
                  >
                    <small {...stylex.attrs(styles.detail)}>
                      {result.kind === "block"
                        ? "Block"
                        : result.kind === "page"
                          ? "Page"
                          : result.kind === "templates"
                            ? "Template"
                            : "Component"}
                    </small>
                    {result.title}
                    <span {...stylex.attrs(styles.detail)}>{result.detail}</span>
                  </button>
                </li>
              )}
            </For>
          </ul>
        </section>
      </Show>
      <Show when={props.children}>
        <div {...stylex.attrs(styles.extra)}>{props.children}</div>
      </Show>
    </nav>
  );
}
