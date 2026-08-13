import { lazy, Suspense, useState } from "react";

import { AuthProvider, useAuth } from "@/features/auth/auth-context";
import { LoginDialog } from "@/features/auth/login-dialog";
import { PlayerProvider } from "@/features/player/player-context";
import { PlayerHome } from "@/features/player/player-home";

const DashboardPage = lazy(() =>
  import("@/pages/dashboard").then((module) => ({ default: module.DashboardPage }))
);

export default function App() {
  return (
    <AuthProvider>
      <PlayerProvider>
        <AppShell />
      </PlayerProvider>
    </AuthProvider>
  );
}

function AppShell() {
  const { user, isAuthenticated, setUser, logout } = useAuth();
  const [mode, setMode] = useState<"player" | "management">("player");

  const leaveManagement = () => setMode("player");
  const handleLogout = () => {
    logout();
    leaveManagement();
  };

  return (
    <>
      {mode === "management" && isAuthenticated && user ? (
        <Suspense fallback={<ManagementLoading />}>
          <DashboardPage
            user={user}
            onUserChanged={setUser}
            onLogout={handleLogout}
            onOpenPlayer={leaveManagement}
          />
        </Suspense>
      ) : (
        <PlayerHome onOpenManagement={() => setMode("management")} />
      )}
      <LoginDialog />
    </>
  );
}

function ManagementLoading() {
  return (
    <main className="grid min-h-screen place-items-center bg-background">
      <div className="flex items-center gap-3 text-sm text-muted-foreground">
        <span className="size-2 animate-pulse rounded-full bg-primary" />
        正在打开管理中心…
      </div>
    </main>
  );
}
