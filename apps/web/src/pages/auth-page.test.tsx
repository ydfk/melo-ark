import { render, screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";

import { AuthPage } from "./auth-page";

describe("AuthPage", () => {
  test("shows a concise product introduction beside the login form", () => {
    render(<AuthPage onAuthenticated={vi.fn()} />);

    expect(screen.getByText("整理、校对并播放你的音乐收藏。")).toBeInTheDocument();
    expect(screen.getByText("扫描曲库")).toBeInTheDocument();
    expect(screen.getByText("完善元数据")).toBeInTheDocument();
    expect(screen.getByText("跟踪每项任务")).toBeInTheDocument();
    expect(screen.getByLabelText("用户名")).toBeInTheDocument();
    expect(screen.getByLabelText("密码")).toBeInTheDocument();
  });
});
