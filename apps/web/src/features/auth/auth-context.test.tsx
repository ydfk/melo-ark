import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { UserResponse } from "@/lib/api/types";

import { AuthProvider, useAuth } from "./auth-context";

const { getProfileMock, loginMock, updateProfileMock } = vi.hoisted(() => ({
  getProfileMock: vi.fn(),
  loginMock: vi.fn(),
  updateProfileMock: vi.fn(),
}));

vi.mock("@/lib/api/methods/user", () => ({
  getProfile: () => ({ send: getProfileMock }),
  login: () => ({ send: loginMock }),
  updateProfile: () => ({ send: updateProfileMock }),
}));

const user: UserResponse = {
  id: "user-1",
  username: "admin",
  passwordChangeRequired: false,
  createdAt: "2026-08-12T00:00:00Z",
  updatedAt: "2026-08-12T00:00:00Z",
};

function wrapper({ children }: { children: ReactNode }) {
  return <AuthProvider>{children}</AuthProvider>;
}

describe("AuthProvider", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.clearAllMocks();
  });

  test("访客操作在登录成功后继续执行", async () => {
    getProfileMock.mockResolvedValue(user);
    loginMock.mockResolvedValue({ token: "token", passwordChangeRequired: false });
    const action = vi.fn();
    const { result } = renderHook(() => useAuth(), { wrapper });

    await waitFor(() => expect(result.current.status).toBe("guest"));
    act(() => result.current.requireAuth(action));
    expect(result.current.loginOpen).toBe(true);
    expect(action).not.toHaveBeenCalled();

    await act(() => result.current.authenticate({ username: "admin", password: "admin" }));

    expect(result.current.status).toBe("authenticated");
    expect(result.current.loginOpen).toBe(false);
    expect(action).toHaveBeenCalledOnce();
  });

  test("默认密码必须在同一登录流程中修改", async () => {
    const forcedUser = { ...user, passwordChangeRequired: true };
    getProfileMock.mockResolvedValue(forcedUser);
    loginMock.mockResolvedValue({ token: "token", passwordChangeRequired: true });
    updateProfileMock.mockResolvedValue({ user, token: "updated-token" });
    const action = vi.fn();
    const { result } = renderHook(() => useAuth(), { wrapper });

    await waitFor(() => expect(result.current.status).toBe("guest"));
    act(() => result.current.requireAuth(action));
    await act(() => result.current.authenticate({ username: "admin", password: "admin" }));

    expect(result.current.status).toBe("password-change-required");
    expect(result.current.loginOpen).toBe(true);
    expect(action).not.toHaveBeenCalled();

    await act(() => result.current.changeRequiredPassword("new-password"));

    expect(result.current.status).toBe("authenticated");
    expect(action).toHaveBeenCalledOnce();
    expect(localStorage.getItem("meloark-access-token")).toBe("updated-token");
  });
});
