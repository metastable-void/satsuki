import {
  BrowserRouter,
  Navigate,
  Route,
  Routes,
  useLocation,
} from "react-router-dom";
import { createContext, ReactNode, useContext, useMemo, useState } from "react";
import LandingPage from "./pages/Landing.js";
import ManagePage from "./pages/Manage.js";
import {
  Credentials,
  loadCredentials,
  storeCredentials,
} from "./lib/api.js";

interface AuthValue {
  credentials: Credentials | null;
  signIn: (creds: Credentials) => void;
  signOut: () => void;
}

const AuthContext = createContext<AuthValue | undefined>(undefined);

export const useAuth = () => {
  const ctx = useContext(AuthContext);
  if (!ctx) {
    throw new Error("useAuth must be used inside AuthProvider");
  }
  return ctx;
};

function AuthProvider({ children }: { children: ReactNode }) {
  const [credentials, setCredentials] = useState<Credentials | null>(() =>
    loadCredentials(),
  );

  const value = useMemo<AuthValue>(() => {
    return {
      credentials,
      signIn(creds) {
        setCredentials(creds);
        storeCredentials(creds);
      },
      signOut() {
        setCredentials(null);
        storeCredentials(null);
      },
    };
  }, [credentials]);

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

function RequireAuth({ children }: { children: ReactNode }) {
  const { credentials } = useAuth();
  const location = useLocation();

  if (!credentials) {
    return <Navigate to="/" state={{ from: location }} replace />;
  }

  return <>{children}</>;
}

export default function App() {
  return (
    <BrowserRouter>
      <AuthProvider>
        <Routes>
          <Route path="/" element={<LandingPage />} />
          <Route
            path="/manage"
            element={
              <RequireAuth>
                <ManagePage />
              </RequireAuth>
            }
          />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </AuthProvider>
    </BrowserRouter>
  );
}
