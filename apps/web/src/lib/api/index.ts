import { createAlova } from "alova";
import { createServerTokenAuthentication } from "alova/client";
import adapterFetch from "alova/fetch";
import ReactHook from "alova/react";

import { mockAdapter } from "./mock";
import type { Problem, TokenResponse } from "./types";

const TOKEN_KEY = "meloark-access-token";

export class ApiError extends Error {
  constructor(
    public readonly status: number,
    public readonly problem: Problem
  ) {
    super(problem.detail);
    this.name = "ApiError";
  }
}

export function getAccessToken() {
  return localStorage.getItem(TOKEN_KEY);
}

export function clearAccessToken() {
  localStorage.removeItem(TOKEN_KEY);
}

export function setAccessToken(token: string) {
  localStorage.setItem(TOKEN_KEY, token);
}

export function apiBaseURL() {
  const origin = import.meta.env.VITE_API_BASE_URL?.trim().replace(/\/$/, "");
  return origin ? `${origin}/api` : "/api";
}

const { onAuthRequired, onResponseRefreshToken } = createServerTokenAuthentication({
  async login(response) {
    const data = (await response.clone().json()) as TokenResponse;
    localStorage.setItem(TOKEN_KEY, data.token);
  },
  assignToken(method) {
    const token = getAccessToken();
    if (token) {
      method.config.headers.Authorization = `Bearer ${token}`;
    }
  },
  logout: clearAccessToken,
});

const useMock = import.meta.env.VITE_USE_MOCK === "true";

export const alovaInstance = createAlova({
  baseURL: apiBaseURL(),
  statesHook: ReactHook,
  requestAdapter: useMock ? mockAdapter : adapterFetch(),
  // 控制台状态必须反映磁盘与任务的当前值，避免首次初始化等流程读到旧数据。
  cacheFor: { GET: 0 },
  beforeRequest: onAuthRequired(),
  responded: onResponseRefreshToken({
    async onSuccess(response) {
      if (!response.ok) {
        const problem = (await response.clone().json()) as Problem;
        throw new ApiError(response.status, problem);
      }
      if (response.status === 204) {
        return undefined;
      }
      return response.json();
    },
    onError(error) {
      throw error;
    },
  }),
});
