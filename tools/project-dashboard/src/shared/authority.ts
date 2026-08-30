export function isAuthorityPath(path: string): boolean {
  return /^(?:AGENTS\.md|CONTRIBUTING\.md|SECURITY\.md|docs\/(?:decisions|governance|specs|vision)\/|docs\/product\/product-roadmap\.md)/.test(path);
}

export function isDecisionAuthorityPath(path: string): boolean {
  return /^docs\/(?:decisions|governance|specs|vision)\//.test(path);
}
