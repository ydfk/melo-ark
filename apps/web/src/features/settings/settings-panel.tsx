import {
  Bot,
  Database,
  FolderSync,
  Gauge,
  HardDrive,
  RadioTower,
  Save,
  ServerCog,
} from "lucide-react";
import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { ProviderPanel } from "@/features/scraper/provider-panel";
import { ApiError } from "@/lib/api";
import { getRuntimeSettings, updateRuntimeSettings } from "@/lib/api/methods/library";
import type { EditableSettings, RuntimeSettings } from "@/lib/api/types";
import { formatBytes } from "@/lib/format";

export function SettingsPanel() {
  const [settings, setSettings] = useState<RuntimeSettings>();
  const [values, setValues] = useState<EditableSettings>();
  const [apiKey, setApiKey] = useState("");
  const [clearApiKey, setClearApiKey] = useState(false);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [loadError, setLoadError] = useState<string>();

  async function refresh() {
    setLoading(true);
    try {
      const response = await getRuntimeSettings().send();
      setSettings(response);
      setValues(response.values);
      setApiKey("");
      setClearApiKey(false);
      setLoadError(undefined);
    } catch (error) {
      setLoadError(error instanceof ApiError ? error.problem.detail : "无法读取设置");
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  function update<K extends keyof EditableSettings>(key: K, value: EditableSettings[K]) {
    setValues((current) => (current ? { ...current, [key]: value } : current));
  }

  async function save() {
    if (!values) return;
    setSaving(true);
    try {
      const response = await updateRuntimeSettings({
        values,
        aiApiKey: apiKey.trim() || undefined,
        clearAiApiKey: clearApiKey,
      }).send();
      setSettings(response);
      setValues(response.values);
      setApiKey("");
      setClearApiKey(false);
      toast.success("设置已保存");
    } catch (error) {
      toast.error(error instanceof ApiError ? error.problem.detail : "设置保存失败");
    } finally {
      setSaving(false);
    }
  }

  if (loading) {
    return <Skeleton className="h-[520px] rounded-2xl" />;
  }
  if (!settings || !values) {
    return (
      <Alert variant="destructive">
        <ServerCog />
        <AlertTitle>设置不可用</AlertTitle>
        <AlertDescription className="flex items-center justify-between gap-4">
          <span>{loadError}</span>
          <Button variant="outline" size="sm" onClick={() => void refresh()}>
            重试
          </Button>
        </AlertDescription>
      </Alert>
    );
  }

  const locked = new Set(settings.lockedByEnvironment);
  const restart = new Set(settings.restartRequiredFields);
  const fieldMeta = (name: string) => ({
    locked: locked.has(name),
    restart: restart.has(name),
  });

  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h2 className="font-display text-3xl font-semibold">设置</h2>
          <p className="mt-1 text-sm text-muted-foreground">调整服务运行参数和在线数据源。</p>
        </div>
        <Button onClick={() => void save()} disabled={saving}>
          <Save />
          {saving ? "保存中…" : "保存设置"}
        </Button>
      </div>

      <Tabs defaultValue="scan">
        <TabsList className="h-auto w-full justify-start overflow-x-auto rounded-xl border bg-card/70 p-1">
          <TabsTrigger value="scan">
            <FolderSync />
            扫描
          </TabsTrigger>
          <TabsTrigger value="sources">
            <Database />
            在线数据源
          </TabsTrigger>
          <TabsTrigger value="analysis">
            <Gauge />
            分析
          </TabsTrigger>
          <TabsTrigger value="ai">
            <Bot />
            AI
          </TabsTrigger>
          <TabsTrigger value="organizer">
            <HardDrive />
            文件整理
          </TabsTrigger>
          <TabsTrigger value="playback">
            <RadioTower />
            播放
          </TabsTrigger>
          <TabsTrigger value="system">
            <ServerCog />
            系统信息
          </TabsTrigger>
        </TabsList>

        <SettingsTab value="scan" title="扫描与监听">
          <SettingsGrid>
            <NumberSetting
              label="扫描并发数"
              value={values.scanWorkers}
              min={1}
              {...fieldMeta("scanWorkers")}
              onChange={(value) => update("scanWorkers", value)}
            />
            <NumberSetting
              label="定期扫描间隔（秒）"
              value={values.reconcileIntervalSec}
              min={60}
              {...fieldMeta("reconcileIntervalSec")}
              onChange={(value) => update("reconcileIntervalSec", value)}
            />
            <NumberSetting
              label="文件监听延迟（秒）"
              value={values.watchDebounceSec}
              min={1}
              max={300}
              {...fieldMeta("watchDebounceSec")}
              onChange={(value) => update("watchDebounceSec", value)}
            />
          </SettingsGrid>
        </SettingsTab>

        <TabsContent value="sources" className="mt-5 space-y-5">
          <Card>
            <CardHeader>
              <CardTitle>请求策略</CardTitle>
            </CardHeader>
            <CardContent>
              <SettingsGrid>
                <NumberSetting
                  label="缓存时间（秒）"
                  value={values.sourceCacheTtlSec}
                  min={1}
                  {...fieldMeta("sourceCacheTtlSec")}
                  onChange={(value) => update("sourceCacheTtlSec", value)}
                />
                <NumberSetting
                  label="失败重试次数"
                  value={values.sourceRetryAttempts}
                  min={0}
                  max={5}
                  {...fieldMeta("sourceRetryAttempts")}
                  onChange={(value) => update("sourceRetryAttempts", value)}
                />
                <NumberSetting
                  label="熔断失败次数"
                  value={values.sourceCircuitBreakerFailures}
                  min={1}
                  {...fieldMeta("sourceCircuitBreakerFailures")}
                  onChange={(value) => update("sourceCircuitBreakerFailures", value)}
                />
                <NumberSetting
                  label="熔断冷却时间（秒）"
                  value={values.sourceCircuitBreakerCooldownSec}
                  min={1}
                  {...fieldMeta("sourceCircuitBreakerCooldownSec")}
                  onChange={(value) => update("sourceCircuitBreakerCooldownSec", value)}
                />
              </SettingsGrid>
            </CardContent>
          </Card>
          <ProviderPanel embedded />
        </TabsContent>

        <SettingsTab value="analysis" title="音频分析">
          <SettingsGrid>
            <NumberSetting
              label="分析并发数"
              value={values.analysisWorkers}
              min={1}
              {...fieldMeta("analysisWorkers")}
              onChange={(value) => update("analysisWorkers", value)}
            />
            <NumberSetting
              label="重复指纹阈值"
              value={values.fingerprintThreshold}
              min={0}
              max={1}
              step={0.01}
              {...fieldMeta("fingerprintThreshold")}
              onChange={(value) => update("fingerprintThreshold", value)}
            />
          </SettingsGrid>
        </SettingsTab>

        <SettingsTab value="ai" title="AI 元数据建议">
          <div className="space-y-5">
            <BooleanSetting
              label="启用 AI"
              checked={values.aiEnabled}
              {...fieldMeta("aiEnabled")}
              onChange={(value) => update("aiEnabled", value)}
            />
            <SettingsGrid>
              <TextSetting
                label="服务地址"
                value={values.aiBaseUrl}
                {...fieldMeta("aiBaseUrl")}
                onChange={(value) => update("aiBaseUrl", value)}
              />
              <TextSetting
                label="模型"
                value={values.aiModel}
                {...fieldMeta("aiModel")}
                onChange={(value) => update("aiModel", value)}
              />
              <NumberSetting
                label="请求超时（秒）"
                value={values.aiTimeoutSec}
                min={1}
                {...fieldMeta("aiTimeoutSec")}
                onChange={(value) => update("aiTimeoutSec", value)}
              />
              <SettingShell label="API Key" locked={locked.has("ai.apiKey")}>
                <Input
                  type="password"
                  value={apiKey}
                  disabled={locked.has("ai.apiKey") || clearApiKey}
                  onChange={(event) => setApiKey(event.target.value)}
                  placeholder={
                    settings.aiApiKeyConfigured ? "已配置，留空则保持不变" : "输入 API Key"
                  }
                />
              </SettingShell>
            </SettingsGrid>
            {settings.aiApiKeyConfigured && !locked.has("ai.apiKey") ? (
              <BooleanSetting
                label="清除已保存的 API Key"
                checked={clearApiKey}
                onChange={setClearApiKey}
              />
            ) : null}
          </div>
        </SettingsTab>

        <SettingsTab value="organizer" title="文件整理">
          <div className="space-y-5">
            <SettingShell label="默认路径模板">
              <Textarea
                value={values.organizerTemplate}
                onChange={(event) => update("organizerTemplate", event.target.value)}
                rows={3}
              />
            </SettingShell>
            <BooleanSetting
              label="使用跨平台安全文件名"
              checked={values.organizerCrossPlatformSafe}
              onChange={(value) => update("organizerCrossPlatformSafe", value)}
            />
          </div>
        </SettingsTab>

        <SettingsTab value="playback" title="播放与转码">
          <SettingsGrid>
            <NumberSetting
              label="转码并发数"
              value={values.transcodeWorkers}
              min={1}
              {...fieldMeta("transcodeWorkers")}
              onChange={(value) => update("transcodeWorkers", value)}
            />
            <NumberSetting
              label="转码缓存上限（字节）"
              value={values.transcodeCacheMaxBytes}
              min={1}
              {...fieldMeta("transcodeCacheMaxBytes")}
              onChange={(value) => update("transcodeCacheMaxBytes", value)}
            />
          </SettingsGrid>
          <p className="mt-3 text-xs text-muted-foreground">
            当前上限：{formatBytes(values.transcodeCacheMaxBytes)}
          </p>
        </SettingsTab>

        <SettingsTab value="system" title="系统信息">
          <div className="divide-y rounded-xl border">
            {[
              ["平台", settings.infrastructure.platform],
              ["监听地址", `${settings.infrastructure.host}:${settings.infrastructure.port}`],
              ["数据库", settings.infrastructure.databasePath],
              ["FFmpeg", settings.infrastructure.ffmpegPath],
              ["音频指纹工具", settings.infrastructure.fpcalcPath],
              ["转码缓存", settings.infrastructure.transcodeCacheDir],
            ].map(([label, value]) => (
              <div
                key={label}
                className="flex flex-col justify-between gap-1 px-4 py-3 sm:flex-row sm:items-center"
              >
                <span className="text-sm text-muted-foreground">{label}</span>
                <span className="break-all font-mono text-sm">{value}</span>
              </div>
            ))}
          </div>
          <p className="mt-3 text-xs text-muted-foreground">这些参数由部署配置管理。</p>
        </SettingsTab>
      </Tabs>
    </div>
  );
}

function SettingsTab({
  value,
  title,
  children,
}: {
  value: string;
  title: string;
  children: ReactNode;
}) {
  return (
    <TabsContent value={value} className="mt-5">
      <Card>
        <CardHeader>
          <CardTitle>{title}</CardTitle>
        </CardHeader>
        <CardContent>{children}</CardContent>
      </Card>
    </TabsContent>
  );
}

function SettingsGrid({ children }: { children: ReactNode }) {
  return <div className="grid gap-5 md:grid-cols-2 xl:grid-cols-3">{children}</div>;
}

function SettingShell({
  label,
  locked,
  restart,
  children,
}: {
  label: string;
  locked?: boolean;
  restart?: boolean;
  children: ReactNode;
}) {
  return (
    <div className="space-y-2">
      <div className="flex min-h-5 items-center gap-2.5">
        <Label>{label}</Label>
        {locked ? <Badge variant="outline">环境变量</Badge> : null}
        {restart ? <Badge variant="secondary">重启后生效</Badge> : null}
      </div>
      {children}
    </div>
  );
}

function NumberSetting({
  label,
  value,
  onChange,
  locked,
  restart,
  ...input
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
  locked?: boolean;
  restart?: boolean;
  min?: number;
  max?: number;
  step?: number;
}) {
  return (
    <SettingShell label={label} locked={locked} restart={restart}>
      <Input
        type="number"
        value={value}
        disabled={locked}
        onChange={(event) => onChange(Number(event.target.value))}
        {...input}
      />
    </SettingShell>
  );
}

function TextSetting({
  label,
  value,
  onChange,
  locked,
  restart,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  locked?: boolean;
  restart?: boolean;
}) {
  return (
    <SettingShell label={label} locked={locked} restart={restart}>
      <Input value={value} disabled={locked} onChange={(event) => onChange(event.target.value)} />
    </SettingShell>
  );
}

function BooleanSetting({
  label,
  checked,
  onChange,
  locked,
  restart,
}: {
  label: string;
  checked: boolean;
  onChange: (value: boolean) => void;
  locked?: boolean;
  restart?: boolean;
}) {
  return (
    <div className="flex items-center justify-between gap-4 rounded-xl border px-4 py-3">
      <div className="flex items-center gap-2.5">
        <Label>{label}</Label>
        {locked ? <Badge variant="outline">环境变量</Badge> : null}
        {restart ? <Badge variant="secondary">重启后生效</Badge> : null}
      </div>
      <Switch checked={checked} disabled={locked} onCheckedChange={onChange} />
    </div>
  );
}
