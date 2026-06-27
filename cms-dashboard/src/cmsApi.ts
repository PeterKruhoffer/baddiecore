import type { Rendering } from "./cmsTypes";

type ProblemDetails = {
  detail?: string;
  title?: string;
  message?: string;
  currentVersion?: number;
  errors?: Record<string, string[]>;
};

export class ApiError extends Error {
  constructor(
    message: string,
    public readonly status: number,
    public readonly currentVersion?: number
  ) {
    super(message);
  }
}

async function readProblem(response: Response): Promise<ProblemDetails | null> {
  return response.json().catch(() => null) as Promise<ProblemDetails | null>;
}

export async function getJson<T>(url: string): Promise<T> {
  const response = await fetch(url, { headers: { Accept: "application/json" } });

  if (!response.ok) {
    const problem = await readProblem(response);
    throw new ApiError(
      problem?.message ?? problem?.detail ?? problem?.title ?? `Request failed with status ${response.status}.`,
      response.status
    );
  }

  return response.json() as Promise<T>;
}

export async function updateRenderingDatasource(
  rendering: Rendering,
  values: Readonly<Record<string, string>>
): Promise<void> {
  const response = await fetch(`/api/renderings/${encodeURIComponent(rendering.id)}/datasource/fields`, {
    method: "PUT",
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify({
      datasourceId: rendering.datasourceId,
      expectedVersion: rendering.datasourceVersion,
      fields: rendering.fields.map((field) => ({ name: field.name, value: values[field.name] ?? "" }))
    })
  });

  if (!response.ok) {
    const problem = await readProblem(response);
    const validationMessage = problem?.errors ? Object.values(problem.errors).flat()[0] : undefined;
    throw new ApiError(
      validationMessage ?? problem?.detail ?? problem?.title ?? `Request failed with status ${response.status}.`,
      response.status,
      problem?.currentVersion
    );
  }
}
