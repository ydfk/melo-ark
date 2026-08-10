import { useCallback, useEffect, useState } from "react";

import { clearAccessToken, getAccessToken } from "@/lib/api";
import { getProfile, getSetupStatus } from "@/lib/api/methods/user";
import type { UserResponse } from "@/lib/api/types";
import { AuthPage } from "@/pages/auth-page";
import { DashboardPage } from "@/pages/dashboard";

type AppState =
  | { kind: "loading" }
  | { kind: "auth"; setupRequired: boolean }
  | { kind: "ready"; user: UserResponse };

export default function App() {
  const [state, setState] = useState<AppState>({ kind: "loading" });

  const bootstrap = useCallback(async () => {
    setState({ kind: "loading" });
    try {
      const setupStatus = await getSetupStatus().send();
      if (setupStatus.setupRequired || !getAccessToken()) {
        setState({ kind: "auth", setupRequired: setupStatus.setupRequired });
        return;
      }

      const user = await getProfile().send();
      setState({ kind: "ready", user });
    } catch {
      clearAccessToken();
      setState({ kind: "auth", setupRequired: false });
    }
  }, []);

  useEffect(() => {
    void bootstrap();
  }, [bootstrap]);

  if (state.kind === "loading") {
    return (
      <main className="flex min-h-screen items-center justify-center">
        <div className="flex items-center gap-3 text-sm text-muted-foreground">
          <span className="size-2 animate-pulse rounded-full bg-primary" />
          正在唤醒你的音乐方舟…
        </div>
      </main>
    );
  }

  if (state.kind === "auth") {
    return <AuthPage setupRequired={state.setupRequired} onAuthenticated={bootstrap} />;
  }

  return (
    <DashboardPage
      user={state.user}
      onLogout={() => {
        clearAccessToken();
        setState({ kind: "auth", setupRequired: false });
      }}
    />
  );
}
