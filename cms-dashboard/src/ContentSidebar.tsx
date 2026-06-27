import { createSelector, For, Match, Show, Switch } from "solid-js";
import type { ContentTreeNode, ContentTreeResponse } from "./cmsTypes";
import { ErrorNotice } from "./StatusNotice";

type ContentSidebarProps = {
  tree?: ContentTreeResponse;
  loading: boolean;
  error: unknown;
  selectedPath: string;
  onSelect: (path: string) => void;
  onRefresh: () => void;
};

function TreeNode(props: {
  item: ContentTreeNode;
  isSelected: (path: string) => boolean;
  onSelect: (path: string) => void;
}) {
  return (
    <div class="tree-node">
      <button
        type="button"
        classList={{ selected: props.isSelected(props.item.path) }}
        onClick={() => props.onSelect(props.item.path)}
      >
        <span>{props.item.name}</span>
        <small>{props.item.template} · {props.item.workflowState}</small>
      </button>
      <Show when={props.item.children.length > 0}>
        <div class="tree-children">
          <For each={props.item.children}>
            {(child) => <TreeNode item={child} isSelected={props.isSelected} onSelect={props.onSelect} />}
          </For>
        </div>
      </Show>
    </div>
  );
}

function ContentTree(props: {
  tree: ContentTreeResponse;
  isSelected: (path: string) => boolean;
  onSelect: (path: string) => void;
}) {
  return (
    <nav aria-label="Content tree">
      <For each={props.tree.items} fallback={<p class="notice">The content tree is empty.</p>}>
        {(item) => <TreeNode item={item} isSelected={props.isSelected} onSelect={props.onSelect} />}
      </For>
    </nav>
  );
}

function ContentSidebar(props: ContentSidebarProps) {
  const isSelected = createSelector(() => props.selectedPath);

  return (
    <aside class="content-panel">
      <div class="brand">
        <span class="brand-mark">B</span>
        <div><strong>Baddiecore</strong><small>CMS dashboard</small></div>
      </div>
      <div class="panel-heading">
        <span>Content</span>
        <button type="button" onClick={props.onRefresh}>Refresh</button>
      </div>
      <Switch>
        <Match when={props.loading}>
          <p class="notice">Loading content…</p>
        </Match>
        <Match when={props.error}>
          {(error) => <ErrorNotice error={error()} />}
        </Match>
        <Match when={props.tree}>
          {(tree) => <ContentTree tree={tree()} isSelected={isSelected} onSelect={props.onSelect} />}
        </Match>
      </Switch>
    </aside>
  );
}

export default ContentSidebar;
