import { useMemo, useState } from "react";

export default function App() {
  const [baseUrl, setBaseUrl] = useState("http://localhost:8080");
  const [status, setStatus] = useState<string | null>(null);

  const healthEndpoint = useMemo(() => `${baseUrl.replace(/\/$/, "")}/health`, [baseUrl]);

  return (
    <main className="app">
      <header>
        <h1>Satsuki Frontend Placeholder</h1>
        <p>Wire this UI to the Rust APIs as needed.</p>
      </header>

      <section>
        <label>
          API base URL
          <input
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            placeholder="https://example.com"
          />
        </label>
        <button
          onClick={async () => {
            try {
              const res = await fetch(healthEndpoint, { mode: "cors" });
              setStatus(`${res.status} ${res.statusText}`);
            } catch (err) {
              setStatus(String(err));
            }
          }}
        >
          Probe /health
        </button>
        {status && <p className="status">Last response: {status}</p>}
      </section>
    </main>
  );
}
