import type { Bootstrap, Role } from "../src/types";

// Browser tests intercept the API. No server credentials or database are used.
export function bootstrap(role: Role = "admin"): Bootstrap {
  const components: Bootstrap["components"] = [
    {
      id: "hero",
      name: "Hero",
      description: "Opening content",
      renderer: "hero",
      fields: [
        { name: "eyebrow", label: "Eyebrow", kind: "text", required: false },
        { name: "title", label: "Title", kind: "text", required: true },
        { name: "body", label: "Body", kind: "textarea", required: false },
        { name: "button_label", label: "Button label", kind: "text", required: false },
        { name: "button_url", label: "Button URL", kind: "url", required: false },
      ],
    },
    {
      id: "text",
      name: "Text",
      description: "Page copy",
      renderer: "text",
      fields: [
        { name: "title", label: "Title", kind: "text", required: false },
        { name: "body", label: "Body", kind: "textarea", required: false },
      ],
    },
    {
      id: "callout",
      name: "Callout",
      description: "Call to action",
      renderer: "callout",
      fields: [],
    },
    { id: "cards", name: "Cards", description: "One card per line", renderer: "cards", fields: [] },
  ];
  const templates = [
    {
      id: "landing",
      name: "Landing page",
      description: "Main and aside",
      regions: [
        { name: "main", allowed_components: ["hero", "text", "cards"], max_components: 8 },
        { name: "aside", allowed_components: ["text", "callout"], max_components: 2 },
      ],
    },
  ];
  const pages: Bootstrap["pages"] = [
    {
      id: "home",
      title: "Home",
      slug: "/",
      aliases: [],
      template_id: "landing",
      revision: 12,
      published_revision: 11,
      blocks: [
        {
          id: "intro",
          component_id: "hero",
          region: "main",
          fields: {
            eyebrow: "INDEPENDENT BY DESIGN",
            title: "A home for your next idea.",
            body: "Build, edit and publish with a CMS you own.",
            button_label: "Explore our work",
            button_url: "/services",
          },
        },
        {
          id: "copy",
          component_id: "text",
          region: "main",
          fields: { title: "Built around your content.", body: "Your pages, your server." },
        },
        {
          id: "note",
          component_id: "text",
          region: "aside",
          fields: { title: "A separate region", body: "Keep this block in aside." },
        },
      ],
    },
    {
      id: "team",
      title: "Team",
      slug: "/about/team",
      aliases: [],
      template_id: "landing",
      revision: 4,
      published_revision: null,
      blocks: [
        {
          id: "team-copy",
          component_id: "text",
          region: "main",
          fields: { title: "Meet the team", body: "Small team, clear ideas." },
        },
      ],
    },
  ];
  return {
    pages,
    templates,
    components,
    access: { id: "test-user", role, paths: ["/"] },
    reviews: pages.map((page) => ({
      id: page.id,
      submission_id: `submission-${page.id}`,
      submitted_by: "Editor 1",
      status: "submitted",
      feedback: "",
      reviewed_by: null,
      content: {
        page: structuredClone(page),
        template: structuredClone(templates[0]),
        components: structuredClone(components),
      },
    })),
  };
}
