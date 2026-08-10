import { cn } from "@/lib/utils";

export function BrandMark({ className }: { className?: string }) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        "relative grid size-10 shrink-0 place-items-center overflow-hidden rounded-xl bg-primary text-primary-foreground shadow-lg shadow-primary/20",
        className
      )}
    >
      <span className="absolute inset-[5px] rounded-full border border-current/55" />
      <svg viewBox="0 0 24 24" className="relative size-5" fill="none">
        <path
          d="M5 15.5V8.3L8.3 12 12 6.5l3.7 5.5L19 8.3v7.2c-3.9 2.7-10.1 2.7-14 0Z"
          stroke="currentColor"
          strokeWidth="1.8"
          strokeLinejoin="round"
        />
      </svg>
    </span>
  );
}
