export type FieldKind = "text" | "textarea" | "url";
export interface Field {
  name: string;
  label: string;
  kind: FieldKind;
  required: boolean;
}
export type RendererName = "hero" | "text" | "callout" | "cards" | "external";
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
  access: Access;
  reviews: Review[];
}
export interface Content {
  page: Page;
  template: Template;
  components: ComponentDef[];
}
export type Role = "admin" | "reviewer" | "editor";
export interface Access {
  id: string;
  role: Role;
  paths: string[];
}
export interface Member extends Access {
  name: string;
  groups: string[];
}
export interface Group {
  id: string;
  name: string;
  paths: string[];
}
export interface Organization {
  revision: number;
  members: Member[];
  groups: Group[];
}
export interface Review {
  id: string;
  submission_id: string;
  content: Content;
  submitted_by: string;
  status: "submitted" | "changes_requested" | "approved";
  feedback: string;
  reviewed_by: string | null;
}
