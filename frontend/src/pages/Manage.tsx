import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  API_BASE,
  buildBasicAuthHeader,
  joinApiUrl,
  ProfileDto,
  RecordDto,
} from "../lib/api.js";
import { useAuth } from "../App.js";

type EditableRecord = RecordDto & { id: string };

const makeId = () =>
  typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : Math.random().toString(36).slice(2);

const emptyNsValues = () => Array(6).fill("");

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

  const useInternalNs = profile ? !profile.external_ns : true;

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

  const fetchZoneRecords = async () => {
    if (!profile || profile.external_ns) {
      setRecords([]);
      return;
    }
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    fetchZoneRecords();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [profile?.external_ns]);

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
        name: profile ? `${profile.subdomain}.` : "",
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

  const saveRecords = async () => {
    if (!records.length) {
      setRecordsMessage("Add at least one record.");
      return;
    }
    const payload = records.map((record) => ({
      name: record.name.trim(),
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
          {records.map((record) => (
            <div className="records-table__row" key={record.id}>
              <input
                value={record.name}
                disabled={profile?.external_ns}
                onChange={(e) => updateRecord(record.id, "name", e.target.value)}
              />
              <input
                value={record.rrtype}
                disabled={profile?.external_ns}
                onChange={(e) => updateRecord(record.id, "rrtype", e.target.value)}
              />
              <input
                type="number"
                value={record.ttl}
                disabled={profile?.external_ns}
                onChange={(e) => updateRecord(record.id, "ttl", e.target.value)}
              />
              <input
                value={record.content}
                disabled={profile?.external_ns}
                onChange={(e) => updateRecord(record.id, "content", e.target.value)}
              />
              <input
                type="number"
                value={record.priority ?? ""}
                disabled={profile?.external_ns}
                onChange={(e) => updateRecord(record.id, "priority", e.target.value)}
              />
              <button
                type="button"
                className="ghost"
                disabled={profile?.external_ns}
                onClick={() => removeRecord(record.id)}
              >
                Remove
              </button>
            </div>
          ))}
        </div>

        <div className="records-actions">
          <button
            type="button"
            onClick={addRecord}
            disabled={!!profile?.external_ns}
            className="secondary"
          >
            Add record
          </button>
          <button
            type="button"
            onClick={saveRecords}
            disabled={!!profile?.external_ns}
          >
            Save records
          </button>
        </div>
        {recordsMessage && <p className="status">{recordsMessage}</p>}
      </section>
    </main>
  );
}
