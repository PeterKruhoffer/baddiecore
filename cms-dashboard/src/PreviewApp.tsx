import { batch, createSignal, onCleanup, onMount, Show } from "solid-js";
import type { LayoutResponse } from "./cmsTypes";
import { PreviewPage } from "./PreviewPage";
import {
  createPreviewReadyMessage,
  createPreviewSelectionMessage,
  isPreviewLayoutMessage
} from "./previewProtocol";
import "./previewStyles.css";

function PreviewApp() {
  const [layout, setLayout] = createSignal<LayoutResponse>();
  const [selectedRenderingId, setSelectedRenderingId] = createSignal<string>();

  const selectRendering = (renderingId: string) => {
    setSelectedRenderingId(renderingId);
    window.parent.postMessage(createPreviewSelectionMessage(renderingId), "*");
  };

  onMount(() => {
    const receiveLayout = (event: MessageEvent) => {
      if (event.source !== window.parent || !isPreviewLayoutMessage(event.data)) return;

      batch(() => {
        setLayout(event.data.layout);
        setSelectedRenderingId(event.data.selectedRenderingId);
      });
    };

    window.addEventListener("message", receiveLayout);
    window.parent.postMessage(createPreviewReadyMessage(), "*");
    onCleanup(() => window.removeEventListener("message", receiveLayout));
  });

  return (
    <Show when={layout()} fallback={<p class="preview-state">Waiting for preview data…</p>}>
      {(page) => (
        <PreviewPage
          layout={page()}
          selectedRenderingId={selectedRenderingId()}
          onSelect={selectRendering}
        />
      )}
    </Show>
  );
}

export default PreviewApp;
