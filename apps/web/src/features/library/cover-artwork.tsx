import { Disc3 } from "lucide-react";
import { useEffect, useState } from "react";

import { getPlayToken } from "@/lib/api/methods/library";
import { cn } from "@/lib/utils";

const artworkTokens = new Map<string, string>();

type CoverArtworkProps = {
  mediaId: string;
  hasArtwork: boolean;
  alt: string;
  className?: string;
};

export function CoverArtwork({ mediaId, hasArtwork, alt, className }: CoverArtworkProps) {
  const [source, setSource] = useState<string>();
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    if (!hasArtwork) return;
    let active = true;
    const cached = artworkTokens.get(mediaId);
    if (cached) {
      setSource(`/api/artwork/${mediaId}?token=${encodeURIComponent(cached)}`);
      return;
    }
    void getPlayToken(mediaId)
      .send()
      .then(({ token }) => {
        artworkTokens.set(mediaId, token);
        if (active) setSource(`/api/artwork/${mediaId}?token=${encodeURIComponent(token)}`);
      })
      .catch(() => active && setFailed(true));
    return () => {
      active = false;
    };
  }, [hasArtwork, mediaId]);

  if (!hasArtwork || failed || !source) {
    return (
      <div
        className={cn(
          "record-cover grid place-items-center overflow-hidden bg-[radial-gradient(circle_at_center,var(--color-primary)_0_7%,transparent_8%_16%,var(--color-border)_17%_18%,transparent_19%_100%),linear-gradient(145deg,var(--color-muted),var(--color-card))] text-primary",
          className
        )}
        aria-label={`${alt}暂无封面`}
      >
        <Disc3 className="size-1/3 opacity-70" aria-hidden="true" />
      </div>
    );
  }

  return (
    <img
      src={source}
      alt={alt}
      loading="lazy"
      className={cn("record-cover object-cover", className)}
      onError={() => setFailed(true)}
    />
  );
}
