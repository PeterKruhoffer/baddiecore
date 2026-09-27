export type FieldKind = "text" | "textarea" | "url";
export interface Field {
  name: string;
  label: string;
  kind: FieldKind;
  required: boolean;
}
export type RendererName = "hero" | "text" | "callout" | "cards";
export interface ComponentDef {
  id: string;
  name: string;
  description: string;
  renderer: RendererName;
  fields: Field[];
}
export interface Region {
  name: string;
  allowed_components: string[];
  max_components: number;
}
export interface Template {
  id: string;
  name: string;
  description: string;
  regions: Region[];
}
export interface Block {
  id: string;
  component_id: string;
  region: string;
  fields: Record<string, string>;
}
export interface Page {
  id: string;
  title: string;
  slug: string;
  template_id: string;
  blocks: Block[];
  revision: number;
  published_revision: number | null;
}
export interface Bootstrap {
  pages: Page[];
  templates: Template[];
  components: ComponentDef[];
}
export interface Content {
  page: Page;
  template: Template;
  components: ComponentDef[];
}
