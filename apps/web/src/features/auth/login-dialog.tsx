import { KeyRound, LogIn } from "lucide-react";
import { useEffect, useState } from "react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { ApiError } from "@/lib/api";

import { useAuth } from "./auth-context";

export function LoginDialog() {
  const { status, user, loginOpen, closeLogin, authenticate, changeRequiredPassword } = useAuth();
  const passwordChangeRequired = status === "password-change-required";
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [error, setError] = useState<string>();
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (!loginOpen) {
      setPassword("");
      setNewPassword("");
      setConfirmation("");
      setError(undefined);
    }
  }, [loginOpen]);

  async function submitLogin(event: React.FormEvent) {
    event.preventDefault();
    if (!username.trim() || !password) {
      setError("请输入用户名和密码");
      return;
    }
    setSubmitting(true);
    setError(undefined);
    try {
      await authenticate({ username: username.trim(), password });
      setPassword("");
    } catch (reason) {
      setError(reason instanceof ApiError ? reason.problem.detail : "无法连接 MeloArk 服务");
    } finally {
      setSubmitting(false);
    }
  }

  async function submitNewPassword(event: React.FormEvent) {
    event.preventDefault();
    if (newPassword.length < 8) {
      setError("新密码至少需要 8 个字符");
      return;
    }
    if (newPassword !== confirmation) {
      setError("两次输入的密码不一致");
      return;
    }
    setSubmitting(true);
    setError(undefined);
    try {
      await changeRequiredPassword(newPassword);
    } catch (reason) {
      setError(reason instanceof ApiError ? reason.problem.detail : "无法修改密码");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Dialog open={loginOpen} onOpenChange={(open) => !open && closeLogin()}>
      <DialogContent
        className="overflow-hidden border-white/10 bg-[oklch(0.16_0.025_270/0.96)] text-white shadow-2xl shadow-black/60 backdrop-blur-2xl sm:max-w-md"
        onPointerDownOutside={(event) => passwordChangeRequired && event.preventDefault()}
        onEscapeKeyDown={(event) => passwordChangeRequired && event.preventDefault()}
      >
        <div className="absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-cyan-300/80 to-transparent" />
        <DialogHeader className="pr-8">
          <DialogTitle className="font-display text-2xl">
            {passwordChangeRequired ? "设置新的登录密码" : "登录 MeloArk"}
          </DialogTitle>
          <DialogDescription className="text-white/55">
            {passwordChangeRequired
              ? `账号 ${user?.username ?? "admin"} 正在使用初始密码，请先完成修改。`
              : "登录后可管理曲库、收藏歌曲并保存歌单。"}
          </DialogDescription>
        </DialogHeader>

        {passwordChangeRequired ? (
          <form className="space-y-5" onSubmit={submitNewPassword}>
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="required-new-password">新密码</FieldLabel>
                <Input
                  id="required-new-password"
                  type="password"
                  autoFocus
                  autoComplete="new-password"
                  value={newPassword}
                  onChange={(event) => setNewPassword(event.target.value)}
                  className="border-white/15 bg-white/5"
                />
              </Field>
              <Field>
                <FieldLabel htmlFor="required-password-confirmation">确认新密码</FieldLabel>
                <Input
                  id="required-password-confirmation"
                  type="password"
                  autoComplete="new-password"
                  value={confirmation}
                  onChange={(event) => setConfirmation(event.target.value)}
                  className="border-white/15 bg-white/5"
                />
              </Field>
            </FieldGroup>
            <SubmitError message={error} />
            <Button className="w-full rounded-xl" disabled={submitting}>
              {submitting ? <Spinner /> : <KeyRound />}
              保存并继续
            </Button>
          </form>
        ) : (
          <form className="space-y-5" onSubmit={submitLogin}>
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="dialog-username">用户名</FieldLabel>
                <Input
                  id="dialog-username"
                  autoFocus
                  autoComplete="username"
                  value={username}
                  onChange={(event) => setUsername(event.target.value)}
                  className="border-white/15 bg-white/5"
                />
              </Field>
              <Field>
                <FieldLabel htmlFor="dialog-password">密码</FieldLabel>
                <Input
                  id="dialog-password"
                  type="password"
                  autoComplete="current-password"
                  value={password}
                  onChange={(event) => setPassword(event.target.value)}
                  className="border-white/15 bg-white/5"
                />
              </Field>
            </FieldGroup>
            <SubmitError message={error} />
            <Button className="w-full rounded-xl" disabled={submitting}>
              {submitting ? <Spinner /> : <LogIn />}
              登录并继续
            </Button>
          </form>
        )}
      </DialogContent>
    </Dialog>
  );
}

function SubmitError({ message }: { message?: string }) {
  return message ? (
    <Alert variant="destructive">
      <AlertDescription>{message}</AlertDescription>
    </Alert>
  ) : null;
}
