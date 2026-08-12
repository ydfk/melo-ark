import { CircleAlert, CloudCog, RefreshCw, Settings2 } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { ApiError } from "@/lib/api";
import { getProviders, updateProvider } from "@/lib/api/methods/library";
import type { ProviderSetting } from "@/lib/api/types";

type ProviderDraft = {
  providerId: string;
  displayName: string;
  baseUrl: string;
  priority: string;
  timeoutMs: string;
  rateLimitMs: string;
};

export function ProviderPanel({ embedded = false }: { embedded?: boolean }) {
  const [items, setItems] = useState<ProviderSetting[]>([]);
  const [busyId, setBusyId] = useState<string>();
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string>();
  const [draft, setDraft] = useState<ProviderDraft>();
  const refresh = async () => {
    setLoading(true);
    try {
      setItems(await getProviders().send());
      setLoadError(undefined);
    } catch (error) {
      const message = error instanceof ApiError ? error.problem.detail : "无法读取在线数据源状态";
      setLoadError(message);
      showError(error);
    } finally {
      setLoading(false);
    }
  };
  useEffect(() => {
    void refresh();
  }, []);

  async function toggle(item: ProviderSetting, enabled: boolean) {
    setBusyId(item.providerId);
    try {
      const updated = await updateProvider(item.providerId, { enabled }).send();
      setItems((current) =>
        current.map((value) => (value.providerId === updated.providerId ? updated : value))
      );
    } catch (error) {
      showError(error);
    } finally {
      setBusyId(undefined);
    }
  }

  function edit(item: ProviderSetting) {
    setDraft({
      providerId: item.providerId,
      displayName: item.displayName,
      baseUrl: item.baseUrl ?? "",
      priority: String(item.priority),
      timeoutMs: String(item.timeoutMs),
      rateLimitMs: String(item.rateLimitMs),
    });
  }

  async function saveDraft() {
    if (!draft) return;
    setBusyId(draft.providerId);
    try {
      const updated = await updateProvider(draft.providerId, {
        baseUrl: draft.baseUrl.trim(),
        priority: Number(draft.priority),
        timeoutMs: Number(draft.timeoutMs),
        rateLimitMs: Number(draft.rateLimitMs),
      }).send();
      setItems((current) =>
        current.map((value) => (value.providerId === updated.providerId ? updated : value))
      );
      setDraft(undefined);
      toast.success(`${updated.displayName} 配置已保存`);
    } catch (error) {
      showError(error);
    } finally {
      setBusyId(undefined);
    }
  }

  return (
    <div className="space-y-5">
      <div className="flex items-end justify-between gap-4">
        <div>
          <h2
            className={embedded ? "text-lg font-semibold" : "font-display text-2xl font-semibold"}
          >
            在线数据源
          </h2>
          {!embedded ? (
            <p className="mt-1 text-sm text-muted-foreground">管理元数据与歌词来源。</p>
          ) : null}
        </div>
        <Button variant="outline" size="sm" onClick={() => void refresh()}>
          <RefreshCw data-icon="inline-start" />
          刷新健康状态
        </Button>
      </div>
      {!embedded ? (
        <Alert>
          <CircleAlert />
          <AlertTitle>数据源稳定性</AlertTitle>
          <AlertDescription>
            部分公开接口可能发生变化，异常时会自动重试并临时熔断。
          </AlertDescription>
        </Alert>
      ) : null}
      {loadError ? (
        <Alert variant="destructive">
          <CircleAlert />
          <AlertTitle>在线数据源不可用</AlertTitle>
          <AlertDescription className="flex flex-wrap items-center justify-between gap-3">
            <span>{loadError}。本地曲库、标签和播放功能不受影响。</span>
            <Button variant="outline" size="sm" onClick={() => void refresh()}>
              重新连接
            </Button>
          </AlertDescription>
        </Alert>
      ) : null}
      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
        {items.map((item) => (
          <Card key={item.providerId} className="bg-card/70">
            <CardHeader>
              <div className="flex items-start justify-between gap-3">
                <div>
                  <CardTitle className="flex items-center gap-2">
                    <CloudCog className="size-4" />
                    {item.displayName}
                  </CardTitle>
                  <CardDescription className="mt-1">
                    优先级 {item.priority} ·{" "}
                    {item.kind === "lyrics"
                      ? "歌词"
                      : item.kind === "metadata"
                        ? "元数据"
                        : "元数据与歌词"}
                  </CardDescription>
                </div>
                <Switch
                  checked={item.enabled}
                  disabled={busyId === item.providerId || (!item.baseUrl && !item.enabled)}
                  onCheckedChange={(value) => void toggle(item, value)}
                />
              </div>
            </CardHeader>
            <CardContent className="space-y-3">
              <div className="flex flex-wrap gap-2">
                <Badge variant={item.maturity === "stable" ? "secondary" : "outline"}>
                  {item.maturity === "stable" ? "稳定" : "测试中"}
                </Badge>
                {item.consecutiveFailures ? (
                  <Badge variant="destructive">连续失败 {item.consecutiveFailures}</Badge>
                ) : (
                  <Badge variant="outline">健康</Badge>
                )}
                {item.circuitOpenUntil ? <Badge variant="destructive">熔断中</Badge> : null}
              </div>
              <p className="truncate text-xs text-muted-foreground">
                {item.baseUrl ?? "尚未配置服务地址"}
              </p>
              <p className="text-xs text-muted-foreground">
                超时 {item.timeoutMs} ms · 请求间隔 {item.rateLimitMs} ms
              </p>
              {item.lastError ? (
                <p className="line-clamp-2 text-xs text-destructive">{item.lastError}</p>
              ) : null}
              <Button variant="outline" size="sm" className="w-full" onClick={() => edit(item)}>
                <Settings2 data-icon="inline-start" />
                配置连接与限流
              </Button>
            </CardContent>
          </Card>
        ))}
      </div>
      {!loading && !loadError && !items.length ? (
        <Alert>
          <CloudCog />
          <AlertTitle>没有可用的在线数据源</AlertTitle>
        </Alert>
      ) : null}
      <Dialog open={Boolean(draft)} onOpenChange={(open) => !open && setDraft(undefined)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>配置 {draft?.displayName}</DialogTitle>
            <DialogDescription>修改服务地址、优先级、请求超时和请求间隔。</DialogDescription>
          </DialogHeader>
          {draft ? (
            <form
              className="grid gap-4"
              onSubmit={(event) => {
                event.preventDefault();
                void saveDraft();
              }}
            >
              <div className="grid gap-2">
                <Label htmlFor="provider-base-url">服务地址</Label>
                <Input
                  id="provider-base-url"
                  type="url"
                  required
                  value={draft.baseUrl}
                  onChange={(event) =>
                    setDraft((current) =>
                      current ? { ...current, baseUrl: event.target.value } : current
                    )
                  }
                  placeholder="https://provider.example.com"
                />
              </div>
              <div className="grid gap-4 sm:grid-cols-3">
                <ProviderNumberField
                  id="provider-priority"
                  label="优先级"
                  min={0}
                  max={10000}
                  value={draft.priority}
                  onChange={(priority) =>
                    setDraft((current) => (current ? { ...current, priority } : current))
                  }
                />
                <ProviderNumberField
                  id="provider-timeout"
                  label="超时（毫秒）"
                  min={100}
                  max={120000}
                  value={draft.timeoutMs}
                  onChange={(timeoutMs) =>
                    setDraft((current) => (current ? { ...current, timeoutMs } : current))
                  }
                />
                <ProviderNumberField
                  id="provider-rate-limit"
                  label="请求间隔 (ms)"
                  min={0}
                  max={60000}
                  value={draft.rateLimitMs}
                  onChange={(rateLimitMs) =>
                    setDraft((current) => (current ? { ...current, rateLimitMs } : current))
                  }
                />
              </div>
              <DialogFooter>
                <Button type="button" variant="outline" onClick={() => setDraft(undefined)}>
                  取消
                </Button>
                <Button disabled={busyId === draft.providerId}>保存配置</Button>
              </DialogFooter>
            </form>
          ) : null}
        </DialogContent>
      </Dialog>
    </div>
  );
}

function ProviderNumberField({
  id,
  label,
  min,
  max,
  value,
  onChange,
}: {
  id: string;
  label: string;
  min: number;
  max: number;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="grid gap-2">
      <Label htmlFor={id}>{label}</Label>
      <Input
        id={id}
        type="number"
        min={min}
        max={max}
        required
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </div>
  );
}

function showError(error: unknown) {
  toast.error(error instanceof ApiError ? error.problem.detail : "无法读取在线数据源状态");
}
