import { Disc3 } from "lucide-react";
import { useEffect, useState } from "react";

import { apiBaseURL } from "@/lib/api";
import { cn } from "@/lib/utils";

export function CatalogArtwork({
  mediaId,
  alt,
  className,
}: {
  mediaId?: string;
  alt: string;
  className?: string;
}) {
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    setFailed(false);
  }, [mediaId]);

  if (!mediaId || failed) {
    return (
      <div
        className={cn(
          "grid place-items-center bg-[radial-gradient(circle_at_42%_35%,oklch(0.5_0.12_240/0.7),transparent_22%),linear-gradient(145deg,oklch(0.19_0.03_265),oklch(0.08_0.015_270))] text-white/55",
          className
        )}
        aria-label={`${alt}暂无封面`}
      >
        <Disc3 className="size-1/3" aria-hidden="true" />
      </div>
    );
  }

  return (
    <img
      src={`${apiBaseURL()}/catalog/artwork/${mediaId}`}
      alt={alt}
      loading="lazy"
      className={cn("object-cover", className)}
      onError={() => setFailed(true)}
    />
  );
}
