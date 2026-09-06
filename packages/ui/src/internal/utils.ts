export function joinClass(...values: Array<string | undefined | false>): string {
  return values.filter(Boolean).join(" ");
}
