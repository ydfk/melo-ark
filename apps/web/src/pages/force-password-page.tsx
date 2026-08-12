import { zodResolver } from "@hookform/resolvers/zod";
import { KeyRound } from "lucide-react";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";

import { BrandMark } from "@/components/brand-mark";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Field, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { ApiError, setAccessToken } from "@/lib/api";
import { updateProfile } from "@/lib/api/methods/user";

const passwordSchema = z
  .object({
    password: z.string().min(8, "新密码至少需要 8 个字符").max(72, "密码最多 72 个字符"),
    confirmation: z.string(),
  })
  .refine((values) => values.password === values.confirmation, {
    path: ["confirmation"],
    message: "两次输入的密码不一致",
  });

type PasswordForm = z.infer<typeof passwordSchema>;

export function ForcePasswordPage({
  username,
  onChanged,
}: {
  username: string;
  onChanged: () => Promise<void>;
}) {
  const [submitError, setSubmitError] = useState<string>();
  const form = useForm<PasswordForm>({
    resolver: zodResolver(passwordSchema),
    defaultValues: { password: "", confirmation: "" },
  });
  const submit = form.handleSubmit(async ({ password }) => {
    setSubmitError(undefined);
    try {
      const response = await updateProfile({ newPassword: password }).send();
      setAccessToken(response.token);
      await onChanged();
    } catch (error) {
      setSubmitError(error instanceof ApiError ? error.problem.detail : "无法修改密码");
    }
  });

  return (
    <main className="relative isolate flex min-h-screen items-center justify-center overflow-hidden px-5 py-12">
      <div className="ark-orbit" aria-hidden="true" />
      <Card className="relative w-full max-w-md border-white/10 bg-card/85 shadow-2xl shadow-black/30 backdrop-blur-xl">
        <CardHeader className="items-center gap-4 text-center">
          <BrandMark className="size-11" />
          <div className="space-y-1.5">
            <CardTitle className="font-display text-2xl">请先修改默认密码</CardTitle>
            <p className="text-sm text-muted-foreground">当前账号：{username}</p>
          </div>
        </CardHeader>
        <CardContent>
          <form className="space-y-5" onSubmit={submit}>
            <FieldGroup>
              <Field data-invalid={Boolean(form.formState.errors.password)}>
                <FieldLabel htmlFor="new-password">新密码</FieldLabel>
                <Input
                  id="new-password"
                  type="password"
                  autoFocus
                  autoComplete="new-password"
                  {...form.register("password")}
                />
                <FieldError errors={[form.formState.errors.password]} />
              </Field>
              <Field data-invalid={Boolean(form.formState.errors.confirmation)}>
                <FieldLabel htmlFor="confirm-password">确认新密码</FieldLabel>
                <Input
                  id="confirm-password"
                  type="password"
                  autoComplete="new-password"
                  {...form.register("confirmation")}
                />
                <FieldError errors={[form.formState.errors.confirmation]} />
              </Field>
            </FieldGroup>
            {submitError ? (
              <Alert variant="destructive">
                <AlertDescription>{submitError}</AlertDescription>
              </Alert>
            ) : null}
            <Button className="w-full" disabled={form.formState.isSubmitting}>
              <KeyRound />
              保存新密码
            </Button>
          </form>
        </CardContent>
      </Card>
    </main>
  );
}
