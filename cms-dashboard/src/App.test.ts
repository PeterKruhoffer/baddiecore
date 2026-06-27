import { describe, expect, it, vi } from "vitest";
import { JSDOM } from "jsdom";
import { createPreviewDocument, sanitizePreviewUrl, type LayoutResponse } from "./App";

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
        fields: [
          { name: "heading", label: "Heading", type: "text", value: "CTA" },
          { name: "buttonText", label: "Button text", type: "text", value: "Open" },
          { name: "buttonUrl", label: "Button URL", type: "text", value: buttonUrl }
        ]
      }]
    }]
  };
}

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

  it("removes an unsafe URL from generated CTA markup", () => {
    const document = createPreviewDocument(createCtaLayout("javascript:alert(1)"));

    expect(document).toContain('href="#"');
    expect(document).not.toContain("javascript:alert(1)");
  });

  it("selects a CTA without allowing its link navigation", () => {
    const preview = new JSDOM(createPreviewDocument(createCtaLayout("/about")), {
      runScripts: "dangerously",
      url: "https://preview.baddiecore.invalid/"
    });
    const link = preview.window.document.querySelector<HTMLAnchorElement>(".cta a");
    const section = preview.window.document.querySelector<HTMLElement>(".cta");
    const click = new preview.window.MouseEvent("click", { bubbles: true, cancelable: true });
    const postMessage = vi.spyOn(preview.window, "postMessage");

    expect(link).not.toBeNull();
    expect(section).not.toBeNull();
    expect(link!.dispatchEvent(click)).toBe(false);
    expect(click.defaultPrevented).toBe(true);
    expect(section!.classList.contains("selected")).toBe(true);
    expect(preview.window.location.href).toBe("https://preview.baddiecore.invalid/");
    expect(postMessage).toHaveBeenCalledWith(
      { source: "baddiecore-preview", renderingId: "rendering-id" },
      "*"
    );

    preview.window.close();
  });
});
