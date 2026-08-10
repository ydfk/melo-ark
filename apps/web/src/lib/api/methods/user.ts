import { alovaInstance } from "..";
import type {
  Credentials,
  HealthResponse,
  SetupStatusResponse,
  TokenResponse,
  UserResponse,
} from "../types";

export const getHealth = () => alovaInstance.Get<HealthResponse>("/health");

export const getSetupStatus = () => alovaInstance.Get<SetupStatusResponse>("/auth/setup-status");

export const setup = (credentials: Credentials) =>
  alovaInstance.Post<UserResponse>("/auth/setup", credentials);

export const login = (credentials: Credentials) =>
  alovaInstance.Post<TokenResponse>("/auth/login", credentials, {
    meta: {
      authRole: "login",
    },
  });

export const getProfile = () => alovaInstance.Get<UserResponse>("/auth/profile");
