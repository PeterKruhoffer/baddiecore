import type { LayoutResponse } from "./cmsTypes";

export const dashboardMessageSource = "baddiecore-dashboard";
export const previewMessageSource = "baddiecore-preview";

export type PreviewLayoutMessage = {
  source: typeof dashboardMessageSource;
  type: "layout";
  layout: LayoutResponse;
  selectedRenderingId?: string;
};

export type PreviewSelectionMessage = {
  source: typeof previewMessageSource;
  type: "select-rendering";
  renderingId: string;
};

export type PreviewReadyMessage = {
  source: typeof previewMessageSource;
  type: "ready";
};

export function createPreviewLayoutMessage(
  layout: LayoutResponse,
  selectedRenderingId?: string
): PreviewLayoutMessage {
  return { source: dashboardMessageSource, type: "layout", layout, selectedRenderingId };
}

export function createPreviewSelectionMessage(renderingId: string): PreviewSelectionMessage {
  return { source: previewMessageSource, type: "select-rendering", renderingId };
}

export function createPreviewReadyMessage(): PreviewReadyMessage {
  return { source: previewMessageSource, type: "ready" };
}

export function isPreviewLayoutMessage(value: unknown): value is PreviewLayoutMessage {
  if (!value || typeof value !== "object") return false;
  const message = value as Partial<PreviewLayoutMessage>;
  return message.source === dashboardMessageSource &&
    message.type === "layout" &&
    typeof message.layout === "object" &&
    message.layout !== null;
}

export function isPreviewSelectionMessage(value: unknown): value is PreviewSelectionMessage {
  if (!value || typeof value !== "object") return false;
  const message = value as Partial<PreviewSelectionMessage>;
  return message.source === previewMessageSource &&
    message.type === "select-rendering" &&
    typeof message.renderingId === "string";
}

export function isPreviewReadyMessage(value: unknown): value is PreviewReadyMessage {
  if (!value || typeof value !== "object") return false;
  const message = value as Partial<PreviewReadyMessage>;
  return message.source === previewMessageSource && message.type === "ready";
}
