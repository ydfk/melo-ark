import { zodResolver } from "@hookform/resolvers/zod";
import { ArrowRight, AudioLines, Library, ListChecks } from "lucide-react";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";

import { BrandMark } from "@/components/brand-mark";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Field, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { ApiError } from "@/lib/api";
import { login } from "@/lib/api/methods/user";

const credentialsSchema = z.object({
  username: z.string().trim().min(1, "请输入用户名").max(64, "用户名最多 64 个字符"),
  password: z.string().min(1, "请输入密码").max(72, "密码最多 72 个字符"),
});

type CredentialsForm = z.infer<typeof credentialsSchema>;

export function AuthPage({ onAuthenticated }: { onAuthenticated: () => Promise<void> }) {
  const [submitError, setSubmitError] = useState<string>();
  const form = useForm<CredentialsForm>({
    resolver: zodResolver(credentialsSchema),
    defaultValues: { username: "", password: "" },
  });

  const submit = form.handleSubmit(async (credentials) => {
    setSubmitError(undefined);
    try {
      await login(credentials).send();
      await onAuthenticated();
    } catch (error) {
      setSubmitError(error instanceof ApiError ? error.problem.detail : "无法连接 MeloArk 服务");
    }
  });

  return (
    <main className="relative isolate flex min-h-screen items-center overflow-hidden px-5 py-10 sm:px-8">
      <div className="ark-orbit" aria-hidden="true" />
      <div className="relative mx-auto grid w-full max-w-6xl items-center gap-10 lg:grid-cols-[minmax(0,1.15fr)_minmax(0,0.85fr)] lg:gap-20">
        <section className="min-w-0 max-w-2xl">
          <div className="flex items-center gap-4">
            <BrandMark className="size-14" />
            <div>
              <p className="font-display text-2xl font-semibold tracking-tight">MeloArk</p>
              <p className="text-sm text-muted-foreground">自己的音乐，自己掌管</p>
            </div>
          </div>
          <h1 className="mt-10 max-w-xl font-display text-5xl font-semibold leading-[1.05] tracking-[-0.05em] sm:text-6xl">
            整理、校对并播放你的音乐收藏。
          </h1>
          <div className="mt-10 grid gap-3 sm:grid-cols-3">
            <Feature icon={Library} title="扫描曲库" />
            <Feature icon={AudioLines} title="完善元数据" />
            <Feature icon={ListChecks} title="跟踪每项任务" />
          </div>
        </section>

        <Card className="w-full border-white/10 bg-card/85 shadow-2xl shadow-black/30 backdrop-blur-xl">
          <CardHeader className="gap-2 pb-3">
            <CardTitle className="font-display text-3xl">登录</CardTitle>
            <p className="text-sm text-muted-foreground">进入 MeloArk 管理台</p>
          </CardHeader>
          <CardContent>
            <form className="space-y-5" onSubmit={submit}>
              <FieldGroup>
                <Field data-invalid={Boolean(form.formState.errors.username)}>
                  <FieldLabel htmlFor="username">用户名</FieldLabel>
                  <Input
                    id="username"
                    autoFocus
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
                    autoComplete="current-password"
                    aria-invalid={Boolean(form.formState.errors.password)}
                    {...form.register("password")}
                  />
                  <FieldError errors={[form.formState.errors.password]} />
                </Field>
              </FieldGroup>
              {submitError ? (
                <Alert variant="destructive">
                  <AlertDescription>{submitError}</AlertDescription>
                </Alert>
              ) : null}
              <Button className="w-full" type="submit" disabled={form.formState.isSubmitting}>
                {form.formState.isSubmitting ? <Spinner /> : <ArrowRight />}
                登录
              </Button>
            </form>
          </CardContent>
        </Card>
      </div>
    </main>
  );
}

function Feature({ icon: Icon, title }: { icon: typeof Library; title: string }) {
  return (
    <div className="flex items-center gap-3 rounded-xl border border-white/10 bg-card/40 px-4 py-3 backdrop-blur-sm">
      <Icon className="size-4 text-primary" />
      <span className="text-sm font-medium">{title}</span>
    </div>
  );
}
