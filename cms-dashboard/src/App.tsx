import { createMemo, createSignal, For, onCleanup, onMount, Show } from "solid-js";

type TreeItem = {
  id: string;
  name: string;
  path: string;
  status: "Draft" | "Awaiting Approval" | "Approved" | "Published";
  children?: TreeItem[];
};

type Rendering = {
  id: string;
  name: string;
  componentKey: string;
  fields: Array<{ name: string; label: string; value: string; type: "text" | "textarea" }>;
};

const contentTree: TreeItem[] = [
  {
    id: "home",
    name: "Home",
    path: "/",
    status: "Draft",
    children: [
      { id: "about", name: "About", path: "/about", status: "Approved" },
      { id: "news", name: "News", path: "/news", status: "Published" },
      { id: "contact", name: "Contact", path: "/contact", status: "Awaiting Approval" }
    ]
  }
];

const renderings: Rendering[] = [
  {
    id: "hero-landing",
    name: "Hero",
    componentKey: "hero",
    fields: [
      { name: "eyebrow", label: "Eyebrow", value: "Linux-first CMS", type: "text" },
      { name: "heading", label: "Heading", value: "A Sitecore-familiar CMS without the Windows-shaped anchor", type: "textarea" },
      { name: "body", label: "Body", value: "Build, preview, approve, and publish headless content from a modern .NET platform.", type: "textarea" }
    ]
  },
  {
    id: "richtext-intro",
    name: "Rich Text",
    componentKey: "rich-text",
    fields: [
      { name: "heading", label: "Heading", value: "Editor-friendly, developer-owned", type: "text" },
      { name: "body", label: "Body", value: "Templates stay in YAML and version control. Editors work in a focused page editor.", type: "textarea" }
    ]
  },
  {
    id: "cta-main",
    name: "CTA",
    componentKey: "cta",
    fields: [
      { name: "heading", label: "Heading", value: "Ready for the next content release?", type: "text" },
      { name: "buttonText", label: "Button text", value: "Submit for approval", type: "text" }
    ]
  }
];

function App() {
  const [selectedPageId, setSelectedPageId] = createSignal("home");
  const [selectedRenderingId, setSelectedRenderingId] = createSignal("hero-landing");
  const selectedRendering = createMemo(() => renderings.find((item) => item.id === selectedRenderingId()));

  const selectedPage = createMemo(() => {
    for (const root of contentTree) {
      if (root.id === selectedPageId()) return root;
      const child = root.children?.find((item) => item.id === selectedPageId());
      if (child) return child;
    }

    return contentTree[0];
  });

  const previewSource = createMemo(() => {
    const hero = renderings[0];
    const richText = renderings[1];
    const cta = renderings[2];

    return `<!doctype html>
      <html lang="en">
        <head>
          <style>
            * { box-sizing: border-box; }
            body {
              margin: 0;
              color: #1f2933;
              font-family: Inter, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
              background: #ffffff;
            }
            .hero {
              min-height: 390px;
              display: grid;
              align-content: center;
              gap: 20px;
              padding: 72px clamp(28px, 7vw, 88px);
              background: linear-gradient(135deg, #f8fbfd 0%, #dbeef5 100%);
              border-bottom: 1px solid #d9e3ea;
            }
            .eyebrow {
              margin: 0;
              color: #1f6f8b;
              font-size: 13px;
              font-weight: 800;
              letter-spacing: 0.08em;
              text-transform: uppercase;
            }
            h1 {
              max-width: 760px;
              margin: 0;
              font-size: clamp(40px, 6vw, 76px);
              line-height: 0.98;
              letter-spacing: 0;
            }
            .hero p {
              max-width: 680px;
              margin: 0;
              color: #526170;
              font-size: 19px;
              line-height: 1.55;
            }
            main {
              display: grid;
              gap: 28px;
              width: min(1080px, calc(100% - 48px));
              margin: 38px auto 56px;
            }
            section {
              border: 2px solid transparent;
              border-radius: 8px;
              transition: border-color 150ms ease, box-shadow 150ms ease;
            }
            section:hover,
            section[data-selected="true"] {
              border-color: #1f6f8b;
              box-shadow: 0 0 0 4px rgba(31, 111, 139, 0.14);
            }
            .block {
              padding: 26px;
              background: #ffffff;
            }
            .block h2 {
              margin: 0 0 12px;
              font-size: 28px;
              letter-spacing: 0;
            }
            .block p {
              margin: 0;
              color: #5d6773;
              font-size: 17px;
              line-height: 1.6;
            }
            .cta {
              display: flex;
              align-items: center;
              justify-content: space-between;
              gap: 20px;
              padding: 28px;
              background: #17384a;
              color: #ffffff;
            }
            .cta h2 { margin: 0; font-size: 26px; }
            .cta button {
              border: 0;
              border-radius: 6px;
              padding: 12px 16px;
              color: #17384a;
              background: #ffffff;
              font-weight: 800;
            }
            @media (max-width: 720px) {
              .cta { align-items: flex-start; flex-direction: column; }
            }
          </style>
        </head>
        <body>
          <section class="hero" data-baddie-rendering-id="${hero.id}" data-selected="${selectedRenderingId() === hero.id}">
            <p class="eyebrow">${hero.fields[0].value}</p>
            <h1>${hero.fields[1].value}</h1>
            <p>${hero.fields[2].value}</p>
          </section>
          <main>
            <section class="block" data-baddie-rendering-id="${richText.id}" data-selected="${selectedRenderingId() === richText.id}">
              <h2>${richText.fields[0].value}</h2>
              <p>${richText.fields[1].value}</p>
            </section>
            <section class="cta" data-baddie-rendering-id="${cta.id}" data-selected="${selectedRenderingId() === cta.id}">
              <h2>${cta.fields[0].value}</h2>
              <button type="button">${cta.fields[1].value}</button>
            </section>
          </main>
          <script>
            document.querySelectorAll("[data-baddie-rendering-id]").forEach((element) => {
              element.addEventListener("click", () => {
                window.parent.postMessage({
                  source: "baddie-preview",
                  type: "rendering:selected",
                  renderingId: element.getAttribute("data-baddie-rendering-id")
                }, "*");
              });
            });
          </script>
        </body>
      </html>`;
  });

  onMount(() => {
    const handlePreviewMessage = (event: MessageEvent) => {
      if (event.data?.source === "baddie-preview" && event.data.type === "rendering:selected") {
        setSelectedRenderingId(event.data.renderingId);
      }
    };

    window.addEventListener("message", handlePreviewMessage);
    onCleanup(() => window.removeEventListener("message", handlePreviewMessage));
  });

  return (
    <div class="app-shell">
      <aside class="content-tree" aria-label="Content tree">
        <div class="brand">
          <span class="brand-mark">B</span>
          <div>
            <strong>Baddiecore</strong>
            <span>CMS dashboard</span>
          </div>
        </div>

        <div class="tree-header">
          <span>Content</span>
          <button type="button" title="Create page">+</button>
        </div>

        <nav>
          <For each={contentTree}>
            {(item) => (
              <TreeNode item={item} selectedId={selectedPageId()} onSelect={setSelectedPageId} />
            )}
          </For>
        </nav>
      </aside>

      <main class="editor">
        <header class="toolbar">
          <div>
            <span class="breadcrumb">Content / {selectedPage().name}</span>
            <h1>{selectedPage().name}</h1>
          </div>
          <div class="toolbar-actions">
            <span class={`status status-${selectedPage().status.toLowerCase().replaceAll(" ", "-")}`}>{selectedPage().status}</span>
            <button type="button">Save draft</button>
            <button type="button">Submit for approval</button>
            <button type="button" class="primary">Preview token</button>
          </div>
        </header>

        <section class="preview-panel" aria-label="Page preview">
          <div class="preview-chrome">
            <span>{selectedPage().path}</span>
            <div>
              <button type="button">Add component</button>
              <button type="button">Reorder</button>
            </div>
          </div>
          <iframe title="Page preview" srcdoc={previewSource()} />
        </section>
      </main>

      <aside class="field-panel" aria-label="Selected component fields">
        <Show when={selectedRendering()} fallback={<p>Select a component in the preview to edit it.</p>}>
          {(rendering) => (
            <>
              <span class="panel-kicker">Selected rendering</span>
              <h2>{rendering().name}</h2>
              <p class="component-key">componentKey: {rendering().componentKey}</p>

              <form>
                <For each={rendering().fields}>
                  {(field) => (
                    <label>
                      <span>{field.label}</span>
                      <Show
                        when={field.type === "textarea"}
                        fallback={<input value={field.value} readOnly />}
                      >
                        <textarea rows="4" readOnly>{field.value}</textarea>
                      </Show>
                    </label>
                  )}
                </For>
              </form>

              <div class="panel-actions">
                <button type="button">Remove</button>
                <button type="button" class="primary">Apply changes</button>
              </div>
            </>
          )}
        </Show>
      </aside>
    </div>
  );
}

function TreeNode(props: { item: TreeItem; selectedId: string; onSelect: (id: string) => void }) {
  return (
    <div class="tree-node">
      <button
        type="button"
        classList={{ selected: props.selectedId === props.item.id }}
        onClick={() => props.onSelect(props.item.id)}
      >
        <span>{props.item.name}</span>
        <small>{props.item.status}</small>
      </button>
      <Show when={props.item.children?.length}>
        <div class="tree-children">
          <For each={props.item.children}>
            {(child) => <TreeNode item={child} selectedId={props.selectedId} onSelect={props.onSelect} />}
          </For>
        </div>
      </Show>
    </div>
  );
}

export default App;
