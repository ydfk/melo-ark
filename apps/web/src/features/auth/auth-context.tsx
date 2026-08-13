import { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";

import { clearAccessToken, getAccessToken, setAccessToken } from "@/lib/api";
import { getProfile, login, updateProfile } from "@/lib/api/methods/user";
import type { Credentials, UserResponse } from "@/lib/api/types";

type AuthStatus = "loading" | "guest" | "authenticated" | "password-change-required";
type DeferredAction = () => void | Promise<void>;

type AuthContextValue = {
  status: AuthStatus;
  user?: UserResponse;
  loginOpen: boolean;
  isAuthenticated: boolean;
  openLogin: () => void;
  closeLogin: () => void;
  requireAuth: (action?: DeferredAction) => boolean;
  authenticate: (credentials: Credentials) => Promise<void>;
  changeRequiredPassword: (password: string) => Promise<void>;
  setUser: (user: UserResponse) => void;
  logout: () => void;
};

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [status, setStatus] = useState<AuthStatus>("loading");
  const [user, setUserState] = useState<UserResponse>();
  const [loginOpen, setLoginOpen] = useState(false);
  const pendingAction = useRef<DeferredAction | undefined>(undefined);

  const becomeGuest = useCallback(() => {
    clearAccessToken();
    setUserState(undefined);
    setStatus("guest");
  }, []);

  useEffect(() => {
    if (!getAccessToken()) {
      setStatus("guest");
      return;
    }

    let active = true;
    void getProfile()
      .send()
      .then((profile) => {
        if (!active) return;
        setUserState(profile);
        if (profile.passwordChangeRequired) {
          setStatus("password-change-required");
          setLoginOpen(true);
        } else {
          setStatus("authenticated");
        }
      })
      .catch(() => active && becomeGuest());
    return () => {
      active = false;
    };
  }, [becomeGuest]);

  const finishAuthentication = useCallback((profile: UserResponse) => {
    setUserState(profile);
    if (profile.passwordChangeRequired) {
      setStatus("password-change-required");
      setLoginOpen(true);
      return;
    }

    setStatus("authenticated");
    setLoginOpen(false);
    const action = pendingAction.current;
    pendingAction.current = undefined;
    if (action) void action();
  }, []);

  const authenticate = useCallback(
    async (credentials: Credentials) => {
      await login(credentials).send();
      finishAuthentication(await getProfile().send());
    },
    [finishAuthentication]
  );

  const changeRequiredPassword = useCallback(
    async (password: string) => {
      const response = await updateProfile({ newPassword: password }).send();
      setAccessToken(response.token);
      finishAuthentication(response.user);
    },
    [finishAuthentication]
  );

  const requireAuth = useCallback(
    (action?: DeferredAction) => {
      if (status === "authenticated") {
        if (action) void action();
        return true;
      }
      pendingAction.current = action;
      setLoginOpen(true);
      return false;
    },
    [status]
  );

  const closeLogin = useCallback(() => {
    if (status === "password-change-required") return;
    pendingAction.current = undefined;
    setLoginOpen(false);
  }, [status]);

  const logout = useCallback(() => {
    pendingAction.current = undefined;
    setLoginOpen(false);
    becomeGuest();
  }, [becomeGuest]);

  const setUser = useCallback((nextUser: UserResponse) => {
    setUserState(nextUser);
    setStatus(nextUser.passwordChangeRequired ? "password-change-required" : "authenticated");
  }, []);

  return (
    <AuthContext.Provider
      value={{
        status,
        user,
        loginOpen,
        isAuthenticated: status === "authenticated",
        openLogin: () => setLoginOpen(true),
        closeLogin,
        requireAuth,
        authenticate,
        changeRequiredPassword,
        setUser,
        logout,
      }}
    >
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const value = useContext(AuthContext);
  if (!value) throw new Error("useAuth 必须在 AuthProvider 内使用");
  return value;
}
