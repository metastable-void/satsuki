import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  API_BASE,
  buildBasicAuthHeader,
  decodeDomain,
  joinApiUrl,
  ProfileDto,
  RecordDto,
  RTYPES,
} from "../lib/api.js";
import { useAuth } from "../App.js";
import { toASCII, toUnicode } from "punycode";

type EditableRecord = RecordDto & { id: string };

const makeId = () =>
  typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : Math.random().toString(36).slice(2);

const emptyNsValues = () => Array(6).fill("");

interface AboutResponse {
  base_domain: string;
}

const trimTrailingDot = (value: string) =>
  value.endsWith(".") ? value.slice(0, -1) : value;

const buildZoneName = (subdomain: string, baseDomain: string) =>
  `${subdomain}.${trimTrailingDot(baseDomain)}.`;

const toRelativeRecordName = (fqdn: string, zoneName: string) => {
  const trimmed = fqdn.trim();
  if (!trimmed) return "";

  const lowerName = trimmed.toLowerCase();
  const lowerZone = zoneName.toLowerCase();

  if (lowerName === lowerZone) {
    return "@";
  }

  if (lowerName.endsWith(lowerZone)) {
    const prefix = trimmed.slice(0, trimmed.length - lowerZone.length);
    const withoutDot = prefix.endsWith(".") ? prefix.slice(0, -1) : prefix;
    return withoutDot || "@";
  }

  return trimmed.endsWith(".") ? trimmed.slice(0, -1) : trimmed;
};

const decodeRelativeName = (value: string) => {
  const trimmed = value.trim();
  if (!trimmed || trimmed === "@") {
    return trimmed;
  }
  return trimmed
    .split(".")
    .map((label) => {
      if (!label) return label;
      try {
        return toUnicode(label);
      } catch {
        return label;
      }
    })
    .join(".");
};

const encodeRelativeName = (value: string) => {
  const trimmed = value.trim();
  if (!trimmed || trimmed === "@") {
    return trimmed;
  }
  return trimmed
    .split(".")
    .map((label) => {
      if (!label) return label;
      try {
        return toASCII(label);
      } catch {
        return label;
      }
    })
    .join(".");
};

export default function ManagePage() {
  const { credentials, signOut } = useAuth();
  const navigate = useNavigate();
  const authHeader = useMemo(
    () => buildBasicAuthHeader(credentials!),
    [credentials],
  );

  const [profile, setProfile] = useState<ProfileDto | null>(null);
  const [profileMessage, setProfileMessage] = useState<string | null>(null);
  const [nsValues, setNsValues] = useState<string[]>(emptyNsValues);
  const [records, setRecords] = useState<EditableRecord[]>([]);
  const [recordsMessage, setRecordsMessage] = useState<string | null>(null);
  const [loadingProfile, setLoadingProfile] = useState(true);
  const [loadingRecords, setLoadingRecords] = useState(false);
  const [baseDomain, setBaseDomain] = useState<string | null>(null);
  const [passwordForm, setPasswordForm] = useState({
    current: "",
    next: "",
    confirm: "",
  });
  const [passwordMessage, setPasswordMessage] = useState<string | null>(null);
  const [passwordBusy, setPasswordBusy] = useState(false);
  const [passwordOpen, setPasswordOpen] = useState(false);
  const decodedBaseDomain = baseDomain ? decodeDomain(baseDomain) : "";

  useEffect(() => {
    document.title = decodedBaseDomain || "Satsuki Admin";
  }, [decodedBaseDomain]);

  const useInternalNs = profile ? !profile.external_ns : true;
  const recordsDisabled = !!profile?.external_ns;

  const fetchProfile = async () => {
    setLoadingProfile(true);
    try {
      const res = await fetch(joinApiUrl("/api/profile"), {
        headers: { Authorization: authHeader },
      });
      if (res.status === 401) {
        signOut();
        navigate("/", { replace: true });
        return;
      }
      if (!res.ok) {
        throw new Error(`Failed to load profile (${res.status})`);
      }
      const data = (await res.json()) as ProfileDto;
      setProfile(data);
      setNsValues([
        data.external_ns1 ?? "",
        data.external_ns2 ?? "",
        data.external_ns3 ?? "",
        data.external_ns4 ?? "",
        data.external_ns5 ?? "",
        data.external_ns6 ?? "",
      ]);
      setProfileMessage(null);
    } catch (err) {
      console.error(err);
      setProfileMessage("Could not load profile");
    } finally {
      setLoadingProfile(false);
    }
  };

  const fetchBaseDomain = async () => {
    try {
      const res = await fetch(joinApiUrl("/api/about"));
      if (!res.ok) {
        throw new Error(`Failed to load DNS metadata (${res.status})`);
      }
      const data = (await res.json()) as AboutResponse;
      setBaseDomain(data.base_domain);
    } catch (err) {
      console.error(err);
      setRecordsMessage("Could not load DNS metadata.");
    }
  };

  const fetchZoneRecords = async () => {
    if (!profile) {
      setRecords([]);
      return;
    }
    if (profile.external_ns) {
      setRecords([]);
      return;
    }
    if (!baseDomain) {
      return;
    }
    const zoneName = buildZoneName(profile.subdomain, baseDomain);

    setLoadingRecords(true);
    try {
      const res = await fetch(joinApiUrl("/api/zone"), {
        headers: { Authorization: authHeader },
      });
      if (res.status === 401) {
        signOut();
        navigate("/", { replace: true });
        return;
      }
      if (!res.ok) {
        throw new Error(`Failed to load zone (${res.status})`);
      }
      const data = (await res.json()) as RecordDto[];
      setRecords(
        data.map((rec, idx) => ({
          ...rec,
          name: decodeRelativeName(toRelativeRecordName(rec.name, zoneName)),
          id: `${rec.name}-${rec.rrtype}-${idx}-${makeId()}`,
        })),
      );
      setRecordsMessage(null);
    } catch (err) {
      console.error(err);
      setRecordsMessage("Could not load DNS records");
    } finally {
      setLoadingRecords(false);
    }
  };

  useEffect(() => {
    fetchProfile();
    fetchBaseDomain();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    fetchZoneRecords();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [profile?.external_ns, baseDomain]);

  const handleToggleNsMode = async (checked: boolean) => {
    if (!profile) return;
    setProfileMessage(null);
    try {
      if (checked) {
        const res = await fetch(joinApiUrl("/api/ns-mode/internal"), {
          method: "POST",
          headers: { Authorization: authHeader },
        });
        if (!res.ok) throw new Error("Failed to switch to internal NS");
      } else {
        const filtered = nsValues.map((ns) => ns.trim()).filter(Boolean);
        if (!filtered.length) {
          setProfileMessage("Provide at least one nameserver to switch to external mode.");
          return;
        }
        const res = await fetch(joinApiUrl("/api/ns-mode/external"), {
          method: "POST",
          headers: {
            Authorization: authHeader,
            "Content-Type": "application/json",
          },
          body: JSON.stringify({ ns: filtered }),
        });
        if (!res.ok) throw new Error("Failed to switch to external NS");
      }
      await fetchProfile();
    } catch (err) {
      console.error(err);
      setProfileMessage(
        err instanceof Error ? err.message : "Failed to update nameserver mode",
      );
    }
  };

  const saveExternalNames = async () => {
    if (!profile) return;
    const filtered = nsValues.map((ns) => ns.trim()).filter(Boolean);
    if (!filtered.length) {
      setProfileMessage("Enter at least one nameserver.");
      return;
    }
    try {
      const res = await fetch(joinApiUrl("/api/ns-mode/external"), {
        method: "POST",
        headers: {
          Authorization: authHeader,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ ns: filtered }),
      });
      if (!res.ok) throw new Error("Failed to update nameservers");
      await fetchProfile();
      setProfileMessage("Nameservers updated.");
    } catch (err) {
      console.error(err);
      setProfileMessage(
        err instanceof Error ? err.message : "Failed to update nameservers",
      );
    }
  };

  const addRecord = () => {
    setRecords((prev) => [
      ...prev,
      {
        id: makeId(),
        name: "@",
        rrtype: "A",
        ttl: 300,
        content: "",
        priority: null,
      },
    ]);
  };

  const updateRecord = (
    id: string,
    field: keyof RecordDto,
    value: string,
  ) => {
    setRecords((prev) =>
      prev.map((record) =>
        record.id === id
          ? {
              ...record,
              [field]:
                field === "ttl"
                  ? Number(value)
                  : field === "priority"
                    ? value === "" ? null : Number(value)
                    : value,
            }
          : record,
      ),
    );
  };

  const removeRecord = (id: string) =>
    setRecords((prev) => prev.filter((record) => record.id !== id));

  const handlePasswordField = (field: "current" | "next" | "confirm", value: string) => {
    setPasswordForm((prev) => ({ ...prev, [field]: value }));
  };

  const changePassword = async () => {
    setPasswordMessage(null);
    if (!passwordForm.current || !passwordForm.next || !passwordForm.confirm) {
      setPasswordMessage("Fill out all password fields.");
      return;
    }
    if (passwordForm.next !== passwordForm.confirm) {
      setPasswordMessage("New password and confirmation do not match.");
      return;
    }
    if (passwordForm.next.length < 8) {
      setPasswordMessage("New password must be at least 8 characters.");
      return;
    }
    setPasswordBusy(true);
    try {
      const res = await fetch(joinApiUrl("/api/password/change"), {
        method: "POST",
        headers: {
          Authorization: authHeader,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          current_password: passwordForm.current,
          new_password: passwordForm.next,
        }),
      });
      if (!res.ok) {
        const text = await res.text();
        throw new Error(text || "Failed to change password");
      }
      setPasswordMessage("Password updated.");
      setPasswordForm({ current: "", next: "", confirm: "" });
    } catch (err) {
      console.error(err);
      setPasswordMessage(
        err instanceof Error ? err.message : "Failed to change password",
      );
    } finally {
      setPasswordBusy(false);
    }
  };

  const saveRecords = async () => {
    if (!records.length) {
      setRecordsMessage("Add at least one record.");
      return;
    }
    const payload = records.map((record) => ({
      name: encodeRelativeName(record.name),
      rrtype: record.rrtype.trim().toUpperCase(),
      ttl: Number(record.ttl) || 300,
      content: record.content.trim(),
      priority:
        typeof record.priority === "number" && !Number.isNaN(record.priority)
          ? record.priority
          : null,
    }));
    try {
      const res = await fetch(joinApiUrl("/api/zone"), {
        method: "PUT",
        headers: {
          Authorization: authHeader,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ records: payload }),
      });
      if (!res.ok) throw new Error("Failed to update DNS zone");
      setRecordsMessage("DNS records updated.");
      await fetchZoneRecords();
    } catch (err) {
      console.error(err);
      setRecordsMessage(
        err instanceof Error ? err.message : "Failed to update DNS zone",
      );
    }
  };

  return (
    <main className="page manage-page">
      <header className="manage-header">
        <div>
          <h1>Subdomain manager</h1>
          <p className="muted">
            Connected to <code>{API_BASE}</code>
          </p>
        </div>
        <div className="header-actions">
          <span className="muted">
            Logged in as <strong>{profile?.subdomain ?? "…"}</strong>
          </span>
          <button
            type="button"
            className="secondary"
            onClick={() => {
              signOut();
              navigate("/");
            }}
          >
            Sign out
          </button>
        </div>
      </header>

      <section className="panel ns-mode-panel">
        <div className="panel-header">
          <h2>Nameserver mode</h2>
          {loadingProfile && <span className="status">Loading profile…</span>}
        </div>

        <label className="checkbox-field">
          <input
            type="checkbox"
            checked={useInternalNs}
            onChange={(e) => handleToggleNsMode(e.target.checked)}
          />
          <span>Use our DNS nameservers</span>
        </label>

        <div className="ns-inputs">
          <div className="ns-inputs__header">
            <h3>External nameservers</h3>
            {useInternalNs && <span className="muted">(disabled)</span>}
          </div>
          {nsValues.map((value, idx) => (
            <label key={idx}>
              NS #{idx + 1}
              <input
                type="text"
                value={value}
                disabled={useInternalNs}
                onChange={(e) => {
                  const next = [...nsValues];
                  next[idx] = e.target.value;
                  setNsValues(next);
                }}
                placeholder="ns1.example.net."
                autoCapitalize="none"
                autoCorrect="off"
                spellCheck={false}
              />
            </label>
          ))}
          <button
            type="button"
            disabled={useInternalNs}
            onClick={saveExternalNames}
          >
            Save nameservers
          </button>
        </div>
        {profileMessage && <p className="status">{profileMessage}</p>}
      </section>

      <section className="panel records-panel">
        <div className="panel-header">
          <h2>
            Manage Records{" "}
            {profile?.external_ns && (
              <span className="muted">(disabled while using external NS)</span>
            )}
          </h2>
          {loadingRecords && !profile?.external_ns && (
            <span className="status">Loading zone…</span>
          )}
        </div>

        <div className="records-table">
          <div className="records-table__header">
            <span>Name</span>
            <span>Type</span>
            <span>TTL</span>
            <span>Content / Target</span>
            <span>Priority</span>
            <span />
          </div>
        {records.map((record) => {
          const normalizedType = record.rrtype.toUpperCase();
          const isKnownType = (RTYPES as readonly string[]).includes(normalizedType);

          return (
          <div className="records-table__row" key={record.id}>
              <label className="records-table__cell">
                <span className="records-table__cell-label">Name</span>
                <input
                  type="text"
                  value={record.name}
                  disabled={recordsDisabled}
                  onChange={(e) => updateRecord(record.id, "name", e.target.value)}
                />
              </label>
              <label className="records-table__cell">
                <span className="records-table__cell-label">Type</span>
                <select
                  value={normalizedType}
                  disabled={recordsDisabled}
                  onChange={(e) =>
                    updateRecord(record.id, "rrtype", e.target.value.toUpperCase())
                  }
                >
                  {RTYPES.map((type) => (
                    <option key={type} value={type}>
                      {type}
                    </option>
                  ))}
                  {!isKnownType && (
                    <option value={record.rrtype}>{record.rrtype}</option>
                  )}
                </select>
              </label>
              <label className="records-table__cell">
                <span className="records-table__cell-label">TTL</span>
                <input
                  type="number"
                  value={record.ttl}
                  disabled={recordsDisabled}
                  onChange={(e) => updateRecord(record.id, "ttl", e.target.value)}
                />
              </label>
              <label className="records-table__cell">
                <span className="records-table__cell-label">Content / Target</span>
                <input
                  type="text"
                  value={record.content}
                  disabled={recordsDisabled}
                  onChange={(e) => updateRecord(record.id, "content", e.target.value)}
                />
              </label>
              <label className="records-table__cell">
                <span className="records-table__cell-label">Priority</span>
                <input
                  type="number"
                  value={record.priority ?? ""}
                  disabled={recordsDisabled}
                  onChange={(e) => updateRecord(record.id, "priority", e.target.value)}
                />
              </label>
              <div className="records-table__cell records-table__cell--action">
                <button
                  type="button"
                  className="ghost material-symbols-outlined"
                  disabled={recordsDisabled}
                  onClick={() => removeRecord(record.id)}
                >
                  delete
                </button>
              </div>
            </div>
          );
        })}
        </div>

        <div className="records-actions">
          <button
            type="button"
            onClick={addRecord}
            disabled={recordsDisabled}
            className="secondary"
          >
            Add record
          </button>
          <button
            type="button"
            onClick={saveRecords}
            disabled={recordsDisabled}
          >
            Save records
          </button>
        </div>
        {recordsMessage && <p className="status">{recordsMessage}</p>}
      </section>

      <section className="panel password-panel">
        <div className="panel-header">
          <h2>Change Password</h2>
          <button
            type="button"
            className="ghost"
            onClick={() => setPasswordOpen((prev) => !prev)}
          >
            {passwordOpen ? "Hide" : "Show"}
          </button>
        </div>
        {passwordOpen && (
          <div className="password-form">
          <label>
            Current password
            <input
              type="password"
              autoComplete="current-password"
              value={passwordForm.current}
              onChange={(e) => handlePasswordField("current", e.target.value)}
            />
          </label>
          <label>
            New password
            <input
              type="password"
              autoComplete="new-password"
              value={passwordForm.next}
              onChange={(e) => handlePasswordField("next", e.target.value)}
            />
          </label>
          <label>
            Confirm new password
            <input
              type="password"
              autoComplete="new-password"
              value={passwordForm.confirm}
              onChange={(e) => handlePasswordField("confirm", e.target.value)}
            />
          </label>
          <button type="button" onClick={changePassword} disabled={passwordBusy}>
            {passwordBusy ? "Updating…" : "Update password"}
          </button>
          {passwordMessage && <p className="status">{passwordMessage}</p>}
        </div>
        )}
      </section>
    </main>
  );
}
