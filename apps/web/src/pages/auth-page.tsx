import { zodResolver } from "@hookform/resolvers/zod";
import { ArrowRight, Database, HardDrive, ShieldCheck } from "lucide-react";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";

import { BrandMark } from "@/components/brand-mark";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Field, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { ApiError } from "@/lib/api";
import { login, setup } from "@/lib/api/methods/user";

const credentialsSchema = z.object({
  username: z.string().trim().min(1, "请输入管理员用户名").max(64, "用户名最多 64 个字符"),
  password: z.string().min(6, "密码至少需要 6 个字符").max(72, "密码最多 72 个字符"),
});

type CredentialsForm = z.infer<typeof credentialsSchema>;

type AuthPageProps = {
  setupRequired: boolean;
  onAuthenticated: () => Promise<void>;
};

const promises = [
  { icon: HardDrive, label: "曲库留在你的 NAS" },
  { icon: ShieldCheck, label: "危险操作先预览再执行" },
  { icon: Database, label: "本地 SQLite，单管理员" },
];

export function AuthPage({ setupRequired, onAuthenticated }: AuthPageProps) {
  const [submitError, setSubmitError] = useState<string>();
  const form = useForm<CredentialsForm>({
    resolver: zodResolver(credentialsSchema),
    defaultValues: { username: "", password: "" },
  });

  const submit = form.handleSubmit(async (credentials) => {
    setSubmitError(undefined);
    try {
      if (setupRequired) {
        await setup(credentials).send();
      }
      await login(credentials).send();
      await onAuthenticated();
    } catch (error) {
      setSubmitError(error instanceof ApiError ? error.problem.detail : "无法连接 MeloArk 服务");
    }
  });

  return (
    <main className="relative isolate flex min-h-screen items-center overflow-hidden px-5 py-12">
      <div className="ark-orbit" aria-hidden="true" />
      <div className="relative mx-auto grid w-full max-w-6xl items-center gap-12 lg:grid-cols-[1.15fr_0.85fr]">
        <section className="max-w-xl">
          <div className="mb-10 flex items-center gap-3">
            <BrandMark />
            <div>
              <p className="font-display text-xl font-semibold tracking-tight">MeloArk</p>
              <p className="text-xs uppercase tracking-[0.22em] text-muted-foreground">
                Your music, safely archived
              </p>
            </div>
          </div>
          <p className="mb-4 font-mono text-xs uppercase tracking-[0.28em] text-primary">
            NAS Music Control Deck
          </p>
          <h1 className="font-display text-5xl font-semibold leading-[1.04] tracking-[-0.045em] sm:text-6xl">
            让散落的音乐，
            <span className="text-gradient">安全归舱。</span>
          </h1>
          <p className="mt-6 max-w-lg text-base leading-7 text-muted-foreground">
            扫描、修正标签、匹配中文元数据、识别重复并以 Hardlink 整理。音乐文件始终由你掌控。
          </p>
          <div className="mt-8 flex flex-col gap-3">
            {promises.map(({ icon: Icon, label }) => (
              <div key={label} className="flex items-center gap-3 text-sm text-foreground/85">
                <span className="grid size-8 place-items-center rounded-lg border bg-card/60">
                  <Icon aria-hidden="true" />
                </span>
                {label}
              </div>
            ))}
          </div>
        </section>

        <Card className="border-white/10 bg-card/80 shadow-2xl shadow-black/30 backdrop-blur-xl">
          <CardHeader>
            <CardTitle className="font-display text-2xl">
              {setupRequired ? "创建首位管理员" : "返回控制台"}
            </CardTitle>
            <CardDescription>
              {setupRequired
                ? "这是唯一的首版管理员账号，创建后初始化入口会自动关闭。"
                : "使用管理员账号登录你的本地 MeloArk。"}
            </CardDescription>
          </CardHeader>
          <CardContent>
            <form id="auth-form" onSubmit={submit}>
              <FieldGroup>
                <Field data-invalid={Boolean(form.formState.errors.username)}>
                  <FieldLabel htmlFor="username">管理员用户名</FieldLabel>
                  <Input
                    id="username"
                    autoComplete="username"
                    aria-invalid={Boolean(form.formState.errors.username)}
                    {...form.register("username")}
                  />
                  <FieldError errors={[form.formState.errors.username]} />
                </Field>
                <Field data-invalid={Boolean(form.formState.errors.password)}>
                  <FieldLabel htmlFor="password">密码</FieldLabel>
                  <Input
                    id="password"
                    type="password"
                    autoComplete={setupRequired ? "new-password" : "current-password"}
                    aria-invalid={Boolean(form.formState.errors.password)}
                    {...form.register("password")}
                  />
                  <FieldError errors={[form.formState.errors.password]} />
                </Field>
              </FieldGroup>
            </form>
            {submitError ? (
              <Alert variant="destructive" className="mt-6">
                <AlertTitle>无法继续</AlertTitle>
                <AlertDescription>{submitError}</AlertDescription>
              </Alert>
            ) : null}
          </CardContent>
          <CardFooter>
            <Button
              className="w-full"
              form="auth-form"
              type="submit"
              disabled={form.formState.isSubmitting}
            >
              {form.formState.isSubmitting ? (
                <Spinner data-icon="inline-start" />
              ) : (
                <ArrowRight data-icon="inline-start" />
              )}
              {setupRequired ? "创建管理员并进入" : "登录 MeloArk"}
            </Button>
          </CardFooter>
        </Card>
      </div>
    </main>
  );
}
