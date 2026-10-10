import { expect, test, type Page as BrowserPage } from "@playwright/test";
import { bootstrap } from "./admin-fixture";
import type { Bootstrap, ComponentDef, Page, Template } from "../src/types";

const organization = {
  revision: 1,
  members: [{ id: "admin", name: "Admin", role: "admin", paths: [], groups: [] }],
  groups: [],
};

async function openWorkspace(page: BrowserPage, data: Bootstrap = bootstrap()) {
  const writes: { url: string; body: Record<string, any> }[] = [];
  const failures = { save: false, publish: false, review: false };
  await page.route("**/api/**", async (route) => {
    const req = route.request();
    const url = new URL(req.url()).pathname;
    const body = req.method() === "GET" ? undefined : req.postDataJSON();
    if (body) writes.push({ url, body });
    if (url === "/api/admin/bootstrap") return route.fulfill({ json: data });
    if (url === "/api/auth/config") return route.fulfill({ json: { method: "password" } });
    if (url === "/api/account/password")
      return body.current_password === "old-password"
        ? route.fulfill({ status: 204 })
        : route.fulfill({ status: 403, json: { error: "current password is incorrect" } });
    if (url === "/api/admin/organization")
      return route.fulfill({
        json: req.method() === "PUT" ? { ...body, revision: body.revision + 1 } : organization,
      });
    if (/^\/api\/admin\/members\/[^/]+\/password$/.test(url)) return route.fulfill({ status: 204 });
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
    if (url === "/api/admin/components/hero" && req.method() === "PUT") {
      data.components[0] = body as ComponentDef;
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

test("members change their own password and admins add members with passwords", async ({
  page,
}) => {
  const { writes } = await openWorkspace(page);
  await page.getByRole("button", { name: "Account", exact: true }).click();
  await page.getByLabel("Current password").fill("wrong-password");
  await page.getByLabel("New password", { exact: true }).fill("new-password");
  await page.getByLabel("Repeat new password").fill("other-password");
  await page.getByRole("button", { name: "Change password" }).click();
  await expect(page.getByRole("alert")).toHaveText("The new passwords do not match.");
  await page.getByLabel("Repeat new password").fill("new-password");
  await page.getByRole("button", { name: "Change password" }).click();
  await expect(page.getByRole("alert")).toHaveText("current password is incorrect");
  await page.getByLabel("Current password").fill("old-password");
  await page.getByRole("button", { name: "Change password" }).click();
  await expect(page.getByRole("status")).toHaveText("Password changed.");
  expect(writes.at(-1)).toEqual({
    url: "/api/account/password",
    body: { current_password: "old-password", password: "new-password" },
  });

  await page.getByRole("button", { name: "Organization", exact: true }).click();
  const add = page.locator("form").filter({ hasText: "Add member" });
  await add.getByLabel("Username").fill("casey");
  await add.getByLabel("Display name").fill("Casey");
  await add.getByLabel("Password").fill("casey-password");
  await add.getByRole("button", { name: "Save member" }).click();
  await expect.poll(() => writes.at(-1)?.url).toBe("/api/admin/members/casey/password");
  expect(writes.at(-2)?.body.members.map((m: { id: string }) => m.id)).toEqual(["admin", "casey"]);
  expect(writes.at(-1)?.body).toEqual({ password: "casey-password" });
});

test("rich text preserves legacy copy, formatting, links, icons and undo across saves and selection", async ({
  page,
}) => {
  const data = bootstrap();
  data.components[0].fields[2].kind = "richtext";
  data.components[0].fields[2].richtext = {
    icons: [{ id: "star", label: "Site star", src: "/assets/star.svg" }],
  };
  await page.route("**/assets/star.svg", (route) =>
    route.fulfill({
      contentType: "image/svg+xml",
      body: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#2563eb" d="m12 1 3 7 8 1-6 5 2 8-7-4-7 4 2-8-6-5 8-1z"/></svg>',
    }),
  );
  const state = await openWorkspace(page, data);
  await page.getByRole("button", { name: "Home", exact: true }).click();
  const body = page.getByRole("textbox", { name: "Body", exact: true });
  const preview = page.getByRole("main");
  await expect(body).toHaveText("Build, edit and publish with a CMS you own.");
  await body.fill("A CMS you own");
  await body.pressSequentially(".");
  await expect(body).toBeFocused();
  await body.press("Control+a");
  await page.getByRole("button", { name: "Bold", exact: true }).click();
  await expect(preview.locator("strong")).toHaveText("A CMS you own.");
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(preview.locator("strong")).toHaveCount(0);
  await page.getByRole("button", { name: "Redo", exact: true }).click();
  await expect(preview.locator("strong")).toHaveText("A CMS you own.");
  await page.getByRole("button", { name: "Link", exact: true }).click();
  await page.getByRole("radio", { name: "External link", exact: true }).check();
  await page.getByRole("textbox", { name: "Link URL", exact: true }).fill("javascript:alert(1)");
  await page.getByRole("button", { name: "Apply link", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Enter a full http:// or https:// URL");
  if (process.env.UI_SCREENSHOTS_DIR)
    await page
      .getByRole("group", { name: "Body formatting" })
      .locator("..")
      .screenshot({ path: `${process.env.UI_SCREENSHOTS_DIR}/richtext-link-error.png` });
  await page.getByRole("radio", { name: "Site link", exact: true }).check();
  await page.getByRole("combobox", { name: "Site page", exact: true }).selectOption("custom");
  await page.getByRole("textbox", { name: "Link URL", exact: true }).fill("/about");
  await page.getByRole("button", { name: "Apply link", exact: true }).click();
  await expect(preview.getByRole("link", { name: "A CMS you own." })).toHaveAttribute(
    "href",
    "/about",
  );
  await page.getByRole("combobox", { name: "Text style" }).selectOption("3");
  await expect(preview.getByRole("heading", { name: "A CMS you own.", level: 3 })).toBeVisible();
  await page.getByRole("combobox", { name: "Text style" }).selectOption("paragraph");
  await expect(preview.getByRole("heading", { name: "A CMS you own.", level: 3 })).toHaveCount(0);
  await expect(body).toBeFocused();
  await body.locator("strong").click();
  await body.press("End");
  await expect
    .poll(() =>
      body.evaluate((element) => {
        const selection = window.getSelection();
        const text = element.querySelector("strong")?.firstChild;
        return (
          selection?.isCollapsed &&
          selection.anchorNode === text &&
          selection.anchorOffset === text?.textContent?.length
        );
      }),
    )
    .toBe(true);
  const iconPicker = page.getByRole("combobox", { name: "Insert site icon" });
  await iconPicker.focus();
  await iconPicker.selectOption("star");
  await expect(preview.getByRole("img", { name: "Site star" })).toBeVisible();
  await expect(body).toBeFocused();
  await page.getByRole("button", { name: "Bullet list", exact: true }).click();
  await expect(preview.locator("ul li")).toContainText("A CMS you own.");
  await page.getByRole("button", { name: "Save draft", exact: true }).click();
  await expect(page.getByText("Saved draft · Revision 13", { exact: true })).toBeVisible();
  const doc = JSON.parse(state.writes[0].body.blocks[0].fields.body);
  expect(doc.content[0].type).toBe("bulletList");
  expect(doc.content[0].content[0].content[0].content).toContainEqual({
    type: "icon",
    attrs: { id: "star" },
  });
  await capture(page, "richtext-editor");
  await preview.getByRole("heading", { name: "Built around your content." }).click();
  await preview.getByRole("heading", { name: "A home for your next idea." }).click();
  await expect(body.locator("strong")).toHaveText("A CMS you own.");
  await expect(body.getByRole("img", { name: "Site star" })).toBeVisible();
  await page.reload();
  await page.getByRole("button", { name: "Home", exact: true }).click();
  await expect(body.locator("strong")).toHaveText("A CMS you own.");
});

test("editors can pick a site page, add an external link, and edit or remove links without changing other text", async ({
  page,
}) => {
  const data = bootstrap("editor");
  data.components[0].fields[2].kind = "richtext";
  data.pages[0].blocks[0].fields.body = "Meet our team. Read the guide.";
  await openWorkspace(page, data);
  await page.getByRole("button", { name: "Home", exact: true }).click();
  const body = page.getByRole("textbox", { name: "Body", exact: true });
  await expect(body).toHaveText("Meet our team. Read the guide.");
  const selectText = async (text: string) => {
    await body.focus();
    await body.evaluate((element, text) => {
      const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
      while (walker.nextNode()) {
        const node = walker.currentNode;
        const start = node.textContent!.indexOf(text);
        if (start < 0) continue;
        window.getSelection()!.setBaseAndExtent(node, start, node, start + text.length);
        return;
      }
      throw new Error(`Text not found: ${text}`);
    }, text);
    await expect.poll(() => page.evaluate(() => window.getSelection()?.toString())).toBe(text);
  };
  await selectText("our team");
  await page.getByRole("button", { name: "Link", exact: true }).click();
  const picker = page.getByRole("combobox", { name: "Site page", exact: true });
  await picker.selectOption("/about/team");
  await expect(page.getByText("Publish this page before readers can visit it.")).toBeVisible();
  await page.getByRole("button", { name: "Apply link", exact: true }).click();
  await expect(body.getByRole("link", { name: "our team", exact: true })).toHaveAttribute(
    "href",
    "/about/team",
  );
  await selectText("the guide");
  await page.getByRole("button", { name: "Link", exact: true }).click();
  await page.getByRole("radio", { name: "External link", exact: true }).check();
  await page
    .getByRole("textbox", { name: "Link URL", exact: true })
    .fill(" https://example.org/guide?topic=cms#editing ");
  await page.getByRole("button", { name: "Apply link", exact: true }).click();
  await expect(body.getByRole("link", { name: "the guide", exact: true })).toHaveAttribute(
    "href",
    "https://example.org/guide?topic=cms#editing",
  );
  await page.getByRole("button", { name: "Save draft", exact: true }).click();
  await expect(page.getByText("Saved draft · Revision 13", { exact: true })).toBeVisible();
  await page.reload();
  await page.getByRole("button", { name: "Home", exact: true }).click();
  await expect(body.getByRole("link", { name: "our team", exact: true })).toHaveAttribute(
    "href",
    "/about/team",
  );
  await expect(body.getByRole("link", { name: "the guide", exact: true })).toHaveAttribute(
    "href",
    "https://example.org/guide?topic=cms#editing",
  );
  await body.getByRole("link", { name: "our team", exact: true }).click();
  await page.getByRole("button", { name: "Link", exact: true }).click();
  await expect(picker).toHaveValue("/about/team");
  await picker.selectOption("/");
  await page.getByRole("button", { name: "Apply link", exact: true }).click();
  await expect(body.getByRole("link", { name: "our team", exact: true })).toHaveAttribute(
    "href",
    "/",
  );
  await body.getByRole("link", { name: "the guide", exact: true }).click();
  await page.getByRole("button", { name: "Link", exact: true }).click();
  await expect(page.getByRole("radio", { name: "External link", exact: true })).toBeChecked();
  await expect(page.getByRole("textbox", { name: "Link URL", exact: true })).toHaveValue(
    "https://example.org/guide?topic=cms#editing",
  );
  await page
    .getByRole("textbox", { name: "Link URL", exact: true })
    .fill("https://example.org/changed");
  await page.keyboard.press("Escape");
  await expect(page.getByRole("group", { name: "Edit link" })).toHaveCount(0);
  await expect(body).toBeFocused();
  await expect(body.getByRole("link", { name: "the guide", exact: true })).toHaveAttribute(
    "href",
    "https://example.org/guide?topic=cms#editing",
  );
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole("button", { name: "Link", exact: true }).click();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.getByRole("button", { name: "Remove link", exact: true }).click();
  await expect(body.getByRole("link", { name: "the guide", exact: true })).toHaveCount(0);
  await expect(body.getByRole("link", { name: "our team", exact: true })).toHaveAttribute(
    "href",
    "/",
  );
  await expect(body).toHaveText("Meet our team. Read the guide.");
});

test("rich text configuration saves disabled controls and site icons without losing field identity", async ({
  page,
}) => {
  const state = await openWorkspace(page);
  await page.getByRole("button", { name: "Expand Components" }).click();
  await page.getByRole("button", { name: "Hero", exact: true }).click();
  await page.getByRole("combobox", { name: "Type", exact: true }).nth(2).selectOption("richtext");
  const options = page.getByRole("group", { name: "Body rich text options" });
  await options.getByRole("checkbox", { name: "Headings", exact: true }).uncheck();
  await options.getByRole("checkbox", { name: "Strikethrough", exact: true }).uncheck();
  await options.getByRole("button", { name: "Add site icon" }).click();
  await options.getByRole("textbox", { name: "Icon ID" }).fill("brand-star");
  await options.getByRole("textbox", { name: "Icon label" }).fill("Brand star");
  await options.getByRole("textbox", { name: "Icon image URL" }).fill("/assets/icons/star.svg");
  if (process.env.UI_SCREENSHOTS_DIR)
    await options
      .locator("..")
      .screenshot({ path: `${process.env.UI_SCREENSHOTS_DIR}/richtext-settings.png` });
  await page.getByRole("button", { name: "Save component", exact: true }).first().click();
  await expect(page.getByText("Saved", { exact: true })).toBeVisible();
  expect(state.writes[0].body.fields[2]).toEqual({
    name: "body",
    label: "Body",
    kind: "richtext",
    required: false,
    richtext: {
      features: [
        "bold",
        "italic",
        "underline",
        "bullet_list",
        "ordered_list",
        "blockquote",
        "link",
      ],
      icons: [{ id: "brand-star", label: "Brand star", src: "/assets/icons/star.svg" }],
    },
  });
  await page.getByRole("button", { name: "Home", exact: true }).click();
  await expect(page.getByRole("combobox", { name: "Text style", exact: true })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Strikethrough", exact: true })).toHaveCount(0);
  await expect(
    page.getByRole("combobox", { name: "Insert site icon" }).locator("option"),
  ).toHaveText(["Site icon", "Brand star"]);
});

test("restricted rich text removes disabled paste formatting and shortcuts and fits narrow screens", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const data = bootstrap();
  data.components[0].fields[2] = {
    ...data.components[0].fields[2],
    kind: "richtext",
    richtext: { features: ["italic"], icons: [] },
  };
  const state = await openWorkspace(page, data);
  await page.getByRole("button", { name: "Navigation", exact: true }).click();
  await page.getByRole("button", { name: "Home", exact: true }).click();
  const body = page.getByRole("textbox", { name: "Body", exact: true });
  await expect(page.getByRole("button", { name: "Bold", exact: true })).toHaveCount(0);
  await body.fill("Plain copy");
  await body.press("Control+a");
  await body.press("Control+b");
  await expect(body.locator("strong")).toHaveCount(0);
  await body.evaluate((element) => {
    const clipboardData = new DataTransfer();
    clipboardData.setData(
      "text/html",
      '<p><strong>Pasted bold</strong> <em>and italic</em> <a href="javascript:alert(1)">unsafe link</a><img src="/unregistered.svg"></p>',
    );
    element.dispatchEvent(
      new ClipboardEvent("paste", { clipboardData, bubbles: true, cancelable: true }),
    );
  });
  await expect(body).toHaveText("Pasted bold and italic unsafe link");
  await expect(body.locator("em")).toHaveText("and italic");
  await expect(body.locator("strong, a, img")).toHaveCount(0);
  await page.getByRole("button", { name: "Save draft", exact: true }).click();
  await expect(page.getByText("Saved draft · Revision 13", { exact: true })).toBeVisible();
  expect(JSON.parse(state.writes[0].body.blocks[0].fields.body).content[0].content).toEqual([
    { type: "text", text: "Pasted bold " },
    { type: "text", text: "and italic", marks: [{ type: "italic" }] },
    { type: "text", text: " unsafe link" },
  ]);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await body.focus();
  await body.evaluate((element) => element.scrollIntoView({ block: "center" }));
  await expect(body).toBeFocused();
  if (process.env.UI_SCREENSHOTS_DIR)
    await page.screenshot({ path: `${process.env.UI_SCREENSHOTS_DIR}/richtext-narrow.png` });
});

test("published rich text renders semantic content and escapes literal HTML", async ({ page }) => {
  const data = bootstrap();
  const field = data.components[0].fields[2];
  field.kind = "richtext";
  field.richtext = { icons: [{ id: "star", label: "Site star", src: "/assets/star.svg" }] };
  data.pages[0].blocks[0].fields.body = JSON.stringify({
    type: "doc",
    content: [
      {
        type: "heading",
        attrs: { level: 3 },
        content: [{ type: "text", text: "Made for your site" }],
      },
      {
        type: "paragraph",
        content: [
          {
            type: "text",
            text: "Own your content",
            marks: [
              { type: "bold" },
              { type: "italic" },
              { type: "link", attrs: { href: "/about" } },
            ],
          },
          { type: "text", text: " " },
          { type: "icon", attrs: { id: "star" } },
        ],
      },
      {
        type: "orderedList",
        attrs: { start: 4, type: "a" },
        content: [
          {
            type: "listItem",
            content: [
              { type: "paragraph", content: [{ type: "text", text: "Choose your formatting" }] },
            ],
          },
        ],
      },
      {
        type: "blockquote",
        content: [
          {
            type: "paragraph",
            content: [{ type: "text", text: '<script>alert("literal")</script>' }],
          },
        ],
      },
    ],
  });
  await page.route("**/api/content?**", (route) =>
    route.fulfill({
      json: { page: data.pages[0], template: data.templates[0], components: data.components },
    }),
  );
  await page.route("**/assets/star.svg", (route) =>
    route.fulfill({
      contentType: "image/svg+xml",
      body: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#2563eb" d="m12 1 3 7 8 1-6 5 2 8-7-4-7 4 2-8-6-5 8-1z"/></svg>',
    }),
  );
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Made for your site", level: 3 })).toBeVisible();
  await expect(page.locator("a em strong")).toHaveText("Own your content");
  await expect(page.locator("ol")).toHaveAttribute("start", "4");
  await expect(page.locator("ol")).toHaveAttribute("type", "a");
  await expect(page.locator("blockquote")).toHaveText('<script>alert("literal")</script>');
  await expect(page.locator("main script")).toHaveCount(0);
  await expect(page.getByRole("img", { name: "Site star" })).toBeVisible();
  await capture(page, "richtext-public");
});
