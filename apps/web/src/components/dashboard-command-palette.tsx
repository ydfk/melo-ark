import {
  CloudCog,
  CopyCheck,
  LayoutDashboard,
  Library,
  ListMusic,
  LogOut,
  RadioTower,
  RefreshCw,
  Search,
  Settings,
  Trash2,
} from "lucide-react";
import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
  CommandShortcut,
} from "@/components/ui/command";

export type DashboardTab =
  | "dashboard"
  | "library"
  | "tasks"
  | "providers"
  | "duplicates"
  | "playback"
  | "trash"
  | "settings";

type DashboardCommandPaletteProps = {
  onNavigate: (tab: DashboardTab) => void;
  onRefresh: () => void;
  onLogout: () => void;
};

const destinations = [
  { value: "dashboard" as const, label: "打开总览", icon: LayoutDashboard, shortcut: "⌃1" },
  { value: "library" as const, label: "打开曲库", icon: Library, shortcut: "⌃2" },
  { value: "tasks" as const, label: "打开任务中心", icon: ListMusic, shortcut: "⌃3" },
  { value: "providers" as const, label: "打开 Provider 舱", icon: CloudCog, shortcut: "⌃4" },
  { value: "duplicates" as const, label: "打开重复分析", icon: CopyCheck, shortcut: "⌃5" },
  { value: "playback" as const, label: "打开播放中心", icon: RadioTower, shortcut: "⌃6" },
  { value: "trash" as const, label: "打开回收站", icon: Trash2, shortcut: "⌃7" },
  { value: "settings" as const, label: "打开设置", icon: Settings, shortcut: "⌃8" },
];

export function DashboardCommandPalette({
  onNavigate,
  onRefresh,
  onLogout,
}: DashboardCommandPaletteProps) {
  const [open, setOpen] = useState(false);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setOpen((current) => !current);
        return;
      }
      if (event.ctrlKey && !event.metaKey && !event.altKey) {
        const destination = destinations[Number(event.key) - 1];
        if (destination) {
          event.preventDefault();
          onNavigate(destination.value);
        }
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onNavigate]);

  function run(command: () => void) {
    setOpen(false);
    command();
  }

  return (
    <>
      <Button
        variant="outline"
        size="sm"
        className="hidden gap-2 bg-card/70 text-muted-foreground sm:flex"
        onClick={() => setOpen(true)}
      >
        <Search />
        快速前往
        <kbd className="rounded border bg-muted px-1.5 font-mono text-[10px]">⌘ K</kbd>
      </Button>
      <Button
        variant="ghost"
        size="icon"
        className="sm:hidden"
        onClick={() => setOpen(true)}
        aria-label="打开命令面板"
      >
        <Search />
      </Button>
      <CommandDialog open={open} onOpenChange={setOpen}>
        <CommandInput placeholder="输入页面或操作名称…" />
        <CommandList>
          <CommandEmpty>没有匹配的页面或操作。</CommandEmpty>
          <CommandGroup heading="前往">
            {destinations.map(({ value, label, icon: Icon, shortcut }) => (
              <CommandItem key={value} onSelect={() => run(() => onNavigate(value))}>
                <Icon />
                {label}
                <CommandShortcut>{shortcut}</CommandShortcut>
              </CommandItem>
            ))}
          </CommandGroup>
          <CommandSeparator />
          <CommandGroup heading="服务">
            <CommandItem onSelect={() => run(onRefresh)}>
              <RefreshCw />
              刷新服务状态
            </CommandItem>
            <CommandItem onSelect={() => run(onLogout)}>
              <LogOut />
              退出登录
            </CommandItem>
          </CommandGroup>
        </CommandList>
      </CommandDialog>
    </>
  );
}
