"use client";

import * as React from "react";
import * as ScrollAreaPrimitive from "@radix-ui/react-scroll-area";
import { cn } from "@/lib/utils";

export type VirtualRange = { startIndex: number; endIndex: number };

export type LoadRangeResult<T> = {
  offset: number;
  items: T[];
  totalCount: number;
  filteredCount: number;
  // Optional extra metadata from backend (passthrough)
  meta?: Record<string, unknown>;
};

type VirtualizedRemoteListProps<T> = {
  /** Changing this resets cache + scroll position */
  queryKey: string;
  /**
   * Reload trigger that refreshes data **in place** (keeps scroll position and does not clear cache/counts first).
   * Useful for live updates (SSE/polling) to avoid flicker.
   */
  reloadToken?: string | number;
  height: number;
  rowHeight: number;
  /** Optional sticky header rendered inside the same scroll viewport */
  header?: React.ReactNode;
  headerHeight?: number;
  /** If set, forces a wider content area to enable horizontal scroll inside the list */
  contentWidth?: number | string;
  /** Shows a bottom horizontal scrollbar (useful on desktop) */
  showHorizontalScrollbar?: boolean;
  pageSize?: number;
  overscan?: number;
  className?: string;

  /** Fetches a page by offset/limit. Must support random access offsets. */
  loadRange: (offset: number, limit: number) => Promise<LoadRangeResult<T>>;

  /** Render a single row (item might be undefined while loading). */
  renderRow: (args: {
    index: number;
    item: T | undefined;
    style: React.CSSProperties;
    isLoaded: boolean;
  }) => React.ReactNode;

  /** Called when counts change (from backend). */
  onCountsChange?: (counts: { total: number; filtered: number; meta?: Record<string, unknown> }) => void;
  /** Called when visible range changes */
  onRangeChange?: (range: VirtualRange) => void;
};

export function VirtualizedRemoteList<T>({
  queryKey,
  reloadToken,
  height,
  rowHeight,
  header,
  headerHeight = 0,
  contentWidth,
  showHorizontalScrollbar,
  pageSize = 200,
  overscan = 10,
  className,
  loadRange,
  renderRow,
  onCountsChange,
  onRangeChange,
}: VirtualizedRemoteListProps<T>) {
  const viewportRef = React.useRef<HTMLDivElement | null>(null);
  const inflightPages = React.useRef<Set<number>>(new Set());
  const onCountsChangeRef = React.useRef<typeof onCountsChange>(onCountsChange);
  const onRangeChangeRef = React.useRef<typeof onRangeChange>(onRangeChange);

  React.useEffect(() => {
    onCountsChangeRef.current = onCountsChange;
  }, [onCountsChange]);

  React.useEffect(() => {
    onRangeChangeRef.current = onRangeChange;
  }, [onRangeChange]);

  const [scrollTop, setScrollTop] = React.useState(0);
  const [counts, setCounts] = React.useState<{ total: number; filtered: number }>({
    total: 0,
    filtered: 0,
  });
  const [, forceRender] = React.useState(0);
  const cache = React.useRef<Map<number, T>>(new Map());

  const totalRows = counts.filtered;

  const requestPage = React.useCallback(
    async (pageIndex: number) => {
      if (inflightPages.current.has(pageIndex)) return;
      inflightPages.current.add(pageIndex);
      try {
        const offset = pageIndex * pageSize;
        const res = await loadRange(offset, pageSize);

        // counts can change due to live traffic; keep it monotonic-ish but accept backend truth
        setCounts((prev) => {
          if (prev.total === res.totalCount && prev.filtered === res.filteredCount) return prev;
          return { total: res.totalCount, filtered: res.filteredCount };
        });
        onCountsChangeRef.current?.({ total: res.totalCount, filtered: res.filteredCount, meta: res.meta });

        // populate cache
        for (let i = 0; i < res.items.length; i++) {
          cache.current.set(res.offset + i, res.items[i]);
        }
        forceRender((v) => v + 1);
      } finally {
        inflightPages.current.delete(pageIndex);
      }
    },
    [loadRange, pageSize]
  );

  // Reset when query changes
  React.useEffect(() => {
    inflightPages.current.clear();
    cache.current.clear();
    setCounts({ total: 0, filtered: 0 });
    forceRender((v) => v + 1);
    setScrollTop(0);
    if (viewportRef.current) viewportRef.current.scrollTop = 0;

    // Prime first page
    void requestPage(0);
  }, [queryKey, requestPage]);

  const onScroll = React.useCallback((e: React.UIEvent<HTMLDivElement>) => {
    setScrollTop(e.currentTarget.scrollTop);
  }, []);

  const effectiveHeaderHeight = header ? headerHeight : 0;
  const effectiveViewportHeight = Math.max(0, height - effectiveHeaderHeight);
  const effectiveScrollTop = Math.max(0, scrollTop - effectiveHeaderHeight);

  // Calculate visible range
  const startIndex = Math.max(0, Math.floor(effectiveScrollTop / rowHeight) - overscan);
  const endIndex = Math.min(
    Math.max(0, totalRows - 1),
    Math.floor((effectiveScrollTop + effectiveViewportHeight) / rowHeight) + overscan
  );

  React.useEffect(() => {
    if (totalRows <= 0) return;
    onRangeChangeRef.current?.({ startIndex, endIndex });

    // Request pages intersecting the visible range
    const firstPage = Math.floor(startIndex / pageSize);
    const lastPage = Math.floor(endIndex / pageSize);
    for (let p = firstPage; p <= lastPage; p++) void requestPage(p);
  }, [startIndex, endIndex, totalRows, pageSize, requestPage]);

  // Soft reload (in place): refetch visible pages without resetting scroll/caches.
  React.useEffect(() => {
    if (reloadToken === undefined) return;
    inflightPages.current.clear();

    // Always refresh first page to refresh counts/meta.
    void requestPage(0);

    if (totalRows <= 0) return;
    const firstPage = Math.floor(startIndex / pageSize);
    const lastPage = Math.floor(endIndex / pageSize);
    for (let p = firstPage; p <= lastPage; p++) void requestPage(p);
  }, [reloadToken, endIndex, pageSize, requestPage, startIndex, totalRows]);

  const totalHeight = effectiveHeaderHeight + totalRows * rowHeight;
  const rowsToRender: number[] = [];
  for (let i = startIndex; i <= endIndex; i++) rowsToRender.push(i);

  const widthStyle: React.CSSProperties | undefined =
    contentWidth !== undefined
      ? { width: typeof contentWidth === "number" ? `${contentWidth}px` : contentWidth }
      : undefined;

  const wantsHorizontal = showHorizontalScrollbar ?? contentWidth !== undefined;

  return (
    <ScrollAreaPrimitive.Root
      className={cn("relative", className)}
      style={{
        height,
        // Used to offset the vertical scrollbar so it doesn't cover the sticky header
        ["--vr-header-height" as unknown as string]: `${effectiveHeaderHeight}px`,
      }}
    >
      <ScrollAreaPrimitive.Viewport
        ref={viewportRef}
        className={cn(
          "size-full rounded-[inherit] outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50",
          // reserve space so scrollbars don't cover last columns/rows
          wantsHorizontal ? "pr-3 pb-3" : "pr-3"
        )}
        onScroll={onScroll}
      >
        <div style={{ height: totalHeight, position: "relative", ...widthStyle }}>
          {header ? (
            <div
              style={{ height: effectiveHeaderHeight, top: 0 }}
              className="sticky top-0 z-30 bg-background"
            >
              {header}
            </div>
          ) : null}

          <div style={{ position: "absolute", top: effectiveHeaderHeight, left: 0, right: 0, ...widthStyle }}>
            {rowsToRender.map((index) => {
              const item = cache.current.get(index);
              const isLoaded = item !== undefined;
              const style: React.CSSProperties = {
                position: "absolute",
                top: index * rowHeight,
                left: 0,
                height: rowHeight,
                ...widthStyle,
              };
              return (
                <React.Fragment key={`${queryKey}:${index}`}>
                  {renderRow({ index, item, style, isLoaded })}
                </React.Fragment>
              );
            })}
          </div>
        </div>
      </ScrollAreaPrimitive.Viewport>

      <ScrollAreaPrimitive.ScrollAreaScrollbar
        orientation="vertical"
        className="relative z-20 flex touch-none select-none p-px transition-colors w-2.5 border-l border-l-transparent bg-sky-100 mt-[var(--vr-header-height)] h-[calc(100%-var(--vr-header-height))]"
      >
        <ScrollAreaPrimitive.ScrollAreaThumb className="bg-white relative flex-1 rounded-full border border-sky-200 shadow-sm" />
      </ScrollAreaPrimitive.ScrollAreaScrollbar>

      {wantsHorizontal ? (
        <ScrollAreaPrimitive.ScrollAreaScrollbar
          orientation="horizontal"
          className="relative z-20 flex touch-none select-none p-px transition-colors w-full h-2.5 border-t border-t-transparent"
        >
          <ScrollAreaPrimitive.ScrollAreaThumb className="bg-border relative flex-1 rounded-full" />
        </ScrollAreaPrimitive.ScrollAreaScrollbar>
      ) : null}
      <ScrollAreaPrimitive.Corner />
    </ScrollAreaPrimitive.Root>
  );
}

