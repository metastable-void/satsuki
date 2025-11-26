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
