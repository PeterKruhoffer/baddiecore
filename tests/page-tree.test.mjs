import assert from "node:assert/strict";
import test from "node:test";
import { buildPageTree, joinPath, pageParentPaths, parentPath, pathSegment } from "../web/src/admin/pageTree.ts";

test("tree keeps page identity, virtual ancestors, and prefix siblings separate", () => {
  const pages = [
    { id: "child", slug: "/about/team" },
    { id: "near", slug: "/about-us" },
    { id: "virtual", slug: "/news/2026/launch" },
    { id: "root", slug: "/" },
    { id: "parent", slug: "/about" },
  ];
  const root = buildPageTree(pages);
  assert.equal(root.page.id, "root");
  assert.deepEqual(root.children.map((n) => n.path), ["/about", "/about-us", "/news"]);
  assert.equal(root.children[0].page.id, "parent");
  assert.equal(root.children[0].children[0].page.id, "child");
  assert.equal(root.children[2].page, undefined);
  assert.equal(root.children[2].children[0].children[0].page.id, "virtual");
  assert.equal(pages[0].id, "child");
});

test("parent options include missing path prefixes once", () => {
  assert.deepEqual(pageParentPaths([{ slug: "/docs/start" }, { slug: "/docs/api" }]), ["/", "/docs", "/docs/api", "/docs/start"]);
  assert.equal(parentPath("/about/team"), "/about");
  assert.equal(parentPath("/about"), "/");
  assert.equal(pathSegment("/about/team"), "team");
  assert.equal(joinPath("/", "about"), "/about");
  assert.equal(joinPath("/about", "team"), "/about/team");
  assert.equal(buildPageTree([]).page, undefined);
});
