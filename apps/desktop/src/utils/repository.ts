const REPOSITORY_WHITESPACE = /\s/u;

export function isLaunchableRepository(fullName: string): boolean {
  const parts = fullName.split("/");
  return (
    parts.length === 2 &&
    !REPOSITORY_WHITESPACE.test(fullName) &&
    parts.every((part) => part.length > 0)
  );
}
