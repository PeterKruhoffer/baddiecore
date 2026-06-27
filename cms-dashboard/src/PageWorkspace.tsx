import { createEffect, createMemo, Match, onCleanup, onMount, Switch } from "solid-js";
import type { LayoutResponse } from "./cmsTypes";
import { createPreviewLayoutMessage, isPreviewReadyMessage } from "./previewProtocol";
import { ErrorNotice } from "./StatusNotice";

function PageHeader(props: { page: LayoutResponse }) {
  return (
    <header class="toolbar">
      <div>
        <small>{props.page.path}</small>
        <h1>{props.page.metadata.name}</h1>
      </div>
      <div class="page-meta">
        <span class="status">{props.page.metadata.workflowState}</span>
        <span>Version {props.page.metadata.version}</span>
      </div>
    </header>
  );
}

function PagePreview(props: {
  page: LayoutResponse;
  selectedRenderingId?: string;
  onFrameReady: (frame: HTMLIFrameElement) => void;
}) {
  let frame: HTMLIFrameElement | undefined;
  const renderingCount = createMemo(() =>
    props.page.placeholders.reduce((count, placeholder) => count + placeholder.renderings.length, 0)
  );
  const sendLayout = () => {
    const message = createPreviewLayoutMessage(props.page, props.selectedRenderingId);
    // A sandbox without allow-same-origin has an opaque origin, so event.source is
    // validated on both sides instead of using a concrete target origin.
    frame?.contentWindow?.postMessage(message, "*");
  };

  createEffect(sendLayout);

  onMount(() => {
    const receiveReady = (event: MessageEvent) => {
      if (event.source === frame?.contentWindow && isPreviewReadyMessage(event.data)) {
        sendLayout();
      }
    };

    window.addEventListener("message", receiveReady);
    onCleanup(() => window.removeEventListener("message", receiveReady));
  });

  return (
    <section class="preview" aria-label="Page preview">
      <div class="preview-bar">
        <span>Preview · {props.page.metadata.template}</span>
        <span>{renderingCount()} rendering{renderingCount() === 1 ? "" : "s"}</span>
      </div>
      <iframe
        ref={(element) => {
          frame = element;
          props.onFrameReady(element);
        }}
        onLoad={sendLayout}
        title={`${props.page.metadata.name} preview`}
        sandbox="allow-scripts"
        src="/cms/preview"
      />
    </section>
  );
}

function PageWorkspace(props: {
  page?: LayoutResponse;
  loading: boolean;
  error: unknown;
  selectedRenderingId?: string;
  onFrameReady: (frame: HTMLIFrameElement) => void;
}) {
  return (
    <main class="workspace">
      <Switch>
        <Match when={props.loading}>
          <p class="page-state">Loading layout…</p>
        </Match>
        <Match when={props.error}>
          {(error) => <ErrorNotice error={error()} />}
        </Match>
        <Match when={props.page}>
          {(page) => (
            <>
              <PageHeader page={page()} />
              <PagePreview
                page={page()}
                selectedRenderingId={props.selectedRenderingId}
                onFrameReady={props.onFrameReady}
              />
            </>
          )}
        </Match>
      </Switch>
    </main>
  );
}

export default PageWorkspace;
