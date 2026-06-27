import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { LayoutResponse } from "./cmsTypes";
import { PreviewPage } from "./PreviewPage";
import {
  createPreviewLayoutMessage,
  createPreviewReadyMessage,
  createPreviewSelectionMessage,
  isPreviewLayoutMessage,
  isPreviewReadyMessage,
  isPreviewSelectionMessage
} from "./previewProtocol";
import { sanitizePreviewUrl } from "./previewUrl";

function createCtaLayout(buttonUrl: string): LayoutResponse {
  return {
    path: "/",
    mode: "preview",
    metadata: {
      id: "page-id",
      name: "Home",
      template: "home-page",
      workflowState: "Draft",
      version: 1,
      isDraft: true
    },
    fields: {},
    placeholders: [{
      name: "main",
      renderings: [{
        id: "rendering-id",
        name: "Call to Action",
        componentKey: "cta",
        datasourceId: "datasource-id",
        datasourceVersion: 3,
        fields: [
          { name: "heading", label: "Heading", type: "text", required: true, value: "CTA" },
          { name: "buttonText", label: "Button text", type: "text", required: true, value: "Open" },
          { name: "buttonUrl", label: "Button URL", type: "text", required: true, value: buttonUrl }
        ]
      }]
    }]
  };
}

afterEach(() => {
  document.body.replaceChildren();
});

describe("preview URL handling", () => {
  it.each([
    "javascript:alert(document.domain)",
    "data:text/html,<script>alert(1)</script>",
    "vbscript:msgbox(1)",
    "  javascript:alert(1)"
  ])("rejects the active URL %s", (url) => {
    expect(sanitizePreviewUrl(url)).toBe("#");
  });

  it.each(["/about", "https://example.com/about", "mailto:editor@example.com", "tel:+4512345678"])(
    "allows the safe URL %s",
    (url) => {
      expect(sanitizePreviewUrl(url)).toBe(url);
    }
  );

  it("renders safe Solid content and selects a CTA without navigating", () => {
    const host = document.createElement("div");
    document.body.append(host);
    const onSelect = vi.fn();
    const dispose = render(
      () => <PreviewPage layout={createCtaLayout("javascript:alert(1)")} onSelect={onSelect} />,
      host
    );
    const link = host.querySelector<HTMLAnchorElement>(".cta a");
    const section = host.querySelector<HTMLElement>(".cta");
    const click = new MouseEvent("click", { bubbles: true, cancelable: true });

    expect(link?.getAttribute("href")).toBe("#");
    expect(section?.dispatchEvent(click)).toBe(false);
    expect(click.defaultPrevented).toBe(true);
    expect(onSelect).toHaveBeenCalledWith("rendering-id");

    dispose();
  });
});

describe("preview messaging", () => {
  it("accepts only typed layout messages", () => {
    expect(isPreviewLayoutMessage(createPreviewLayoutMessage(createCtaLayout("/about")))).toBe(true);
    expect(isPreviewLayoutMessage({ source: "someone-else", type: "layout" })).toBe(false);
  });

  it("accepts only typed selection messages", () => {
    expect(isPreviewSelectionMessage(createPreviewSelectionMessage("rendering-id"))).toBe(true);
    expect(isPreviewSelectionMessage({ source: "baddiecore-preview", type: "other" })).toBe(false);
  });

  it("accepts only typed ready messages", () => {
    expect(isPreviewReadyMessage(createPreviewReadyMessage())).toBe(true);
    expect(isPreviewReadyMessage({ source: "baddiecore-preview", type: "other" })).toBe(false);
  });
});
