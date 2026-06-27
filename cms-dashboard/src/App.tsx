import {
  createEffect,
  createMemo,
  createResource,
  createSignal,
  For,
  onCleanup,
  onMount,
  Show
} from "solid-js";

type ContentTreeNode = {
  id: string;
  name: string;
  path: string;
  template: string;
  workflowState: string;
  children: ContentTreeNode[];
};

type ContentTreeResponse = {
  items: ContentTreeNode[];
};

type RenderingField = {
  name: string;
  label: string;
  type: string;
  value: string;
};

type Rendering = {
  id: string;
  name: string;
  componentKey: string;
  datasourceId: string;
  fields: RenderingField[];
};

export type LayoutResponse = {
  path: string;
  mode: string;
  metadata: {
    id: string;
    name: string;
    template: string;
    workflowState: string;
    version: number;
    isDraft: boolean;
  };
  fields: Record<string, string>;
  placeholders: Array<{
    name: string;
    renderings: Rendering[];
  }>;
};

async function getJson<T>(url: string): Promise<T> {
  const response = await fetch(url, { headers: { Accept: "application/json" } });

  if (!response.ok) {
    const problem = (await response.json().catch(() => null)) as { message?: string; title?: string } | null;
    throw new Error(problem?.message ?? problem?.title ?? `Request failed with status ${response.status}.`);
  }

  return response.json() as Promise<T>;
}

function getField(rendering: Rendering, name: string): string {
  return rendering.fields.find((field) => field.name === name)?.value ?? "";
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

export function sanitizePreviewUrl(value: string): string {
  const trimmedValue = value.trim();

  if (!trimmedValue) {
    return "#";
  }

  try {
    const parsedUrl = new URL(trimmedValue, "https://preview.baddiecore.invalid");
    const allowedProtocols = new Set(["http:", "https:", "mailto:", "tel:"]);
    return allowedProtocols.has(parsedUrl.protocol) ? trimmedValue : "#";
  } catch {
    return "#";
  }
}

function renderComponent(rendering: Rendering): string {
  const id = escapeHtml(rendering.id);

  switch (rendering.componentKey) {
    case "hero":
      return `<section class="hero" data-rendering-id="${id}">
        <p class="eyebrow">${escapeHtml(getField(rendering, "eyebrow"))}</p>
        <h1>${escapeHtml(getField(rendering, "heading"))}</h1>
        <p>${escapeHtml(getField(rendering, "body"))}</p>
      </section>`;
    case "rich-text":
      return `<section class="content-block" data-rendering-id="${id}">
        <h2>${escapeHtml(getField(rendering, "heading"))}</h2>
        <p>${escapeHtml(getField(rendering, "body"))}</p>
      </section>`;
    case "cta":
      return `<section class="cta" data-rendering-id="${id}">
        <h2>${escapeHtml(getField(rendering, "heading"))}</h2>
        <a href="${escapeHtml(sanitizePreviewUrl(getField(rendering, "buttonUrl")))}">${escapeHtml(getField(rendering, "buttonText"))}</a>
      </section>`;
    default:
      return `<section class="content-block" data-rendering-id="${id}">
        <h2>${escapeHtml(rendering.name)}</h2>
        ${rendering.fields.map((field) => `<p><strong>${escapeHtml(field.label)}:</strong> ${escapeHtml(field.value)}</p>`).join("")}
      </section>`;
  }
}

export function createPreviewDocument(layout: LayoutResponse): string {
  const sections = layout.placeholders
    .flatMap((placeholder) => placeholder.renderings)
    .map(renderComponent)
    .join("");

  return `<!doctype html>
  <html lang="en">
    <head>
      <meta charset="utf-8">
      <meta name="viewport" content="width=device-width, initial-scale=1">
      <style>
        * { box-sizing: border-box; }
        body { margin: 0; color: #17212b; font-family: Inter, system-ui, sans-serif; background: #fff; }
        section { border: 2px solid transparent; cursor: pointer; transition: border-color 120ms ease; }
        section:hover, section.selected { border-color: #16738c; }
        .hero { display: grid; min-height: 340px; align-content: center; gap: 16px; padding: 60px clamp(28px, 7vw, 80px); background: linear-gradient(135deg, #f5fafb, #d8edf2); }
        .eyebrow { margin: 0; color: #16738c; font-size: 13px; font-weight: 800; letter-spacing: .08em; text-transform: uppercase; }
        h1 { max-width: 780px; margin: 0; font-size: clamp(38px, 6vw, 68px); line-height: 1; }
        .hero > p:last-child { max-width: 680px; margin: 0; color: #4d5f69; font-size: 19px; line-height: 1.55; }
        .content-block { margin: 34px auto; padding: 28px; width: min(960px, calc(100% - 48px)); }
        .content-block h2, .cta h2 { margin: 0 0 12px; font-size: 28px; }
        .content-block p { margin: 0; color: #52616b; font-size: 17px; line-height: 1.6; }
        .cta { display: flex; align-items: center; justify-content: space-between; gap: 24px; margin: 34px auto; padding: 30px; width: min(960px, calc(100% - 48px)); color: white; background: #173b4a; }
        .cta h2 { margin: 0; }
        .cta a { padding: 12px 16px; border-radius: 5px; color: #173b4a; background: white; font-weight: 800; text-decoration: none; }
      </style>
    </head>
    <body>
      ${sections || '<section class="content-block"><h2>No renderings</h2><p>This item has no composed page content.</p></section>'}
      <script>
        document.querySelectorAll('[data-rendering-id]').forEach((element) => {
          element.addEventListener('click', (event) => {
            event.preventDefault();
            event.stopPropagation();
            document.querySelectorAll('.selected').forEach((selected) => selected.classList.remove('selected'));
            element.classList.add('selected');
            window.parent.postMessage({ source: 'baddiecore-preview', renderingId: element.dataset.renderingId }, '*');
          });
        });
      </script>
    </body>
  </html>`;
}

function App() {
  const [selectedPath, setSelectedPath] = createSignal("/");
  const [selectedRenderingId, setSelectedRenderingId] = createSignal<string>();
  const [contentTree, { refetch: refetchTree }] = createResource(() => getJson<ContentTreeResponse>("/api/content/tree"));
  const [layout, { refetch: refetchLayout }] = createResource(
    selectedPath,
    (path) => getJson<LayoutResponse>(`/api/layout?path=${encodeURIComponent(path)}&mode=preview`)
  );
  let previewFrame: HTMLIFrameElement | undefined;

  const renderings = createMemo(() => layout()?.placeholders.flatMap((placeholder) => placeholder.renderings) ?? []);
  const selectedRendering = createMemo(() =>
    renderings().find((rendering) => rendering.id === selectedRenderingId())
  );
  const previewDocument = createMemo(() => layout() ? createPreviewDocument(layout()!) : "");

  createEffect(() => {
    const availableRenderings = renderings();
    const currentId = selectedRenderingId();

    if (!availableRenderings.some((rendering) => rendering.id === currentId)) {
      setSelectedRenderingId(availableRenderings[0]?.id);
    }
  });

  onMount(() => {
    const handlePreviewMessage = (event: MessageEvent) => {
      if (
        event.source === previewFrame?.contentWindow &&
        event.data?.source === "baddiecore-preview" &&
        typeof event.data.renderingId === "string"
      ) {
        setSelectedRenderingId(event.data.renderingId);
      }
    };

    window.addEventListener("message", handlePreviewMessage);
    onCleanup(() => window.removeEventListener("message", handlePreviewMessage));
  });

  const selectItem = (path: string) => {
    setSelectedRenderingId(undefined);
    setSelectedPath(path);
  };

  const refresh = () => {
    void refetchTree();
    void refetchLayout();
  };

  return (
    <div class="app-shell">
      <aside class="content-panel">
        <div class="brand">
          <span class="brand-mark">B</span>
          <div><strong>Baddiecore</strong><small>CMS dashboard</small></div>
        </div>
        <div class="panel-heading">
          <span>Content</span>
          <button type="button" onClick={refresh}>Refresh</button>
        </div>
        <Show when={!contentTree.loading} fallback={<p class="notice">Loading content…</p>}>
          <Show when={!contentTree.error} fallback={<ErrorNotice error={contentTree.error} />}>
            <nav aria-label="Content tree">
              <For each={contentTree()?.items} fallback={<p class="notice">The content tree is empty.</p>}>
                {(item) => <TreeNode item={item} selectedPath={selectedPath()} onSelect={selectItem} />}
              </For>
            </nav>
          </Show>
        </Show>
      </aside>

      <main class="workspace">
        <Show when={!layout.loading} fallback={<p class="page-state">Loading layout…</p>}>
          <Show when={!layout.error} fallback={<ErrorNotice error={layout.error} />}>
            <Show when={layout()}>
              {(page) => (
                <>
                  <header class="toolbar">
                    <div>
                      <small>{page().path}</small>
                      <h1>{page().metadata.name}</h1>
                    </div>
                    <div class="page-meta">
                      <span class="status">{page().metadata.workflowState}</span>
                      <span>Version {page().metadata.version}</span>
                    </div>
                  </header>
                  <section class="preview" aria-label="Page preview">
                    <div class="preview-bar">
                      <span>Preview · {page().metadata.template}</span>
                      <span>{renderings().length} rendering{renderings().length === 1 ? "" : "s"}</span>
                    </div>
                    <iframe
                      ref={previewFrame}
                      title={`${page().metadata.name} preview`}
                      sandbox="allow-scripts"
                      srcdoc={previewDocument()}
                    />
                  </section>
                </>
              )}
            </Show>
          </Show>
        </Show>
      </main>

      <aside class="field-panel">
        <Show when={selectedRendering()} fallback={<p class="notice">Select a rendering in the preview to inspect its fields.</p>}>
          {(rendering) => (
            <>
              <small class="kicker">Selected rendering</small>
              <h2>{rendering().name}</h2>
              <code>{rendering().componentKey}</code>
              <div class="fields">
                <For each={rendering().fields}>
                  {(field) => (
                    <label>
                      <span>{field.label}</span>
                      <Show when={field.type === "textarea"} fallback={<input value={field.value} readOnly />}>
                        <textarea rows="5" readOnly>{field.value}</textarea>
                      </Show>
                    </label>
                  )}
                </For>
              </div>
              <p class="readonly-note">Read-only API data. Editing is the next vertical slice.</p>
            </>
          )}
        </Show>
      </aside>
    </div>
  );
}

function TreeNode(props: {
  item: ContentTreeNode;
  selectedPath: string;
  onSelect: (path: string) => void;
}) {
  return (
    <div class="tree-node">
      <button
        type="button"
        classList={{ selected: props.item.path === props.selectedPath }}
        onClick={() => props.onSelect(props.item.path)}
      >
        <span>{props.item.name}</span>
        <small>{props.item.template} · {props.item.workflowState}</small>
      </button>
      <Show when={props.item.children.length > 0}>
        <div class="tree-children">
          <For each={props.item.children}>
            {(child) => <TreeNode item={child} selectedPath={props.selectedPath} onSelect={props.onSelect} />}
          </For>
        </div>
      </Show>
    </div>
  );
}

function ErrorNotice(props: { error: unknown }) {
  const message = () => props.error instanceof Error ? props.error.message : "An unexpected error occurred.";
  return <p class="notice error">{message()}</p>;
}

export default App;
