import { LogOut, UserRound } from "lucide-react";
import { type FormEvent, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ApiError, setAccessToken } from "@/lib/api";
import { updateProfile } from "@/lib/api/methods/user";
import type { UserResponse } from "@/lib/api/types";

type ProfileMenuProps = {
  user: UserResponse;
  onChanged: (user: UserResponse) => void;
  onLogout: () => void;
};

export function ProfileMenu({ user, onChanged, onLogout }: ProfileMenuProps) {
  const [open, setOpen] = useState(false);
  const [username, setUsername] = useState(user.username);
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [saving, setSaving] = useState(false);

  function openProfile() {
    setUsername(user.username);
    setCurrentPassword("");
    setNewPassword("");
    setConfirmation("");
    setOpen(true);
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    const nextUsername = username.trim();
    const usernameChanged = nextUsername !== user.username;
    if (!usernameChanged && !newPassword) {
      toast.info("没有需要保存的修改");
      return;
    }
    if (newPassword && newPassword !== confirmation) {
      toast.error("两次输入的新密码不一致");
      return;
    }

    setSaving(true);
    try {
      const response = await updateProfile({
        username: usernameChanged ? nextUsername : undefined,
        currentPassword,
        newPassword: newPassword || undefined,
      }).send();
      setAccessToken(response.token);
      onChanged(response.user);
      setOpen(false);
      toast.success("账号资料已更新");
    } catch (error) {
      toast.error(error instanceof ApiError ? error.problem.detail : "账号资料保存失败");
    } finally {
      setSaving(false);
    }
  }

  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" className="gap-2.5 px-2.5">
            <span className="flex size-7 items-center justify-center rounded-full bg-primary/12 text-primary">
              <UserRound className="size-4" />
            </span>
            <span className="hidden max-w-36 truncate sm:inline">{user.username}</span>
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-48">
          <DropdownMenuLabel className="truncate">{user.username}</DropdownMenuLabel>
          <DropdownMenuSeparator />
          <DropdownMenuItem onSelect={openProfile}>
            <UserRound />
            账号设置
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={onLogout}>
            <LogOut />
            退出登录
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <form onSubmit={submit} className="space-y-5">
            <DialogHeader>
              <DialogTitle>账号设置</DialogTitle>
              <DialogDescription>可修改用户名或密码。</DialogDescription>
            </DialogHeader>
            <div className="space-y-2">
              <Label htmlFor="profile-username">用户名</Label>
              <Input
                id="profile-username"
                value={username}
                onChange={(event) => setUsername(event.target.value)}
                maxLength={64}
                required
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="profile-current-password">当前密码</Label>
              <Input
                id="profile-current-password"
                type="password"
                value={currentPassword}
                onChange={(event) => setCurrentPassword(event.target.value)}
                autoComplete="current-password"
                required
              />
            </div>
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor="profile-new-password">新密码</Label>
                <Input
                  id="profile-new-password"
                  type="password"
                  value={newPassword}
                  onChange={(event) => setNewPassword(event.target.value)}
                  autoComplete="new-password"
                  minLength={8}
                  placeholder="不修改可留空"
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="profile-confirmation">确认新密码</Label>
                <Input
                  id="profile-confirmation"
                  type="password"
                  value={confirmation}
                  onChange={(event) => setConfirmation(event.target.value)}
                  autoComplete="new-password"
                  minLength={newPassword ? 8 : undefined}
                  required={Boolean(newPassword)}
                />
              </div>
            </div>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => setOpen(false)}>
                取消
              </Button>
              <Button type="submit" disabled={saving}>
                {saving ? "保存中…" : "保存"}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </>
  );
}
