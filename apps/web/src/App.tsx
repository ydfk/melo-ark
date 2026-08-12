import { useCallback, useEffect, useState } from "react";

import { clearAccessToken, getAccessToken } from "@/lib/api";
import { getProfile } from "@/lib/api/methods/user";
import type { UserResponse } from "@/lib/api/types";
import { AuthPage } from "@/pages/auth-page";
import { DashboardPage } from "@/pages/dashboard";
import { ForcePasswordPage } from "@/pages/force-password-page";

type AppState =
  | { kind: "loading" }
  | { kind: "auth" }
  | { kind: "force-password"; user: UserResponse }
  | { kind: "ready"; user: UserResponse };

export default function App() {
  const [state, setState] = useState<AppState>({ kind: "loading" });

  const bootstrap = useCallback(async () => {
    setState({ kind: "loading" });
    try {
      if (!getAccessToken()) {
        setState({ kind: "auth" });
        return;
      }

      const user = await getProfile().send();
      setState(
        user.passwordChangeRequired ? { kind: "force-password", user } : { kind: "ready", user }
      );
    } catch {
      clearAccessToken();
      setState({ kind: "auth" });
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
    return <AuthPage onAuthenticated={bootstrap} />;
  }

  if (state.kind === "force-password") {
    return <ForcePasswordPage username={state.user.username} onChanged={bootstrap} />;
  }

  return (
    <DashboardPage
      user={state.user}
      onUserChanged={(user) => setState({ kind: "ready", user })}
      onLogout={() => {
        clearAccessToken();
        setState({ kind: "auth" });
      }}
    />
  );
}
