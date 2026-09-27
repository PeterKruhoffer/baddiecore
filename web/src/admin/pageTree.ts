import type { Page } from "../types";

export type PageTreeNode = {
  path: string;
  name: string;
  page?: Page;
  children: PageTreeNode[];
};

export function parentPath(path: string) {
  if (path === "/") return "/";
  const parts = path.split("/").filter(Boolean);
  parts.pop();
  return parts.length ? `/${parts.join("/")}` : "/";
}

export function pathSegment(path: string) {
  return path === "/" ? "" : (path.split("/").filter(Boolean).pop() ?? "");
}

export function joinPath(parent: string, segment: string) {
  return `${parent === "/" ? "" : parent}/${segment}`;
}

export function buildPageTree(pages: Page[]) {
  const root: PageTreeNode = { path: "/", name: "/", children: [] };
  const nodes = new Map<string, PageTreeNode>([["/", root]]);

  for (const page of [...pages].sort((a, b) => a.slug.localeCompare(b.slug))) {
    if (page.slug === "/") {
      root.page = page;
      continue;
    }
    let current = root;
    let path = "";
    for (const part of page.slug.split("/").filter(Boolean)) {
      path += `/${part}`;
      let node = nodes.get(path);
      if (!node) {
        node = { path, name: part, children: [] };
        nodes.set(path, node);
        current.children.push(node);
      }
      current = node;
    }
    current.page = page;
  }

  const sort = (node: PageTreeNode) => {
    node.children.sort((a, b) => a.name.localeCompare(b.name));
    node.children.forEach(sort);
  };
  sort(root);
  return root;
}

export function pageParentPaths(pages: Page[]) {
  const paths = new Set<string>(["/"]);
  for (const page of pages) {
    let path = "";
    for (const part of page.slug.split("/").filter(Boolean)) {
      path += `/${part}`;
      paths.add(path);
    }
  }
  return [...paths].sort((a, b) => a.localeCompare(b));
}
