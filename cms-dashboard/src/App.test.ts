import { describe, expect, it, vi } from "vitest";
import { updateRenderingDatasource } from "./cmsApi";
import type { LayoutResponse } from "./cmsTypes";

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

describe("datasource field updates", () => {
  it("sends the datasource version and edited field values", async () => {
    const layout = createCtaLayout("/about");
    const rendering = layout.placeholders[0].renderings[0];
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(null, { status: 204 }));

    await updateRenderingDatasource(rendering, {
      heading: "A changed heading",
      buttonText: "Open",
      buttonUrl: "/about"
    });

    expect(fetchMock).toHaveBeenCalledWith(
      "/api/renderings/rendering-id/datasource/fields",
      expect.objectContaining({
        method: "PUT",
        body: JSON.stringify({
          datasourceId: "datasource-id",
          expectedVersion: 3,
          fields: [
            { name: "heading", value: "A changed heading" },
            { name: "buttonText", value: "Open" },
            { name: "buttonUrl", value: "/about" }
          ]
        })
      })
    );

    fetchMock.mockRestore();
  });

  it("surfaces stale-version conflicts with the current version", async () => {
    const rendering = createCtaLayout("/about").placeholders[0].renderings[0];
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(JSON.stringify({
      title: "Datasource version conflict",
      detail: "The datasource changed after it was loaded.",
      currentVersion: 4
    }), { status: 409, headers: { "Content-Type": "application/problem+json" } }));

    await expect(updateRenderingDatasource(rendering, {
      heading: "Stale edit",
      buttonText: "Open",
      buttonUrl: "/about"
    })).rejects.toMatchObject({
      status: 409,
      currentVersion: 4,
      message: "The datasource changed after it was loaded."
    });

    fetchMock.mockRestore();
  });
});
