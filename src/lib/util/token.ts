/**
 * Token normalization for slash-qualified catalog identifiers.
 *
 * Some inherited helpers still receive path-like identifiers. Lookups that
 * receive a qualified token can fall back to the final path component. Bare
 * tokens pass through unchanged.
 */
export function bareToken(token: string): string {
  const slash = token.lastIndexOf("/");
  return slash >= 0 ? token.slice(slash + 1) : token;
}
