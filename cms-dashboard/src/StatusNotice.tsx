export function ErrorNotice(props: { error: unknown }) {
  const message = () => props.error instanceof Error ? props.error.message : "An unexpected error occurred.";
  return <p class="notice error">{message()}</p>;
}
