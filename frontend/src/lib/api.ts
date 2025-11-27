export interface Credentials {
  subdomain: string;
  password: string;
}

const base =
  (import.meta.env.VITE_API_BASE as string | undefined)?.replace(/\/$/, "") ??
  window.location.origin.replace(/\/$/, "");

export const API_BASE = base;

export const joinApiUrl = (path: string) => `${API_BASE}${path}`;

export const storageKeys = {
  creds: "satsuki.auth",
} as const;

export function buildBasicAuthHeader(creds: Credentials): string {
  return `Basic ${btoa(`${creds.subdomain}:${creds.password}`)}`;
}

export function loadCredentials(): Credentials | null {
  try {
    const raw = localStorage.getItem(storageKeys.creds);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Credentials;
    if (parsed.subdomain && parsed.password) {
      return parsed;
    }
  } catch (err) {
    console.warn("failed to parse stored credentials", err);
  }
  return null;
}

export function storeCredentials(creds: Credentials | null) {
  if (!creds) {
    localStorage.removeItem(storageKeys.creds);
    return;
  }
  localStorage.setItem(storageKeys.creds, JSON.stringify(creds));
}

export function compareDomain(d1: string, d2: string): -1 | 0 | 1 {
  const d1Parts = String(d1).replace(/\.$/, '').split('.').reverse();
  const d2Parts = String(d2).replace(/\.$/, '').split('.').reverse();
  const minLen = Math.min(d1Parts.length, d2Parts.length);
  for (let i = 0; i < minLen; i++) {
    const d1 = d1Parts[i].toLowerCase();
    const d2 = d2Parts[i].toLowerCase();
    if (d1 < d2) {
      return -1;
    } else if (d1 > d2) {
      return 1;
    }
  }

  if (d1Parts.length > d2Parts.length) {
    return 1;
  } else if (d1Parts.length < d2Parts.length) {
    return -1;
  } else {
    return 0;
  }
}

export interface RecordDto {
  id?: string;
  name: string;
  rrtype: string;
  ttl: number;
  content: string;
  priority: number | null;
}

export interface ProfileDto {
  subdomain: string;
  external_ns: boolean;
  external_ns1: string | null;
  external_ns2: string | null;
  external_ns3: string | null;
  external_ns4: string | null;
  external_ns5: string | null;
  external_ns6: string | null;
}

export interface NsListEntry {
  name: string;
  records: string[];
}
