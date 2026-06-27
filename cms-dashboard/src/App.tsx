import { createEffect, createMemo, createResource, createSignal, on, onCleanup, onMount } from "solid-js";
import { getJson } from "./cmsApi";
import type { ContentTreeResponse, LayoutResponse } from "./cmsTypes";
import ContentSidebar from "./ContentSidebar";
import { FieldPanel } from "./FieldEditor";
import PageWorkspace from "./PageWorkspace";
import { isPreviewSelectionMessage } from "./previewProtocol";
import "./styles.css";

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
  createEffect(on([renderings, selectedRenderingId], ([availableRenderings, currentRenderingId]) => {
    if (!availableRenderings.some((rendering) => rendering.id === currentRenderingId)) {
      setSelectedRenderingId(availableRenderings[0]?.id);
    }
  }));

  onMount(() => {
    const handlePreviewMessage = (event: MessageEvent) => {
      if (
        event.source === previewFrame?.contentWindow &&
        isPreviewSelectionMessage(event.data)
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
    void Promise.all([refetchTree(), refetchLayout()]);
  };

  const refreshLayout = async () => {
    await refetchLayout();
  };

  return (
    <div class="app-shell">
      <ContentSidebar
        tree={contentTree()}
        loading={contentTree.loading}
        error={contentTree.error}
        selectedPath={selectedPath()}
        onSelect={selectItem}
        onRefresh={refresh}
      />
      <PageWorkspace
        page={layout()}
        loading={layout.loading}
        error={layout.error}
        selectedRenderingId={selectedRenderingId()}
        onFrameReady={(frame) => { previewFrame = frame; }}
      />
      <FieldPanel rendering={selectedRendering()} refreshLayout={refreshLayout} />
    </div>
  );
}

export default App;
