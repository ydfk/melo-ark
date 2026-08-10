import { beforeEach, describe, expect, test } from "vitest";

import { clearAccessToken, getAccessToken } from ".";
import { getHealth, getProfile, getSetupStatus, login, setup } from "./methods/user";

describe("shared API contract", () => {
  beforeEach(() => {
    clearAccessToken();
  });

  test("uses the same health shape as both real backends", async () => {
    await expect(getHealth().send()).resolves.toEqual({
      status: "ok",
      service: "mock-backend",
      version: "1.0.0",
    });
  });

  test("sets up, stores the login token, and loads a profile", async () => {
    await expect(getSetupStatus().send()).resolves.toEqual({ setupRequired: true });
    const user = await setup({ username: "alice", password: "pass123" }).send();
    expect(user.username).toBe("alice");
    await expect(getSetupStatus().send()).resolves.toEqual({ setupRequired: false });

    await login({ username: "alice", password: "pass123" }).send();
    expect(getAccessToken()).toBe("mock-jwt-token");

    await expect(getProfile().send()).resolves.toMatchObject({ username: "admin" });
  });
});
