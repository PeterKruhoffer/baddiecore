import assert from "node:assert/strict";
import test from "node:test";
import { searchContent } from "../web/src/admin/contentSearch.ts";

const components = [{ id: "hero", name: "Hero banner", description: "Opening promotion", fields: [] }];
const templates = [{ id: "article", name: "Article", description: "Editorial layout" }];
const pages = [
  { id: "home", title: "Welcome", slug: "/", blocks: [{ id: "intro", component_id: "hero", fields: { heading: "Visit Copenhagen", body: "Fresh bread every morning" } }] },
  { id: "guide", title: "Bakery guide", slug: "/guides/bread", blocks: [{ id: "details", component_id: "missing", fields: { body: "Rye sourdough", link: "/recipes/rye" } }] },
];
const search = (query, accessiblePages = pages) => searchContent(query, accessiblePages, components, templates);

test("blank search returns no results; title and path searches retain page identity", () => {
  assert.deepEqual(search(" \n "), []);
  assert.deepEqual(search(" WELCOME "), [{ kind: "page", pageId: "home", title: "Welcome", detail: "/" }]);
  assert.equal(search("/guides")[0].pageId, "guide");
  assert.deepEqual(search("unmatched"), []);
});

test("field text and component names target individual blocks, not just pages", () => {
  assert.deepEqual(search("COPENHAGEN").map(({ kind, pageId, blockId }) => ({ kind, pageId, blockId })), [{ kind: "block", pageId: "home", blockId: "intro" }]);
  assert.deepEqual(search("Hero").map((result) => result.kind), ["block", "components"]);
  assert.equal(search("recipes/rye")[0].blockId, "details");
  assert.equal(search("bread morning")[0].blockId, "intro");
  assert.equal(search("bakery sourdough")[0].blockId, "details");
  assert.deepEqual(search("Copenhagen sourdough"), []);
});

test("definitions search names and descriptions and carry their own IDs", () => {
  assert.deepEqual(search("editorial"), [{ kind: "templates", definitionId: "article", title: "Article", detail: "Editorial layout" }]);
  assert.equal(search("promotion")[0].definitionId, "hero");
});

test("editors can find component names on allowed pages without definition results", () => {
  assert.deepEqual(searchContent("hero", pages, components, templates, false).map((result) => [result.kind, result.pageId, result.blockId]), [["block", "home", "intro"]]);
  assert.deepEqual(searchContent("editorial", pages, components, templates, false), []);
  assert.deepEqual(searchContent("promotion", pages, components, templates, false), []);
});

test("search does not leak excluded pages, mutate input, or interpret markup", () => {
  const before = structuredClone(pages);
  assert.deepEqual(search("Copenhagen", [pages[1]]), []);
  assert.deepEqual(search("[.*"), []);
  const result = searchContent("script", [{ ...pages[0], blocks: [{ ...pages[0].blocks[0], fields: { body: "<script>alert(1)</script>" } }] }], [], []);
  assert.match(result[0].detail, /<script>/);
  assert.deepEqual(pages, before);
});

test("long content snippets include the matched text rather than just the beginning", () => {
  const result = searchContent("needle", [{ ...pages[0], blocks: [{ ...pages[0].blocks[0], fields: { body: `${"prefix ".repeat(80)}needle ${"suffix ".repeat(80)}` } }] }], components, []);
  assert.match(result[0].detail, /….*needle.*…/);
  assert.ok(result[0].detail.length < 160);
});

test("rich text searches visible copy across marks and icon labels, not JSON metadata", () => {
  const definitions = [{ ...components[0], fields: [{ name: "body", kind: "richtext", richtext: { icons: [{ id: "brand-star", label: "Site star", src: "/assets/star.svg" }] } }] }];
  const document = JSON.stringify({ type: "doc", content: [{ type: "paragraph", content: [
    { type: "text", text: "Copen" },
    { type: "text", text: "hagen", marks: [{ type: "bold" }] },
    { type: "hardBreak" },
    { type: "icon", attrs: { id: "brand-star" } },
  ] }] });
  const richPages = [{ ...pages[0], blocks: [{ ...pages[0].blocks[0], fields: { body: document } }] }];
  assert.equal(searchContent("copenhagen", richPages, definitions, [])[0].detail, "/ · Copenhagen Site star");
  assert.equal(searchContent("site star", richPages, definitions, [])[0].blockId, "intro");
  assert.deepEqual(searchContent("paragraph", richPages, definitions, []), []);
  assert.deepEqual(searchContent("brand-star", richPages, definitions, []), []);
});
