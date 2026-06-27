export type ContentTreeNode = {
  id: string;
  name: string;
  path: string;
  template: string;
  workflowState: string;
  children: ContentTreeNode[];
};

export type ContentTreeResponse = {
  items: ContentTreeNode[];
};

export type RenderingField = {
  name: string;
  label: string;
  type: string;
  required: boolean;
  value: string;
};

export type Rendering = {
  id: string;
  name: string;
  componentKey: string;
  datasourceId: string;
  datasourceVersion: number;
  fields: RenderingField[];
};

export type LayoutResponse = {
  path: string;
  mode: string;
  metadata: {
    id: string;
    name: string;
    template: string;
    workflowState: string;
    version: number;
    isDraft: boolean;
  };
  fields: Record<string, string>;
  placeholders: Array<{
    name: string;
    renderings: Rendering[];
  }>;
};
