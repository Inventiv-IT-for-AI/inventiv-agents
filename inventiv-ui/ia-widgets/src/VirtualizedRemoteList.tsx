"use client";

import * as React from "react";
import * as ScrollAreaPrimitive from "@radix-ui/react-scroll-area";
import { cn } from "./utils/cn";

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
  const headerRef = React.useRef<HTMLDivElement | null>(null);
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
  const [measuredHeaderHeight, setMeasuredHeaderHeight] = React.useState<number>(header ? headerHeight : 0);
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

        setCounts((prev) => {
          if (prev.total === res.totalCount && prev.filtered === res.filteredCount) return prev;
          return { total: res.totalCount, filtered: res.filteredCount };
        });
        onCountsChangeRef.current?.({ total: res.totalCount, filtered: res.filteredCount, meta: res.meta });

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
    void requestPage(0);
  }, [queryKey, requestPage]);

  // Refresh in place when reloadToken changes
  React.useEffect(() => {
    if (reloadToken === undefined) return;
    const firstPage = Math.floor(Math.max(0, Math.floor(scrollTop / rowHeight) - overscan) / pageSize);
    const lastPage = Math.floor(
      Math.min(Math.max(0, totalRows - 1), Math.floor((scrollTop + height) / rowHeight) + overscan) / pageSize
    );
    for (let p = firstPage; p <= lastPage; p++) void requestPage(p);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [reloadToken]);

  const onScroll = React.useCallback((e: React.UIEvent<HTMLDivElement>) => {
    setScrollTop(e.currentTarget.scrollTop);
  }, []);

  // Measure header height (more robust than relying on a hardcoded value).
  React.useEffect(() => {
    if (!header) {
      setMeasuredHeaderHeight(0);
      return;
    }
    const el = headerRef.current;
    if (!el) return;
    const measure = () => setMeasuredHeaderHeight(el.getBoundingClientRect().height || headerHeight || 0);
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => measure());
    ro.observe(el);
    return () => ro.disconnect();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [header]);

  const effectiveHeaderHeight = header ? (measuredHeaderHeight || headerHeight || 0) : 0;
  const effectiveViewportHeight = Math.max(0, height - effectiveHeaderHeight);
  const effectiveScrollTop = Math.max(0, scrollTop - effectiveHeaderHeight);

  const startIndex = Math.max(0, Math.floor(effectiveScrollTop / rowHeight) - overscan);
  const endIndex = Math.min(
    Math.max(0, totalRows - 1),
    Math.floor((effectiveScrollTop + effectiveViewportHeight) / rowHeight) + overscan
  );

  React.useEffect(() => {
    if (totalRows <= 0) return;
    onRangeChangeRef.current?.({ startIndex, endIndex });
    const firstPage = Math.floor(startIndex / pageSize);
    const lastPage = Math.floor(endIndex / pageSize);
    for (let p = firstPage; p <= lastPage; p++) void requestPage(p);
  }, [startIndex, endIndex, totalRows, pageSize, requestPage]);

  const innerHeight = Math.max(0, totalRows * rowHeight + effectiveHeaderHeight);

  const items: React.ReactNode[] = [];
  for (let i = startIndex; i <= endIndex; i++) {
    const item = cache.current.get(i);
    items.push(
      renderRow({
        index: i,
        item,
        isLoaded: item !== undefined,
        style: { position: "absolute", top: i * rowHeight, height: rowHeight, left: 0, right: 0 },
      })
    );
  }

  const outerWidth = typeof contentWidth === "number" ? `${contentWidth}px` : (contentWidth ?? "100%");
  const wrapperStyle: React.CSSProperties = {
    position: "relative",
    width: outerWidth,
    minWidth: "100%",
  };
  const listStyle: React.CSSProperties = {
    position: "relative",
    height: Math.max(0, totalRows * rowHeight),
    width: "100%",
  };

  return (
    <ScrollAreaPrimitive.Root 
      className={cn("relative w-full overflow-hidden", className)} 
      style={{ height }}
      suppressHydrationWarning
    >
      <ScrollAreaPrimitive.Viewport
        ref={viewportRef}
        className="h-full w-full"
        onScroll={onScroll}
        style={{ height }}
        suppressHydrationWarning
      >
        <div style={wrapperStyle}>
          {header ? (
            <div
              ref={headerRef}
              style={{ position: "sticky", top: 0, zIndex: 20 }}
              className="bg-background"
            >
              {header}
            </div>
          ) : null}
          <div style={listStyle}>{items}</div>
        </div>
      </ScrollAreaPrimitive.Viewport>
      <ScrollAreaPrimitive.Scrollbar orientation="vertical" />
      {showHorizontalScrollbar ? <ScrollAreaPrimitive.Scrollbar orientation="horizontal" /> : null}
      <ScrollAreaPrimitive.Corner />
    </ScrollAreaPrimitive.Root>
  );
}


