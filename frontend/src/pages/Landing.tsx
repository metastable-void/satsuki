import { FormEvent, useEffect, useMemo, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import {
  API_BASE,
  compareDomain,
  decodeDomain,
  joinApiUrl,
  NsListEntry,
  ParentSoaResponse,
} from "../lib/api.js";
import { useAuth } from "../App.js";

type AvailabilityState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "available" }
  | { kind: "existing" }
  | { kind: "error"; message: string };

interface AboutResponse {
  base_domain: string;
}

export default function LandingPage() {
  const { signIn, credentials } = useAuth();
  const navigate = useNavigate();
  const [baseDomain, setBaseDomain] = useState<string>("");
  const [nsList, setNsList] = useState<NsListEntry[]>([]);
  const [nsError, setNsError] = useState<string | null>(null);
  const [soaLine, setSoaLine] = useState<string | null>(null);
  const [soaError, setSoaError] = useState<string | null>(null);

  const [subdomain, setSubdomain] = useState("");
  const [availability, setAvailability] = useState<AvailabilityState>({
    kind: "idle",
  });
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [formMessage, setFormMessage] = useState<string | null>(null);
  const [formBusy, setFormBusy] = useState(false);

  useEffect(() => {
    const loadAbout = async () => {
      try {
        const res = await fetch(joinApiUrl("/api/about"));
        if (!res.ok) throw new Error(`About request failed: ${res.status}`);
        const data = (await res.json()) as AboutResponse;
        setBaseDomain(data.base_domain);
      } catch (err) {
        console.error(err);
      }
    };
    loadAbout();
  }, []);

  useEffect(() => {
    const loadNs = async () => {
      try {
        const res = await fetch(joinApiUrl("/api/subdomain/list"));
        if (!res.ok) throw new Error(`NS list failed with ${res.status}`);
        const data = (await res.json()) as NsListEntry[];
        data.sort((e1, e2) => compareDomain(e1.name, e2.name));
        data.forEach((e) => {
          e.records.sort((r1, r2) => compareDomain(r1, r2));
        });
        setNsList(data);
        setNsError(null);
      } catch (err) {
        console.error(err);
        setNsError("Failed to load NS records");
      }
    };
    loadNs();
  }, []);

  useEffect(() => {
    const loadSoa = async () => {
      try {
        const res = await fetch(joinApiUrl("/api/subdomain/soa"));
        if (!res.ok) throw new Error(`SOA fetch failed with ${res.status}`);
        const data = (await res.json()) as ParentSoaResponse;
        setSoaLine(data.soa.trim());
        setSoaError(null);
      } catch (err) {
        console.error(err);
        setSoaLine(null);
        setSoaError("Failed to load SOA record");
      }
    };
    loadSoa();
  }, []);

  useEffect(() => {
    const trimmed = subdomain.trim().toLowerCase();
    if (!trimmed) {
      setAvailability({ kind: "idle" });
      return;
    }

    let cancelled = false;
    setAvailability({ kind: "checking" });
    const controller = new AbortController();
    const timer = setTimeout(async () => {
      try {
        const res = await fetch(
          `${joinApiUrl("/api/subdomain/check")}?name=${encodeURIComponent(trimmed)}`,
          { signal: controller.signal },
        );
        if (!res.ok) {
          throw new Error(`Check failed with ${res.status}`);
        }
        const data = (await res.json()) as { available: boolean };
        if (cancelled) return;
        setAvailability({ kind: data.available ? "available" : "existing" });
      } catch (err) {
        if (controller.signal.aborted) return;
        console.error(err);
        setAvailability({
          kind: "error",
          message: "Unable to validate subdomain",
        });
      }
    }, 400);

    return () => {
      cancelled = true;
      clearTimeout(timer);
      controller.abort();
    };
  }, [subdomain]);

  const decodedBaseDomain = baseDomain ? decodeDomain(baseDomain) : "";

  const bindLines = useMemo(() => {
    const sections = nsList
      .map((entry) =>
        entry.records.map((record: string) => `${entry.name}\tIN\tNS\t${record}`).join("\n"),
      )
      .filter(Boolean);
    const parts: string[] = [];
    const soaOwner = decodedBaseDomain || baseDomain || ".";
    const soa = soaLine?.trim();
    if (soa) {
      parts.push(`${soaOwner}\tIN\tSOA\t${soa}`);
    }
    if (sections.length) {
      parts.push(sections.join("\n\n"));
    }
    return parts.join("\n\n");
  }, [decodedBaseDomain, nsList, soaLine]);

  const showSignIn = availability.kind === "existing";
  const showSignUp = availability.kind === "available";

  const handleSubmit = async (evt: FormEvent) => {
    evt.preventDefault();
    const trimmed = subdomain.trim().toLowerCase();
    if (!trimmed) {
      setFormMessage("Please enter a subdomain");
      return;
    }
    if (availability.kind === "checking") {
      setFormMessage("Please wait until the availability check finishes");
      return;
    }
    setFormMessage(null);
    setFormBusy(true);
    try {
      if (showSignIn) {
        const res = await fetch(joinApiUrl("/api/signin"), {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ subdomain: trimmed, password }),
        });
        if (!res.ok) {
          const text = await res.text();
          throw new Error(text || "Sign-in failed");
        }
        signIn({ subdomain: trimmed, password });
        navigate("/manage");
        return;
      }
      if (showSignUp) {
        if (!password || !confirmPassword) {
          setFormMessage("Enter your password in both fields");
          return;
        }
        if (password !== confirmPassword) {
          setFormMessage("Passwords do not match");
          return;
        }
        const res = await fetch(joinApiUrl("/api/signup"), {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ subdomain: trimmed, password }),
        });
        if (!res.ok) {
          const text = await res.text();
          throw new Error(text || "Signup failed");
        }
        // automatically sign in
        const signinRes = await fetch(joinApiUrl("/api/signin"), {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ subdomain: trimmed, password }),
        });
        if (!signinRes.ok) {
          throw new Error("Registered but automatic sign-in failed, try logging in manually.");
        }
        signIn({ subdomain: trimmed, password });
        navigate("/manage");
        return;
      }
      setFormMessage("Enter a valid subdomain to continue.");
    } catch (err) {
      console.error(err);
      setFormMessage(err instanceof Error ? err.message : "Request failed");
    } finally {
      setFormBusy(false);
    }
  };


  useEffect(() => {
    document.title = decodedBaseDomain || "Satsuki Admin";
  }, [decodedBaseDomain]);

  const manageLabel =
    credentials && decodedBaseDomain
      ? `Go to ${credentials.subdomain}.${decodedBaseDomain}`
      : null;

  return (
    <main className="page landing-page">
      {manageLabel && (
        <p className="status">
          <Link to="/manage">{manageLabel}</Link>
        </p>
      )}
      <section className="panel domain-panel">
        <h1>{decodedBaseDomain || "example.com"}</h1>
        <p className="muted">
          API endpoint: <code>{API_BASE}</code>
        </p>
        <form className="domain-form" onSubmit={handleSubmit}>
          <label className="domain-input">
            <span>Your domain</span>
            <div className="domain-input__control">
              <input
                type="text"
                value={subdomain}
                onChange={(e) => setSubdomain(e.target.value)}
                placeholder="alice"
                autoCapitalize="none"
                autoCorrect="off"
                spellCheck={false}
              />
              <span className="domain-input__suffix">
                .{decodedBaseDomain || "example.com"}
              </span>
            </div>
          </label>

          {availability.kind === "checking" && (
            <p className="status">Checking availability…</p>
          )}
          {availability.kind === "error" && (
            <p className="status error">{availability.message}</p>
          )}

          {showSignIn && (
            <div className="auth-box">
              <h2>Sign in</h2>
              <label>
                Password
                <input
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                />
              </label>
              <button type="submit" disabled={formBusy}>
                {formBusy ? "Signing in…" : "Sign in"}
              </button>
            </div>
          )}

          {showSignUp && (
            <div className="auth-box">
              <h2>Register a new subdomain</h2>
              <label>
                Password
                <input
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                />
              </label>
              <label>
                Confirm password
                <input
                  type="password"
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
                />
              </label>
              <button type="submit" disabled={formBusy}>
                {formBusy ? "Registering…" : "Create account"}
              </button>
            </div>
          )}

          {formMessage && <p className="status error">{formMessage}</p>}
        </form>
      </section>

      <section className="panel ns-panel">
        <h2>Nameserver delegation ({nsList.length - 1} subdomains)</h2>
        {nsError && <p className="status error">{nsError}</p>}
        {soaError && <p className="status error">{soaError}</p>}
        {!nsList.length && !nsError && <p className="status">Loading…</p>}
        {bindLines && (
          <pre className="bind-list">
            {bindLines}
          </pre>
        )}
      </section>
    </main>
  );
}
