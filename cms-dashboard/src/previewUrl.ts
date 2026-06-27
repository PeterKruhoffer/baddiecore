export function sanitizePreviewUrl(value: string): string {
  const trimmedValue = value.trim();

  if (!trimmedValue) return "#";

  try {
    const parsedUrl = new URL(trimmedValue, "https://preview.baddiecore.invalid");
    const allowedProtocols = new Set(["http:", "https:", "mailto:", "tel:"]);
    return allowedProtocols.has(parsedUrl.protocol) ? trimmedValue : "#";
  } catch {
    return "#";
  }
}
