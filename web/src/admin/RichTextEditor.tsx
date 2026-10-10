import { createEffect, createSignal, createUniqueId, For, Show } from "solid-js";
import { Editor, Node } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import * as stylex from "@stylexjs/stylex";
import type { Field, Page, RichTextFeature } from "../types";
import {
  defaultRichTextFeatures,
  richTextDocument,
  richTextFeatures,
  safeHref,
} from "../lib/richtext";
import { richTextStyles } from "../components/RichText";
import { common } from "../common.stylex";

const styles = stylex.create({
  frame: {
    backgroundColor: "#fff",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#dce2ea",
    borderRadius: 4,
    position: "relative",
  },
  toolbar: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    gap: "6px 0",
    padding: 6,
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "#dce2ea",
    backgroundColor: "#f8fafc",
    borderTopLeftRadius: 4,
    borderTopRightRadius: 4,
  },
  group: {
    display: "flex",
    alignItems: "center",
    gap: 2,
    paddingInline: 6,
    borderRightWidth: 1,
    borderRightStyle: "solid",
    borderRightColor: "#dce2ea",
  },
  history: { borderRightWidth: 0, marginLeft: "auto" },
  button: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    width: 36,
    height: 36,
    padding: 0,
    fontSize: 18,
    borderWidth: 0,
    backgroundColor: { default: "transparent", ":hover": "#e2e8f0" },
    outlineColor: "#2563eb",
    outlineOffset: 2,
    opacity: { default: 1, ":disabled": 0.35 },
    cursor: { default: "pointer", ":disabled": "default" },
  },
  active: { backgroundColor: "#dbeafe", color: "#1d4ed8" },
  glyph: { fontWeight: 700 },
  italic: { fontFamily: "Georgia, serif", fontStyle: "italic" },
  underline: { textDecoration: "underline" },
  strike: { textDecoration: "line-through" },
  input: {
    minHeight: 140,
    padding: 12,
    fontSize: 13,
    outlineStyle: { default: "none", ":focus-visible": "solid" },
    outlineColor: "#2563eb",
    outlineWidth: 2,
    outlineOffset: -2,
  },
  linkForm: {
    position: "absolute",
    top: "calc(100% + 6px)",
    right: 0,
    zIndex: 20,
    width: 340,
    maxWidth: "100%",
    padding: 16,
    backgroundColor: "#fff",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#dce2ea",
    borderRadius: 6,
    boxShadow: "0 8px 24px #0f172a26",
    fontSize: 13,
    fontWeight: 400,
  },
  toolbarWrap: { position: "relative" },
  linkTitle: { margin: "0 0 12px", fontSize: 14, fontWeight: 600 },
  linkChoices: { display: "flex", gap: 16 },
  choice: { display: "flex", alignItems: "center", gap: 6 },
  hint: { color: "#64748b", fontSize: 12, overflowWrap: "anywhere" },
  actions: { display: "flex", flexWrap: "wrap", gap: 6, marginTop: 14 },
  action: { padding: "7px 10px", fontSize: 12 },
  select: {
    width: "auto",
    maxWidth: "100%",
    padding: "7px 4px",
    height: 36,
    backgroundColor: "transparent",
    borderWidth: 0,
    fontSize: 12,
  },
});

function ToolIcon(p: { name: string }) {
  const paths: Record<string, string> = {
    bullet_list: "M9 6h11M9 12h11M9 18h11M4 6h.01M4 12h.01M4 18h.01",
    ordered_list: "M10 6h10M10 12h10M10 18h10M3 4h1v5M3 9h2M3 13c0-2 3-2 3 0 0 1-3 2-3 4h3",
    blockquote: "M9 5H4v7h5v-7ZM4 12c0 4 2 6 5 7M20 5h-5v7h5v-7ZM15 12c0 4 2 6 5 7",
    link: "m10 13 4-4M8 16l-1 1a4 4 0 0 1-6-6l4-4a4 4 0 0 1 6 0M16 8l1-1a4 4 0 0 1 6 6l-4 4a4 4 0 0 1-6 0",
    undo: "M9 5 4 10l5 5M4 10h10a6 6 0 0 1 0 12",
    redo: "m15 5 5 5-5 5M20 10H10a6 6 0 0 0 0 12",
  };
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.8"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      <path d={paths[p.name]} />
    </svg>
  );
}

export function RichTextEditor(p: {
  field: Field;
  pages: Page[];
  value: string;
  onChange: (value: string) => void;
}) {
  let host!: HTMLDivElement;
  const [editor, setEditor] = createSignal<Editor>();
  const [version, setVersion] = createSignal(0);
  const [showLink, setShowLink] = createSignal(false);
  const [linkKind, setLinkKind] = createSignal<"internal" | "external">("internal");
  const [sitePage, setSitePage] = createSignal("");
  const [href, setHref] = createSignal("");
  const [error, setError] = createSignal("");
  let linkPanel!: HTMLDivElement;
  const linkGroup = createUniqueId();
  const glyphs: Partial<Record<RichTextFeature, string>> = {
    bold: "B",
    italic: "I",
    underline: "U",
    strike: "S",
  };
  const features = () => p.field.richtext?.features ?? defaultRichTextFeatures;
  const icons = () => p.field.richtext?.icons ?? [];
  const enabled = (feature: RichTextFeature) => features().includes(feature);

  createEffect(
    () => p.field.richtext,
    () => {
      const icon = Node.create({
        name: "icon",
        inline: true,
        group: "inline",
        atom: true,
        addAttributes: () => ({ id: { default: null } }),
        parseHTML: () => [
          {
            tag: "img[data-site-icon]",
            getAttrs: (element) => {
              const id = element.getAttribute("data-site-icon");
              return icons().some((icon) => icon.id === id) ? { id } : false;
            },
          },
        ],
        renderHTML: ({ node }) => {
          const icon = icons().find((icon) => icon.id === node.attrs.id);
          return [
            "img",
            {
              "data-site-icon": icon?.id,
              src: safeHref(icon?.src),
              alt: icon?.label ?? "",
              ...stylex.attrs(richTextStyles.icon),
            },
          ];
        },
      });
      const instance = new Editor({
        element: host,
        extensions: [
          StarterKit.configure({
            bold: enabled("bold") ? {} : false,
            italic: enabled("italic") ? {} : false,
            underline: enabled("underline") ? {} : false,
            strike: enabled("strike") ? {} : false,
            paragraph: { HTMLAttributes: stylex.attrs(richTextStyles.paragraph) },
            heading: enabled("heading")
              ? { levels: [2, 3], HTMLAttributes: stylex.attrs(richTextStyles.heading) }
              : false,
            bulletList: enabled("bullet_list") ? {} : false,
            orderedList: enabled("ordered_list") ? {} : false,
            listItem: enabled("bullet_list") || enabled("ordered_list") ? {} : false,
            listKeymap: enabled("bullet_list") || enabled("ordered_list") ? {} : false,
            blockquote: enabled("blockquote")
              ? { HTMLAttributes: stylex.attrs(richTextStyles.quote) }
              : false,
            link: enabled("link")
              ? { openOnClick: false, isAllowedUri: (url) => !!safeHref(url) }
              : false,
            code: false,
            codeBlock: false,
            horizontalRule: false,
          }),
          ...(icons().length ? [icon] : []),
        ],
        content: richTextDocument(p.value),
        editorProps: {
          attributes: {
            ...stylex.attrs(richTextStyles.content, styles.input),
            role: "textbox",
            "aria-label": p.field.label,
            "aria-multiline": "true",
            "aria-required": String(p.field.required),
          },
        },
        onUpdate: ({ editor }) => p.onChange(JSON.stringify(editor.getJSON())),
        onTransaction: () => setVersion((value) => value + 1),
      });
      setEditor(instance);
      return () => instance.destroy();
    },
  );
  createEffect(
    () => ({ value: p.value, editor: editor() }),
    ({ value, editor }) => {
      if (editor && JSON.stringify(editor.getJSON()) !== JSON.stringify(richTextDocument(value))) {
        editor.commands.setContent(richTextDocument(value), { emitUpdate: false });
      }
    },
  );
  function active(feature: RichTextFeature) {
    version();
    const name =
      feature === "bullet_list"
        ? "bulletList"
        : feature === "ordered_list"
          ? "orderedList"
          : feature;
    return editor()?.isActive(name) ?? false;
  }
  function toggle(feature: RichTextFeature) {
    const chain = editor()?.chain().focus();
    if (!chain) return;
    switch (feature) {
      case "bold":
        chain.toggleBold().run();
        break;
      case "italic":
        chain.toggleItalic().run();
        break;
      case "underline":
        chain.toggleUnderline().run();
        break;
      case "strike":
        chain.toggleStrike().run();
        break;
      case "heading":
        chain.toggleHeading({ level: 2 }).run();
        break;
      case "bullet_list":
        chain.toggleBulletList().run();
        break;
      case "ordered_list":
        chain.toggleOrderedList().run();
        break;
      case "blockquote":
        chain.toggleBlockquote().run();
        break;
      case "link":
        const current = editor()?.getAttributes("link").href ?? "";
        setHref(current);
        setLinkKind(current && !current.startsWith("/") ? "external" : "internal");
        setSitePage(
          p.pages.some((page) => page.slug === current) ? current : current ? "custom" : "",
        );
        setError("");
        setShowLink(!showLink());
        if (showLink())
          queueMicrotask(() => linkPanel.querySelector<HTMLInputElement>("input:checked")?.focus());
        break;
    }
  }
  function closeLink() {
    setShowLink(false);
    editor()?.commands.focus();
  }
  function applyLink() {
    const value = href().trim();
    if (
      !safeHref(value) ||
      (linkKind() === "internal" ? !value.startsWith("/") : !/^https?:\/\//i.test(value))
    ) {
      setError(
        linkKind() === "internal"
          ? "Choose a page or enter a site path starting with /."
          : "Enter a full http:// or https:// URL.",
      );
      return;
    }
    editor()?.chain().focus().extendMarkRange("link").setLink({ href: value }).run();
    setShowLink(false);
  }
  const tool = (feature: { id: RichTextFeature; label: string }) => (
    <button
      type="button"
      {...stylex.attrs(
        common.button,
        styles.button,
        (active(feature.id) || (feature.id === "link" && showLink())) && styles.active,
      )}
      aria-label={feature.label}
      title={feature.label}
      aria-pressed={active(feature.id) ? "true" : "false"}
      aria-expanded={feature.id === "link" ? (showLink() ? "true" : "false") : undefined}
      onMouseDown={(e) => e.preventDefault()}
      onClick={() => toggle(feature.id)}
    >
      <Show when={glyphs[feature.id]} fallback={<ToolIcon name={feature.id} />}>
        <span
          aria-hidden="true"
          {...stylex.attrs(
            styles.glyph,
            feature.id === "italic" && styles.italic,
            feature.id === "underline" && styles.underline,
            feature.id === "strike" && styles.strike,
          )}
        >
          {glyphs[feature.id]}
        </span>
      </Show>
    </button>
  );
  return (
    <div {...stylex.attrs(styles.frame)}>
      <div
        {...stylex.attrs(styles.toolbarWrap)}
        onFocusOut={(e) => {
          if (e.relatedTarget instanceof HTMLElement && !e.currentTarget.contains(e.relatedTarget))
            setShowLink(false);
        }}
      >
        <div
          {...stylex.attrs(styles.toolbar)}
          role="group"
          aria-label={`${p.field.label} formatting`}
        >
          <Show when={enabled("heading")}>
            <div {...stylex.attrs(styles.group)}>
              <select
                {...stylex.attrs(common.control, styles.select)}
                aria-label="Text style"
                value={
                  (version(),
                  editor()?.isActive("heading", { level: 2 })
                    ? "2"
                    : editor()?.isActive("heading", { level: 3 })
                      ? "3"
                      : "paragraph")
                }
                onChange={(e) => {
                  const value = e.currentTarget.value;
                  const chain = editor()?.chain().focus();
                  if (value === "paragraph") chain?.setParagraph().run();
                  else chain?.setHeading({ level: Number(value) as 2 | 3 }).run();
                }}
              >
                <option value="paragraph">Paragraph</option>
                <option value="2">Heading 2</option>
                <option value="3">Heading 3</option>
              </select>
            </div>
          </Show>
          <For
            each={[
              ["bold", "italic", "underline", "strike"],
              ["bullet_list", "ordered_list", "blockquote"],
              ["link"],
            ]}
          >
            {(group) => (
              <Show when={group.some((feature) => enabled(feature as RichTextFeature))}>
                <div {...stylex.attrs(styles.group)}>
                  <For
                    each={richTextFeatures.filter(
                      (feature) => group.includes(feature.id) && enabled(feature.id),
                    )}
                  >
                    {tool}
                  </For>
                </div>
              </Show>
            )}
          </For>
          <Show when={icons().length}>
            <div {...stylex.attrs(styles.group)}>
              <select
                {...stylex.attrs(common.control, styles.select)}
                aria-label="Insert site icon"
                value=""
                onChange={(e) => {
                  if (!e.currentTarget.value) return;
                  editor()
                    ?.chain()
                    .focus()
                    .insertContent({ type: "icon", attrs: { id: e.currentTarget.value } })
                    .run();
                  e.currentTarget.value = "";
                }}
              >
                <option value="">Site icon</option>
                <For each={icons()}>{(icon) => <option value={icon.id}>{icon.label}</option>}</For>
              </select>
            </div>
          </Show>
          <div {...stylex.attrs(styles.group, styles.history)}>
            <button
              type="button"
              {...stylex.attrs(common.button, styles.button)}
              aria-label="Undo"
              title="Undo"
              disabled={(version(), !editor()?.can().undo())}
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => editor()?.chain().focus().undo().run()}
            >
              <ToolIcon name="undo" />
            </button>
            <button
              type="button"
              {...stylex.attrs(common.button, styles.button)}
              aria-label="Redo"
              title="Redo"
              disabled={(version(), !editor()?.can().redo())}
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => editor()?.chain().focus().redo().run()}
            >
              <ToolIcon name="redo" />
            </button>
          </div>
        </div>
        <Show when={showLink()}>
          <div
            ref={(element) => {
              linkPanel = element;
            }}
            {...stylex.attrs(styles.linkForm)}
            role="group"
            aria-label="Edit link"
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                e.preventDefault();
                e.stopPropagation();
                closeLink();
              }
              if (
                e.key === "Enter" &&
                e.target instanceof HTMLInputElement &&
                e.target.type !== "radio"
              ) {
                e.preventDefault();
                applyLink();
              }
            }}
          >
            <p {...stylex.attrs(styles.linkTitle)}>Link destination</p>
            <div {...stylex.attrs(styles.linkChoices)}>
              <For
                each={[
                  { id: "internal" as const, label: "Site link" },
                  { id: "external" as const, label: "External link" },
                ]}
              >
                {(kind) => (
                  <label {...stylex.attrs(styles.choice)}>
                    <input
                      type="radio"
                      name={linkGroup}
                      checked={linkKind() === kind.id}
                      onChange={() => {
                        setLinkKind(kind.id);
                        setHref("");
                        setSitePage("");
                        setError("");
                      }}
                    />
                    {kind.label}
                  </label>
                )}
              </For>
            </div>
            <Show when={linkKind() === "internal"}>
              <label {...stylex.attrs(common.label)}>
                Site page
                <select
                  {...stylex.attrs(common.control)}
                  value={sitePage()}
                  onChange={(e) => {
                    setSitePage(e.currentTarget.value);
                    setHref(e.currentTarget.value === "custom" ? "" : e.currentTarget.value);
                    setError("");
                  }}
                >
                  <option value="">Choose a page…</option>
                  <For each={[...p.pages].sort((a, b) => a.title.localeCompare(b.title))}>
                    {(page) => (
                      <option value={page.slug}>
                        {page.title} · {page.slug}
                        {page.published_revision === null ? " (unpublished)" : ""}
                      </option>
                    )}
                  </For>
                  <option value="custom">Custom site path…</option>
                </select>
              </label>
              <Show when={sitePage() && sitePage() !== "custom"}>
                <p {...stylex.attrs(styles.hint)}>
                  {href()}
                  <Show
                    when={
                      p.pages.find((page) => page.slug === sitePage())?.published_revision === null
                    }
                  >
                    <br />
                    Publish this page before readers can visit it.
                  </Show>
                </p>
              </Show>
            </Show>
            <Show when={linkKind() === "external" || sitePage() === "custom"}>
              <label {...stylex.attrs(common.label)}>
                Link URL
                <input
                  {...stylex.attrs(common.control)}
                  value={href()}
                  onInput={(e) => setHref(e.currentTarget.value)}
                  placeholder={linkKind() === "internal" ? "/about#team" : "https://example.com"}
                />
              </label>
            </Show>
            <div {...stylex.attrs(styles.actions)}>
              <button
                type="button"
                {...stylex.attrs(common.button, common.primary, styles.action)}
                onClick={applyLink}
              >
                Apply link
              </button>
              <Show when={active("link")}>
                <button
                  type="button"
                  {...stylex.attrs(common.button, common.danger, styles.action)}
                  onClick={() => {
                    editor()?.chain().focus().extendMarkRange("link").unsetLink().run();
                    setShowLink(false);
                  }}
                >
                  Remove link
                </button>
              </Show>
              <button
                type="button"
                {...stylex.attrs(common.button, styles.action)}
                onClick={closeLink}
              >
                Cancel
              </button>
            </div>
            <Show when={error()}>
              <p {...stylex.attrs(common.error)} role="alert">
                {error()}
              </p>
            </Show>
          </div>
        </Show>
      </div>
      <div
        ref={(element) => {
          host = element;
        }}
      />
    </div>
  );
}
