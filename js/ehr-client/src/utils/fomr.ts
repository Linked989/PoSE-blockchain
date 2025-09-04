export function parseFormEvent<T>(event: React.FormEvent<HTMLFormElement>) {
  event.preventDefault();
  const formData = new FormData(event.currentTarget);
  const entries = formData.entries();
  const search = Array.from(entries).reduce(
    (acc, [key, value]) => ({ ...acc, [key]: value }),
    {}
  );
  return search as T;
}
