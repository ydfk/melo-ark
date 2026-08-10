import { render, screen } from "@testing-library/react";
import { describe, expect, test } from "vitest";

import { OperationPreview } from "./operation-preview";

describe("OperationPreview", () => {
  test("shows tag diffs before any write is confirmed", () => {
    render(
      <OperationPreview
        operation={{
          id: "operation-1",
          kind: "tag_edit",
          status: "previewed",
          items: [
            {
              id: "item-1",
              status: "previewed",
              sourcePath: "/music/source/song.flac",
              diffs: [{ field: "artist", before: "周杰倫", after: "周杰伦" }],
            },
          ],
        }}
      />
    );

    expect(screen.getByText("周杰倫")).toBeInTheDocument();
    expect(screen.getByText("周杰伦")).toBeInTheDocument();
    expect(screen.getByText("previewed")).toBeInTheDocument();
  });

  test("makes a hardlink conflict visible", () => {
    render(
      <OperationPreview
        operation={{
          id: "operation-2",
          kind: "organize",
          status: "previewed",
          items: [
            {
              id: "item-2",
              status: "previewed",
              sourcePath: "/music/source/song.flac",
              targetPath: "/music/managed/song.flac",
              diffs: [],
              preflight: {
                sameFilesystem: true,
                targetExists: true,
                sameInode: false,
                pathConflict: true,
                canApply: false,
              },
            },
          ],
        }}
      />
    );

    expect(screen.getByText("路径冲突")).toBeInTheDocument();
    expect(screen.getByText("同一文件系统")).toBeInTheDocument();
  });
});
