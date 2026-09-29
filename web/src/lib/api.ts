import type {
  Bootstrap,
  ComponentDef,
  Content,
  Organization,
  Page,
  Review,
  Template,
} from "../types";
export class ApiError extends Error {
  constructor(
    message: string,
    public status: number,
  ) {
    super(message);
  }
}
async function request<T>(url: string, init?: RequestInit): Promise<T> {
  const response = await fetch(url, {
    credentials: "same-origin",
    headers: { "Content-Type": "application/json", ...init?.headers },
    ...init,
  });
  if (!response.ok) {
    let message = `Request failed (${response.status})`;
    try {
      message = (await response.json()).error || message;
    } catch {}
    throw new ApiError(message, response.status);
  }
  return response.status === 204 ? (undefined as T) : response.json();
}
export const api = {
  authConfig: () =>
    request<{ method: "password" | "redirect"; label?: string }>("/api/auth/config"),
  login: (password: string) =>
    request<void>("/api/login", {
      method: "POST",
      body: JSON.stringify({ password }),
    }),
  logout: () => request<{ redirect_url: string } | undefined>("/api/logout", { method: "POST" }),
  bootstrap: () => request<Bootstrap>("/api/admin/bootstrap"),
  organization: () => request<Organization>("/api/admin/organization"),
  saveOrganization: (value: Organization) =>
    request<Organization>("/api/admin/organization", {
      method: "PUT",
      body: JSON.stringify(value),
    }),
  submit: (page: Page) =>
    request<Review>(`/api/admin/pages/${encodeURIComponent(page.id)}/submit`, {
      method: "POST",
      body: JSON.stringify({ revision: page.revision }),
    }),
  review: (review: Review, approve: boolean, feedback: string) =>
    request<Review>(`/api/admin/reviews/${encodeURIComponent(review.id)}`, {
      method: "POST",
      body: JSON.stringify({
        revision: review.content.page.revision,
        submission_id: review.submission_id,
        approve,
        feedback,
      }),
    }),
  content: (slug: string) => request<Content>(`/api/content?slug=${encodeURIComponent(slug)}`),
  createPage: (value: Pick<Page, "title" | "slug" | "template_id">) =>
    request<Page>("/api/admin/pages", {
      method: "POST",
      body: JSON.stringify(value),
    }),
  savePage: (page: Page) =>
    request<Page>(`/api/admin/pages/${page.id}`, {
      method: "PUT",
      body: JSON.stringify(page),
    }),
  publish: (page: Page) =>
    request<Page>(`/api/admin/pages/${page.id}/publish`, {
      method: "POST",
      body: JSON.stringify({ revision: page.revision }),
    }),
  deletePage: (id: string) => request<void>(`/api/admin/pages/${id}`, { method: "DELETE" }),
  createTemplate: (value: Omit<Template, "id">) =>
    request<Template>("/api/admin/templates", {
      method: "POST",
      body: JSON.stringify(value),
    }),
  updateTemplate: (value: Template) =>
    request<Template>(`/api/admin/templates/${value.id}`, {
      method: "PUT",
      body: JSON.stringify(value),
    }),
  createComponent: (value: Omit<ComponentDef, "id">) =>
    request<ComponentDef>("/api/admin/components", {
      method: "POST",
      body: JSON.stringify(value),
    }),
  updateComponent: (value: ComponentDef) =>
    request<ComponentDef>(`/api/admin/components/${value.id}`, {
      method: "PUT",
      body: JSON.stringify(value),
    }),
};
