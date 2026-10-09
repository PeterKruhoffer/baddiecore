import { expect, test, type Page as BrowserPage } from "@playwright/test";
import { bootstrap } from "./admin-fixture";
import type { Bootstrap, Page, Template } from "../src/types";

async function openWorkspace(page: BrowserPage, data: Bootstrap = bootstrap()) {
  const writes: { url: string; body: Record<string, any> }[] = [];
  const failures = { save: false, publish: false, review: false };
  await page.route("**/api/**", async (route) => {
    const req = route.request();
    const url = new URL(req.url()).pathname;
    const body = req.method() === "GET" ? undefined : req.postDataJSON();
    if (body) writes.push({ url, body });
    if (url === "/api/admin/bootstrap") return route.fulfill({ json: data });
    if (/^\/api\/admin\/pages\/[^/]+$/.test(url) && req.method() === "PUT") {
      if (failures.save)
        return route.fulfill({
          status: 409,
          json: { error: "Draft changed. Reload before saving." },
        });
      const index = data.pages.findIndex((p) => p.id === url.split("/").at(-1));
      data.pages[index] = { ...body, revision: body.revision + 1 } as Page;
      return route.fulfill({ json: data.pages[index] });
    }
    if (url === "/api/admin/pages/home/publish") {
      if (failures.publish)
        return route.fulfill({
          status: 409,
          json: { error: "Publication conflict. Reload the draft." },
        });
      data.pages[0].published_revision = body.revision;
      return route.fulfill({ json: data.pages[0] });
    }
    if (url === "/api/admin/pages/home/submit") {
      data.reviews[0] = {
        ...data.reviews[0],
        status: "submitted",
        content: { ...data.reviews[0].content, page: structuredClone(data.pages[0]) },
      };
      return route.fulfill({ json: data.reviews[0] });
    }
    if (url.startsWith("/api/admin/reviews/")) {
      if (failures.review)
        return route.fulfill({ status: 409, json: { error: "Submission changed. Submit again." } });
      const review = data.reviews.find((r) => r.id === url.split("/").at(-1))!;
      review.status = body.approve ? "approved" : "changes_requested";
      review.feedback = body.feedback;
      return route.fulfill({ json: review });
    }
    if (url === "/api/admin/templates" && req.method() === "POST") {
      const template = { ...body, id: "campaign" } as Template;
      data.templates.push(template);
      return route.fulfill({ json: template });
    }
    if (url === "/api/admin/templates/campaign" && req.method() === "PUT") {
      data.templates[1] = body;
      return route.fulfill({ json: body });
    }
    return route.fulfill({ status: 500, json: { error: `Unexpected test request: ${url}` } });
  });
  await page.goto("/admin");
  await expect(page.getByRole("heading", { name: "Pages", exact: true })).toBeVisible();
  return { data, writes, failures };
}

async function capture(page: BrowserPage, name: string) {
  if (process.env.UI_SCREENSHOTS_DIR) {
    await page.screenshot({
      path: `${process.env.UI_SCREENSHOTS_DIR}/${name}.png`,
      fullPage: true,
    });
  }
}

test("fields retain focus, update the preview, and keep edits across modes and guarded navigation", async ({
  page,
}) => {
  await openWorkspace(page);
  await page.getByRole("button", { name: "Home", exact: true }).click();
  const title = page.getByRole("textbox", { name: "Title *", exact: true });
  await title.fill("A better home");
  await title.pressSequentially(" for ideas");
  await expect(title).toBeFocused();
  await expect(title).toHaveValue("A better home for ideas");
  await expect(page.getByRole("heading", { name: "A better home for ideas" })).toBeVisible();
  await capture(page, "blue-editor");
  await page.getByRole("button", { name: "Content", exact: true }).click();
  await expect(page.getByRole("main")).toBeHidden();
  await expect(title).toHaveValue("A better home for ideas");
  await page.getByRole("button", { name: "Experience", exact: true }).click();
  await expect(page.getByRole("main")).toBeVisible();
  page.once("dialog", (dialog) => dialog.dismiss());
  await page.getByRole("button", { name: "Team", exact: true }).click();
  await expect(page.getByRole("textbox", { name: "Page title" })).toHaveValue("Home");
  await expect(title).toHaveValue("A better home for ideas");
  await page.getByRole("button", { name: "Mobile", exact: true }).click();
  await expect(page.getByRole("button", { name: "Mobile", exact: true })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
});

test("preview selection, reordering and removal preserve component identity and region limits", async ({
  page,
}) => {
  const state = await openWorkspace(page);
  await page.getByRole("button", { name: "Home", exact: true }).click();
  await page.getByRole("main").getByRole("heading", { name: "Built around your content." }).click();
  await page.getByRole("textbox", { name: "Title", exact: true }).fill("Reordered copy");
  await page.getByRole("button", { name: "Move Hero down", exact: true }).click();
  const aside = page.getByRole("combobox", { name: "Add to aside", exact: true });
  await expect(aside.locator("option")).toHaveText(["+ Add component", "Text", "Callout"]);
  await aside.selectOption("callout");
  await aside.selectOption("callout");
  await expect(page.getByRole("alert")).toContainText("aside allows up to 2 components");
  await page.getByRole("button", { name: "Remove component", exact: true }).click();
  await aside.selectOption("text");
  await page.getByRole("textbox", { name: "Title", exact: true }).fill("New aside copy");
  await page.getByRole("button", { name: "Save draft", exact: true }).click();
  await expect(page.getByText("Saved draft · Revision 13", { exact: true })).toBeVisible();
  const blocks = state.writes[0].body.blocks;
  expect(blocks.slice(0, 3).map((block: { id: string }) => block.id)).toEqual([
    "copy",
    "intro",
    "note",
  ]);
  expect(blocks[0].fields.title).toBe("Reordered copy");
  expect(blocks[1].fields.title).toBe("A home for your next idea.");
  expect(blocks[2].fields.title).toBe("A separate region");
  expect(blocks).toHaveLength(4);
  expect(blocks[3]).toMatchObject({
    component_id: "text",
    region: "aside",
    fields: { title: "New aside copy", body: "" },
  });
});

test("confirmation cancels without writes; publishing saves first and uses the new revision only for this page", async ({
  page,
}) => {
  const state = await openWorkspace(page);
  await page.getByRole("button", { name: "Home", exact: true }).click();
  await page.getByRole("textbox", { name: "Title *", exact: true }).fill("Ready to publish");
  await page.getByRole("button", { name: "Publish", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await expect(dialog).toContainText("Child pages will not be published");
  await capture(page, "blue-publishing");
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  expect(state.writes).toEqual([]);
  await expect(page.getByRole("button", { name: "Publish", exact: true })).toBeFocused();
  await page.getByRole("button", { name: "Publish", exact: true }).click();
  await dialog.getByRole("button", { name: "Save and publish" }).click();
  await expect(dialog).not.toBeVisible();
  expect(state.writes.map((w) => w.url)).toEqual([
    "/api/admin/pages/home",
    "/api/admin/pages/home/publish",
  ]);
  expect(state.writes[0].body.blocks[0].fields.title).toBe("Ready to publish");
  expect(state.writes[1].body).toEqual({ revision: 13 });
  expect(state.data.pages[1].published_revision).toBeNull();
  await expect(page.getByText("Saved draft · Revision 13", { exact: true })).toBeVisible();
});

test("save conflicts prevent publication and retain edits; a later publish failure keeps the saved draft and dialog", async ({
  page,
}) => {
  const state = await openWorkspace(page);
  state.failures.save = true;
  await page.getByRole("button", { name: "Home", exact: true }).click();
  await page.getByRole("textbox", { name: "Title *", exact: true }).fill("Keep my changes");
  await page.getByRole("button", { name: "Publish", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("button", { name: "Save and publish" }).click();
  await expect(dialog.getByRole("alert")).toContainText("Draft changed");
  expect(state.writes.map((w) => w.url)).toEqual(["/api/admin/pages/home"]);
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(page.getByRole("textbox", { name: "Title *", exact: true })).toHaveValue(
    "Keep my changes",
  );
  state.failures.save = false;
  state.failures.publish = true;
  await page.getByRole("button", { name: "Publish", exact: true }).click();
  await dialog.getByRole("button", { name: "Save and publish" }).click();
  await expect(dialog.getByRole("alert")).toContainText("Publication conflict");
  await expect(dialog.getByRole("button", { name: "Publish page", exact: true })).toBeEnabled();
  await expect(dialog).toContainText("13");
  expect(state.data.pages[0].published_revision).toBe(11);
});

test("route aliases preserve multiline editing, save with the draft, and can be removed", async ({
  page,
}) => {
  const state = await openWorkspace(page);
  await page.getByRole("button", { name: "Team", exact: true }).click();
  await page.getByText("Page details · Landing page", { exact: true }).click();
  const aliases = page.getByRole("textbox", { name: "Route aliases", exact: true });
  await expect(aliases).toHaveValue("");
  await expect(aliases).toHaveAttribute("aria-describedby", "route-alias-help");
  await capture(page, "aliases-empty");
  await aliases.fill("/team");
  await aliases.press("End");
  await aliases.press("Enter");
  await aliases.pressSequentially("/meet-us");
  await expect(aliases).toHaveValue("/team\n/meet-us");
  await page.getByRole("button", { name: "Save draft", exact: true }).click();
  expect(state.writes[0].body.aliases).toEqual(["/team", "/meet-us"]);
  expect(state.writes[0].body.slug).toBe("/about/team");
  await expect(page.getByText("Saved draft · Revision 5", { exact: true })).toBeVisible();
  await capture(page, "aliases-saved");
  await page.setViewportSize({ width: 390, height: 844 });
  await aliases.scrollIntoViewIfNeeded();
  await expect(aliases).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await capture(page, "aliases-narrow");
  await aliases.fill("");
  await page.getByRole("button", { name: "Save draft", exact: true }).click();
  expect(state.writes[1].body.aliases).toEqual([]);
});

test("alias changes save before publication and survive a rejected save", async ({ page }) => {
  const state = await openWorkspace(page);
  await page.getByRole("button", { name: "Home", exact: true }).click();
  await page.getByText("Page details · Landing page", { exact: true }).click();
  const aliases = page.getByRole("textbox", { name: "Route aliases", exact: true });
  await aliases.fill("/welcome");
  state.failures.save = true;
  await page.getByRole("button", { name: "Save draft", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Draft changed");
  await expect(aliases).toHaveValue("/welcome");
  state.failures.save = false;
  await page.getByRole("button", { name: "Publish", exact: true }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Save and publish" }).click();
  await expect(page.getByRole("dialog")).not.toBeVisible();
  expect(state.writes.slice(1).map((write) => write.url)).toEqual([
    "/api/admin/pages/home",
    "/api/admin/pages/home/publish",
  ]);
  expect(state.data.pages[0].aliases).toEqual(["/welcome"]);
});

test("review decisions target the selected submitted snapshot, require change feedback, and retain feedback on conflicts", async ({
  page,
}) => {
  const data = bootstrap("reviewer");
  data.pages[0].blocks[0].fields.title = "A newer draft that is not submitted";
  data.pages[0].revision = 13;
  const state = await openWorkspace(page, data);
  await page.getByRole("button", { name: /Reviews .*pending/ }).click();
  await expect(page.getByRole("region", { name: "Submitted preview" })).toContainText(
    "A home for your next idea.",
  );
  await expect(page.getByRole("region", { name: "Submitted preview" })).not.toContainText(
    "A newer draft",
  );
  await capture(page, "blue-approvals");
  await page.getByRole("button", { name: "Request changes", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Feedback is required");
  expect(state.writes).toEqual([]);
  const queue = page.getByRole("navigation", { name: "Submissions" });
  await queue.getByRole("button", { name: /Team/ }).click();
  await expect(page.getByRole("region", { name: "Submitted preview" })).toContainText(
    "Meet the team",
  );
  state.failures.review = true;
  const feedback = page.getByRole("textbox", { name: /Feedback/ });
  await feedback.fill("Please describe each role.");
  await page.getByRole("button", { name: "Approve and publish", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Submission changed");
  await expect(feedback).toHaveValue("Please describe each role.");
  expect(state.writes[0]).toEqual({
    url: "/api/admin/reviews/team",
    body: {
      revision: 4,
      submission_id: "submission-team",
      approve: true,
      feedback: "Please describe each role.",
    },
  });
  state.failures.review = false;
  await page.getByRole("button", { name: "Request changes", exact: true }).click();
  await expect(queue.getByRole("button", { name: /Team/ })).toHaveCount(0);
  await page.getByRole("button", { name: /Changes requested \(1\)/ }).click();
  await expect(page.getByRole("complementary", { name: "Review decision" })).toContainText(
    "Please describe each role.",
  );
  await expect(page.getByRole("button", { name: "Approve and publish", exact: true })).toHaveCount(
    0,
  );
});

test("approval publishes with empty optional feedback and moves out of the pending queue", async ({
  page,
}) => {
  const state = await openWorkspace(page, bootstrap("reviewer"));
  await page.getByRole("button", { name: /Reviews .*pending/ }).click();
  await page.getByRole("button", { name: "Approve and publish", exact: true }).click();
  await expect(
    page.getByRole("navigation", { name: "Submissions" }).getByRole("button", { name: /Home/ }),
  ).toHaveCount(0);
  expect(state.writes[0].body).toEqual({
    revision: 12,
    submission_id: "submission-home",
    approve: true,
    feedback: "",
  });
  await page.getByRole("button", { name: /Approved \(1\)/ }).click();
  await expect(page.getByText("Approved and published", { exact: true })).toBeVisible();
});

test("templates preview each region's own allowlist and limits, then remain open for updates after creation", async ({
  page,
}) => {
  const state = await openWorkspace(page);
  await page.getByRole("button", { name: "Templates", exact: true }).click();
  await page.getByRole("textbox", { name: "Name", exact: true }).fill("Campaign page");
  await page
    .getByRole("textbox", { name: "Description", exact: true })
    .fill("Main content and a supporting callout.");
  await page.getByRole("spinbutton", { name: "Maximum components", exact: true }).fill("8");
  await page.getByRole("checkbox", { name: "Hero", exact: true }).check();
  await page.getByRole("checkbox", { name: "Cards", exact: true }).check();
  await page.getByRole("button", { name: "Add region", exact: true }).click();
  await page.getByRole("textbox", { name: "Region name", exact: true }).nth(1).fill("aside");
  await page.getByRole("spinbutton", { name: "Maximum components", exact: true }).nth(1).fill("2");
  await page.getByRole("checkbox", { name: "Callout", exact: true }).nth(1).check();
  const preview = page.getByRole("complementary", { name: "Structure preview" });
  await expect(preview.locator("section").nth(0)).toContainText("Up to 8 components");
  await expect(preview.locator("section").nth(0)).toContainText("Cards");
  await expect(preview.locator("section").nth(0)).not.toContainText("Callout");
  await expect(preview.locator("section").nth(1)).toContainText("Up to 2 components");
  await expect(preview.locator("section").nth(1)).not.toContainText("Hero");
  await capture(page, "blue-template");
  await page.getByRole("button", { name: "Save template", exact: true }).first().click();
  await expect(page.getByRole("heading", { name: "Campaign page", exact: true })).toBeVisible();
  expect(state.writes[0].body.regions).toEqual([
    { name: "main", allowed_components: ["hero", "cards"], max_components: 8 },
    { name: "aside", allowed_components: ["callout"], max_components: 2 },
  ]);
  await page.getByRole("textbox", { name: "Description", exact: true }).fill("Updated description");
  await page.getByRole("button", { name: "Save template", exact: true }).first().click();
  await expect(page.getByText("Saved", { exact: true })).toBeVisible();
  expect(state.writes.map((w) => w.url)).toEqual([
    "/api/admin/templates",
    "/api/admin/templates/campaign",
  ]);
});

test("admins export page branches and install packages as drafts", async ({ page }) => {
  const state = await openWorkspace(page);
  const uploads: string[] = [];
  let bootstraps = 0;
  await page.route("**/api/admin/bootstrap", (route) => {
    bootstraps++;
    return route.fulfill({ json: state.data });
  });
  await page.route("**/api/admin/package**", async (route) => {
    const req = route.request();
    if (req.method() === "GET") {
      expect(new URL(req.url()).searchParams.get("path")).toBe("/");
      return route.fulfill({
        body: "zip",
        headers: {
          "Content-Type": "application/zip",
          "Content-Disposition": 'attachment; filename="baddiecore-site.zip"',
        },
      });
    }
    uploads.push(req.headers()["content-type"]);
    if (uploads.length === 1)
      return route.fulfill({
        status: 409,
        json: { error: "a page or alias already uses this path" },
      });
    return route.fulfill({
      json: {
        created: ["/news", "/news/story"],
        updated: ["/about"],
        unchanged: [],
        components_added: 0,
        templates_added: 1,
      },
    });
  });
  const download = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export /", exact: true }).click();
  expect((await download).suggestedFilename()).toBe("baddiecore-site.zip");

  await page.getByRole("button", { name: "Import package", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Import a content package" });
  const install = dialog.getByRole("button", { name: "Install package", exact: true });
  await expect(install).toBeDisabled();
  await dialog.getByLabel("Package (.zip)").setInputFiles({
    name: "baddiecore-news.zip",
    mimeType: "application/zip",
    buffer: Buffer.from("zip"),
  });
  await install.click();
  await expect(dialog.getByRole("alert")).toHaveText("a page or alias already uses this path");
  await install.click();
  await expect(dialog.getByRole("status")).toContainText("Created 2: /news, /news/story");
  await expect(dialog.getByRole("status")).toContainText("Updated 1: /about");
  await expect(dialog.getByRole("status")).toContainText("Added 1 templates and 0 components");
  expect(uploads).toEqual(["application/zip", "application/zip"]);
  expect(bootstraps).toBe(1);
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await expect(dialog).toBeHidden();
});

test("deleting a template explains why it is still in use, then clears the editor", async ({
  page,
}) => {
  const state = await openWorkspace(page);
  const deletes: string[] = [];
  await page.route("**/api/admin/templates/landing", (route) => {
    deletes.push(route.request().method());
    if (deletes.length === 1)
      return route.fulfill({
        status: 409,
        json: { error: "template landing is still used by page /" },
      });
    state.data.templates = [];
    return route.fulfill({ status: 204 });
  });
  page.on("dialog", (dialog) => void dialog.accept());
  await page.getByRole("button", { name: "Expand Templates", exact: true }).click();
  await page.getByRole("button", { name: "Landing page", exact: true }).click();
  await page.getByRole("button", { name: "Delete", exact: true }).click();
  await expect(page.getByRole("alert")).toHaveText("template landing is still used by page /");
  await expect(page.getByRole("heading", { name: "Landing page", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Delete", exact: true }).click();
  await expect(page.getByRole("heading", { name: "New template", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Delete", exact: true })).toHaveCount(0);
  expect(deletes).toEqual(["DELETE", "DELETE"]);
});

test("editors cannot export or import packages", async ({ page }) => {
  await openWorkspace(page, bootstrap("editor"));
  await expect(page.getByRole("button", { name: "Import package" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: /^Export / })).toHaveCount(0);
});

test("editors cannot publish, decide reviews or edit schemas; submission saves the latest revision", async ({
  page,
}) => {
  const state = await openWorkspace(page, bootstrap("editor"));
  await expect(page.getByRole("region", { name: "System", exact: true })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Organization", exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "Home", exact: true }).click();
  await expect(page.getByRole("button", { name: "Publish", exact: true })).toHaveCount(0);
  await page.getByRole("textbox", { name: "Title *", exact: true }).fill("Ready for review");
  await page.getByRole("button", { name: "Submit for review", exact: true }).click();
  await expect(page.getByText("Saved draft · Revision 13", { exact: true })).toBeVisible();
  expect(state.writes[1]).toEqual({ url: "/api/admin/pages/home/submit", body: { revision: 13 } });
  await page.getByRole("button", { name: /Reviews .*pending/ }).click();
  await expect(page.getByRole("button", { name: "Approve and publish", exact: true })).toHaveCount(
    0,
  );
  await expect(page.getByRole("button", { name: "Request changes", exact: true })).toHaveCount(0);
});

test("narrow editor keeps navigation, fields and publish confirmation usable without horizontal overflow", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await openWorkspace(page);
  await page.getByRole("button", { name: "Navigation", exact: true }).click();
  await page.getByRole("button", { name: "Home", exact: true }).click();
  await expect(page.getByRole("navigation", { name: "Content navigation" })).toBeHidden();
  await expect(page.getByRole("textbox", { name: "Title *", exact: true })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await capture(page, "blue-narrow-editor");
  await page.getByRole("button", { name: "Publish", exact: true }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  const rect = await page.getByRole("dialog").boundingBox();
  expect(rect!.width).toBeLessThanOrEqual(390);
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).not.toBeVisible();
});
