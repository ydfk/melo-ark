import { alovaInstance } from "..";

export type CatalogTrack = {
  id: string;
  mediaId: string;
  title: string;
  artist: string;
  albumId?: string;
  album: string;
  year?: number;
  durationMs?: number;
  hasLyrics: boolean;
  hasArtwork: boolean;
  artworkMediaId?: string;
};

export type CatalogTrackPage = {
  items: CatalogTrack[];
  page: number;
  perPage: number;
  total: number;
};

export type CatalogAlbum = {
  id: string;
  title: string;
  artist: string;
  year?: number;
  trackCount: number;
  durationMs: number;
  coverMediaId?: string;
};

export type CatalogLyrics = {
  trackId: string;
  content: string;
  language?: string;
  translatedContent?: string;
  format: "plain" | "lrc";
  synced: boolean;
};

export type PublicPlayToken = {
  token: string;
  expiresIn: number;
};

export const getCatalogTracks = (page = 1, perPage = 48, search = "") =>
  alovaInstance.Get<CatalogTrackPage>("/catalog/tracks", {
    params: { page, perPage, search: search.trim() || undefined },
  });

export const getCatalogAlbums = (limit = 24) =>
  alovaInstance.Get<CatalogAlbum[]>("/catalog/albums", { params: { limit } });

export const getCatalogLyrics = (trackId: string) =>
  alovaInstance.Get<CatalogLyrics | null>(`/catalog/tracks/${trackId}/lyrics`);

export const getPublicPlayToken = (mediaId: string) =>
  alovaInstance.Post<PublicPlayToken>(`/catalog/media/${mediaId}/play-token`);
