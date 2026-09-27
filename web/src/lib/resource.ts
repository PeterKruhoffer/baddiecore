import { createSignal } from "solid-js";
export function createRequest<T>(load: () => Promise<T>) {
  const [value, setValue] = createSignal<T>();
  const [error, setError] = createSignal<unknown>();
  const [loading, setLoading] = createSignal(true);
  async function refetch() {
    setLoading(true);
    setError(undefined);
    try {
      const result = await load();
      setValue(() => result);
    } catch (e) {
      setError(e);
    } finally {
      setLoading(false);
    }
  }
  void refetch();
  return { value, error, loading, refetch };
}
