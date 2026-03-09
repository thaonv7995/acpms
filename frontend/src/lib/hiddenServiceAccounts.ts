export const OPENCLAW_SERVICE_ACCOUNT_EMAIL = 'openclaw-gateway@acpms.local';

export function isHiddenServiceAccountEmail(email?: string | null): boolean {
  return (email ?? '').trim().toLowerCase() === OPENCLAW_SERVICE_ACCOUNT_EMAIL;
}

export function filterHiddenServiceAccounts<T extends { email?: string | null }>(
  items: T[]
): T[] {
  return items.filter((item) => !isHiddenServiceAccountEmail(item.email));
}
