"use client";

import * as React from "react";
import { GripVertical, Settings2 } from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { VirtualizedRemoteList, type LoadRangeResult } from "@/components/shared/VirtualizedRemoteList";

type ColumnId = string;

export type DataTableColumn<Row> = {
  id: ColumnId;
  label: string;
  width?: number; // px
  minWidth?: number; // px
  maxWidth?: number; // px
  defaultHidden?: boolean;
  disableHiding?: boolean;
  disableResize?: boolean;
  disableReorder?: boolean;
  align?: "left" | "center" | "right";
  headerClassName?: string;
  cellClassName?: string;
  header?: React.ReactNode | ((col: DataTableColumn<Row>) => React.ReactNode);
  cell: (args: { row: Row; rowIndex: number }) => React.ReactNode;
};

type PersistedPrefs = {
  v: 1;
  order?: ColumnId[];
  hidden?: ColumnId[];
  widths?: Record<ColumnId, number>;
};

function storageKey(listId: string) {
  return `vdt:prefs:${listId}`;
}

function safeReadPrefs(listId: string): PersistedPrefs | null {
  if (typeof window === "undefined") return null;
  try {
    const raw = window.localStorage.getItem(storageKey(listId));
    if (!raw) return null;
    const parsed = JSON.parse(raw) as PersistedPrefs;
    if (!parsed || parsed.v !== 1) return null;

    // Validate shape defensively (localStorage can be stale/corrupted across versions)
    const out: PersistedPrefs = { v: 1 };
    if (parsed.order !== undefined) {
      if (!Array.isArray(parsed.order) || !parsed.order.every((x) => typeof x === "string")) return null;
      out.order = parsed.order;
    }
    if (parsed.hidden !== undefined) {
      if (!Array.isArray(parsed.hidden) || !parsed.hidden.every((x) => typeof x === "string")) return null;
      out.hidden = parsed.hidden;
    }
    if (parsed.widths !== undefined) {
      if (typeof parsed.widths !== "object" || parsed.widths === null || Array.isArray(parsed.widths)) return null;
      const widths: Record<string, number> = {};
      for (const [k, v] of Object.entries(parsed.widths as Record<string, unknown>)) {
        if (typeof v === "number" && Number.isFinite(v)) widths[k] = v;
      }
      out.widths = widths;
    }

    return out;
  } catch {
    return null;
  }
}

function safeWritePrefs(listId: string, prefs: PersistedPrefs) {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(storageKey(listId), JSON.stringify(prefs));
  } catch {
    // ignore
  }
}

function clamp(n: number, min: number, max: number) {
  return Math.max(min, Math.min(max, n));
}

function hash32(input: string): string {
  // Fast non-crypto 32-bit hash (FNV-1a)
  let h = 0x811c9dc5;
  for (let i = 0; i < input.length; i++) {
    h ^= input.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  // unsigned, compact
  return (h >>> 0).toString(36);
}

// Stable, O(1) signature for local `rows` based on array identity.
// This avoids freezing the UI by hashing huge arrays/strings.
const __rowsRefIds = new WeakMap<object, string>();
let __rowsRefSeq = 0;
function rowsRefId(rows: unknown[]): string {
  const key = rows as unknown as object;
  const existing = __rowsRefIds.get(key);
  if (existing) return existing;
  const next = (++__rowsRefSeq).toString(36);
  __rowsRefIds.set(key, next);
  return next;
}

export type VirtualizedDataTableProps<Row> = {
  listId: string;
  /** Optional key to force list reload (clears virtualization cache) when data changes */
  dataKey?: string;
  title?: React.ReactNode;
  rightHeader?: React.ReactNode;
  /** Optional override for the left "Total ..." text */
  leftMeta?: React.ReactNode;
  /** Notified when backend counts change */
  onCountsChange?: (counts: { total: number; filtered: number }) => void;

  height: number;
  rowHeight: number;
  pageSize?: number;
  overscan?: number;

  /** Show a leading row number column (#). Default: true */
  showRowNumbers?: boolean;

  columns: DataTableColumn<Row>[];
  /** Use either `loadRange` (remote) or `rows` (local) */
  loadRange?: (offset: number, limit: number) => Promise<LoadRangeResult<Row>>;
  rows?: Row[];
  getRowKey?: (row: Row, rowIndex: number) => string;
  onRowClick?: (row: Row) => void;

  className?: string;
};

export function VirtualizedDataTable<Row>({
  listId,
  dataKey,
  title,
  rightHeader,
  leftMeta,
  onCountsChange,
  height,
  rowHeight,
  pageSize,
  overscan,
  showRowNumbers = true,
  columns,
  loadRange,
  rows,
  getRowKey,
  onRowClick,
  className,
}: VirtualizedDataTableProps<Row>) {
  const rowNumCol = React.useMemo((): DataTableColumn<Row> => {
    return {
      id: "__rownum__",
      label: "#",
      width: 56,
      minWidth: 48,
      maxWidth: 80,
      disableHiding: true,
      disableResize: true,
      disableReorder: true,
      align: "right",
      header: <span className="text-muted-foreground">#</span>,
      cell: ({ rowIndex }) => (
        <span className="font-mono text-xs text-muted-foreground tabular-nums">{rowIndex + 1}</span>
      ),
    };
  }, []);

  const effectiveColumns = React.useMemo(() => {
    return showRowNumbers ? [rowNumCol, ...columns] : columns;
  }, [columns, rowNumCol, showRowNumbers]);

  // IMPORTANT:
  // Some screens rebuild `columns` on every render (because cell renderers close over handlers).
  // We must NOT treat that as "columns changed", otherwise we re-hydrate prefs in a loop and freeze the UI.
  // Use a stable signature based on column ids + defaults instead.
  const columnsMetaKey = React.useMemo(() => {
    return effectiveColumns
      .map((c) => `${c.id}:${c.defaultHidden ? 1 : 0}:${c.width ?? ""}:${c.minWidth ?? ""}:${c.maxWidth ?? ""}`)
      .join("|");
  }, [effectiveColumns]);

  const baseColumnsById = React.useMemo(() => {
    const m = new Map<string, DataTableColumn<Row>>();
    for (const c of effectiveColumns) m.set(c.id, c);
    return m;
  }, [effectiveColumns]);

  const [settingsOpen, setSettingsOpen] = React.useState(false);
  const [tableCounts, setTableCounts] = React.useState<{ total: number; filtered: number }>({ total: 0, filtered: 0 });
  const [prefsLoaded, setPrefsLoaded] = React.useState(false);
  const [isResizing, setIsResizing] = React.useState(false);

  const [order, setOrder] = React.useState<ColumnId[]>(() => effectiveColumns.map((c) => c.id));
  const [hidden, setHidden] = React.useState<Set<ColumnId>>(() => new Set(effectiveColumns.filter((c) => c.defaultHidden).map((c) => c.id)));
  const [widths, setWidths] = React.useState<Record<ColumnId, number>>(() => {
    const w: Record<string, number> = {};
    for (const c of effectiveColumns) w[c.id] = c.width ?? 160;
    return w;
  });

  // Load persisted prefs once columns are known (client-side)
  React.useEffect(() => {
    setPrefsLoaded(false);
    try {
      const prefs = safeReadPrefs(listId);
      const colIds = effectiveColumns.map((c) => c.id);
      const colIdSet = new Set(colIds);
      if (!prefs) {
        setOrder(colIds);
        setHidden(new Set(effectiveColumns.filter((c) => c.defaultHidden).map((c) => c.id)));
        setWidths(() => {
          const w: Record<string, number> = {};
          for (const c of effectiveColumns) w[c.id] = c.width ?? 160;
          return w;
        });
        setPrefsLoaded(true);
        return;
      }

      const nextOrder: ColumnId[] = [];
      for (const id of prefs.order ?? []) if (colIdSet.has(id)) nextOrder.push(id);
      for (const id of colIds) if (!nextOrder.includes(id)) nextOrder.push(id);
      // Keep row numbers column always first (even when it was added after prefs were saved)
      if (showRowNumbers) {
        const idx = nextOrder.indexOf("__rownum__");
        if (idx >= 0) nextOrder.splice(idx, 1);
        nextOrder.unshift("__rownum__");
      }
      setOrder(nextOrder);

      const nextHidden = new Set<ColumnId>();
      for (const id of prefs.hidden ?? []) if (colIdSet.has(id)) nextHidden.add(id);
      // apply defaults for new columns only
      for (const c of effectiveColumns) if (c.defaultHidden && !nextHidden.has(c.id) && !(prefs.hidden ?? []).includes(c.id)) nextHidden.add(c.id);
      // Ensure row number column is always visible
      nextHidden.delete("__rownum__");
      setHidden(nextHidden);

      setWidths(() => {
        const w: Record<string, number> = {};
        for (const c of effectiveColumns) {
          const persisted = prefs.widths?.[c.id];
          w[c.id] = typeof persisted === "number" ? persisted : (c.width ?? 160);
        }
        return w;
      });
      setPrefsLoaded(true);
    } catch {
      // Fallback to defaults if anything goes wrong
      const colIds = effectiveColumns.map((c) => c.id);
      setOrder(colIds);
      setHidden(new Set(effectiveColumns.filter((c) => c.defaultHidden).map((c) => c.id)));
      setWidths(() => {
        const w: Record<string, number> = {};
        for (const c of effectiveColumns) w[c.id] = c.width ?? 160;
        return w;
      });
      setPrefsLoaded(true);
    }
  }, [columnsMetaKey, effectiveColumns, listId, showRowNumbers]);

  // Persist on changes
  React.useEffect(() => {
    if (!prefsLoaded) return;
    // Avoid hammering localStorage during resize drag (can freeze the UI on some machines/browsers)
    if (isResizing) return;
    const prefs: PersistedPrefs = {
      v: 1,
      order,
      hidden: Array.from(hidden),
      widths,
    };
    const t = window.setTimeout(() => {
      safeWritePrefs(listId, prefs);
    }, 150);
    return () => window.clearTimeout(t);
  }, [hidden, isResizing, listId, order, prefsLoaded, widths]);

  const visibleOrderedColumns = React.useMemo(() => {
    const result: DataTableColumn<Row>[] = [];
    for (const id of order) {
      const col = baseColumnsById.get(id);
      if (!col) continue;
      if (hidden.has(id)) continue;
      result.push(col);
    }
    return result;
  }, [baseColumnsById, hidden, order]);

  const gapPx = 8; // Tailwind gap-2
  const contentWidth = React.useMemo(() => {
    const totalCols = visibleOrderedColumns.length;
    const widthsSum = visibleOrderedColumns.reduce((acc, c) => acc + (widths[c.id] ?? 160), 0);
    const gaps = totalCols > 0 ? (totalCols - 1) * gapPx : 0;
    // + padding left/right (px-3 => 12px * 2)
    return widthsSum + gaps + 24;
  }, [visibleOrderedColumns, widths]);

  const gridTemplateColumns = React.useMemo(() => {
    return visibleOrderedColumns.map((c) => `${widths[c.id] ?? 160}px`).join(" ");
  }, [visibleOrderedColumns, widths]);

  const toggleColumn = React.useCallback((id: ColumnId) => {
    setHidden((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const resetColumns = React.useCallback(() => {
    const defaultHidden = new Set(effectiveColumns.filter((c) => c.defaultHidden).map((c) => c.id));
    defaultHidden.delete("__rownum__");
    const nextOrder = effectiveColumns.map((c) => c.id);
    if (showRowNumbers) {
      const idx = nextOrder.indexOf("__rownum__");
      if (idx >= 0) nextOrder.splice(idx, 1);
      nextOrder.unshift("__rownum__");
    }
    setOrder(nextOrder);
    setHidden(defaultHidden);
    setWidths(() => {
      const w: Record<string, number> = {};
      for (const c of effectiveColumns) w[c.id] = c.width ?? 160;
      return w;
    });
  }, [effectiveColumns, showRowNumbers]);

  // Column resize
  const resizeState = React.useRef<{
    colId: ColumnId;
    startX: number;
    startWidth: number;
  } | null>(null);

  const onResizePointerDown = (e: React.PointerEvent, col: DataTableColumn<Row>) => {
    if (col.disableResize) return;
    const currentWidth = widths[col.id] ?? 160;
    resizeState.current = { colId: col.id, startX: e.clientX, startWidth: currentWidth };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    setIsResizing(true);
    e.preventDefault();
    e.stopPropagation();
  };

  const onResizePointerMove = (e: React.PointerEvent) => {
    if (!resizeState.current) return;
    const { colId, startX, startWidth } = resizeState.current;
    const col = baseColumnsById.get(colId);
    const minW = col?.minWidth ?? 80;
    const maxW = col?.maxWidth ?? 900;
    const next = clamp(startWidth + (e.clientX - startX), minW, maxW);
    setWidths((prev) => ({ ...prev, [colId]: Math.round(next) }));
  };

  const onResizePointerUp = () => {
    resizeState.current = null;
    setIsResizing(false);
  };

  // Drag & drop reorder (HTML5)
  const dragIdRef = React.useRef<ColumnId | null>(null);
  const [draggingId, setDraggingId] = React.useState<ColumnId | null>(null);
  const [dropHint, setDropHint] = React.useState<{ targetId: ColumnId; position: "before" | "after" } | null>(
    null
  );
  const onDragStart = (e: React.DragEvent, col: DataTableColumn<Row>) => {
    if (col.disableReorder) return;
    dragIdRef.current = col.id;
    setDraggingId(col.id);
    e.dataTransfer.effectAllowed = "move";
    try {
      e.dataTransfer.setData("text/plain", col.id);
    } catch {
      // ignore
    }
  };

  const onDragOver = (e: React.DragEvent, col: DataTableColumn<Row>) => {
    if (col.disableReorder) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    const fromId = draggingId;
    if (!fromId || fromId === col.id) {
      setDropHint(null);
      return;
    }
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const position = e.clientX < rect.left + rect.width / 2 ? "before" : "after";
    setDropHint({ targetId: col.id, position });
  };

  const onDrop = (e: React.DragEvent, col: DataTableColumn<Row>) => {
    e.preventDefault();
    const fromId = dragIdRef.current ?? (() => {
      try {
        return e.dataTransfer.getData("text/plain") as ColumnId;
      } catch {
        return null;
      }
    })();
    dragIdRef.current = null;
    setDraggingId(null);
    const hint = dropHint;
    setDropHint(null);
    if (!fromId || fromId === col.id) return;

    setOrder((prev) => {
      const next = prev.slice();
      const fromIdx = next.indexOf(fromId);
      const toIdx = next.indexOf(col.id);
      if (fromIdx < 0 || toIdx < 0) return prev;
      next.splice(fromIdx, 1);
      const insertAt = hint?.targetId === col.id && hint.position === "after" ? toIdx + 1 : toIdx;
      // If removing shifts the insertion index, adjust
      const adjustedInsertAt = fromIdx < insertAt ? insertAt - 1 : insertAt;
      next.splice(adjustedInsertAt, 0, fromId);
      return next;
    });
  };

  const headerNode = (
    <div
      className="border-b bg-background"
      style={{ paddingLeft: 12, paddingRight: 12, paddingTop: 8, paddingBottom: 8 }}
      onPointerMove={onResizePointerMove}
      onPointerUp={onResizePointerUp}
      onPointerCancel={onResizePointerUp}
    >
      <div
        className="grid gap-2 text-xs font-semibold text-muted-foreground select-none"
        style={{ gridTemplateColumns }}
      >
        {visibleOrderedColumns.map((col) => {
          const align =
            col.align === "right" ? "text-right" : col.align === "center" ? "text-center" : "text-left";
          const headerContent = typeof col.header === "function" ? col.header(col) : (col.header ?? col.label);
          const isDropTarget = dropHint?.targetId === col.id && draggingId && draggingId !== col.id;
          const dropPos = isDropTarget ? dropHint?.position : null;

          return (
            <div
              key={col.id}
              className={cn(
                "group relative min-w-0 flex items-center gap-2 rounded-sm px-1 -mx-1",
                align,
                col.headerClassName,
                isDropTarget ? "bg-sky-50" : ""
              )}
              draggable={!col.disableReorder}
              onDragStart={(e) => onDragStart(e, col)}
              onDragOver={(e) => onDragOver(e, col)}
              onDrop={(e) => onDrop(e, col)}
              onDragEnd={() => {
                dragIdRef.current = null;
                setDraggingId(null);
                setDropHint(null);
              }}
              onDragLeave={() => {
                // If leaving the current target, clear hint
                if (dropHint?.targetId === col.id) setDropHint(null);
              }}
              title={col.disableReorder ? undefined : "Glisser pour réordonner"}
            >
              {!col.disableReorder ? <GripVertical className="h-3.5 w-3.5 text-muted-foreground/70" /> : null}
              <span className="truncate">{headerContent}</span>

              {/* Drop indicator (animated) */}
              {isDropTarget && dropPos ? (
                <div
                  className={cn(
                    "pointer-events-none absolute top-[-6px] bottom-[-6px] w-[2px] bg-sky-500 animate-pulse rounded-full",
                    dropPos === "before" ? "left-0" : "right-0"
                  )}
                />
              ) : null}

              {!col.disableResize ? (
                <div
                  className="absolute right-0 top-0 h-full w-4 cursor-col-resize flex items-center justify-center"
                  onPointerDown={(e) => onResizePointerDown(e, col)}
                  title="Redimensionner"
                >
                  <div className="h-6 w-[3px] rounded-full bg-gray-200 opacity-90 group-hover:bg-gray-400 group-hover:opacity-100 transition-colors" />
                </div>
              ) : null}
            </div>
          );
        })}
      </div>
    </div>
  );

  const effectiveLoadRange = React.useCallback(
    async (offset: number, limit: number): Promise<LoadRangeResult<Row>> => {
      if (loadRange) return await loadRange(offset, limit);
      const all = rows ?? [];
      const items = all.slice(offset, offset + limit);
      return {
        offset,
        items,
        totalCount: all.length,
        filteredCount: all.length,
      };
    },
    [loadRange, rows]
  );

  const virtualKey = React.useMemo(() => {
    // Important: keep queryKey small even if dataKey is large (avoids huge React keys/freezes).
    if (dataKey) return `${listId}:${hash32(dataKey)}`;
    if (rows) return `${listId}:rows:${rowsRefId(rows)}`;
    return `${listId}:0`;
  }, [dataKey, listId, rows]);

  const handleCountsChange = React.useCallback(
    ({ total, filtered }: { total: number; filtered: number }) => {
      setTableCounts((prev) => {
        if (prev.total === total && prev.filtered === filtered) return prev;
        return { total, filtered };
      });
      onCountsChange?.({ total, filtered });
    },
    [onCountsChange]
  );

  return (
    <div className={cn("w-full", className)}>
      <div className="flex items-center justify-between gap-3 mb-2">
        <div className="min-w-0 flex items-center gap-2">
          {title ? <div className="text-lg font-semibold truncate">{title}</div> : null}
          <span className="text-sm text-muted-foreground font-mono truncate">
            {leftMeta ??
              `- ${
                tableCounts.filtered !== tableCounts.total
                  ? `Filtré ${tableCounts.filtered} - Total ${tableCounts.total}`
                  : `Total ${tableCounts.total}`
              }`}
          </span>
        </div>
        <div className="flex items-center gap-2">
          {rightHeader}
          <Button variant="outline" size="sm" onClick={() => setSettingsOpen(true)} title="Colonnes">
            <Settings2 className="h-4 w-4 mr-2" />
            Colonnes
          </Button>
        </div>
      </div>

      <div className="border rounded-md overflow-hidden bg-background">
        <VirtualizedRemoteList<Row>
          queryKey={virtualKey}
          height={height}
          header={headerNode}
          headerHeight={48}
          contentWidth={contentWidth}
          rowHeight={rowHeight}
          pageSize={pageSize}
          overscan={overscan}
          loadRange={effectiveLoadRange}
          showHorizontalScrollbar
          onCountsChange={handleCountsChange}
          renderRow={({ index, item, style, isLoaded }) => {
            const row = item;
            const key = row ? (getRowKey?.(row, index) ?? String(index)) : String(index);

            return (
              <div
                key={key}
                style={style}
                className={cn(
                  "grid gap-2 px-3 items-center border-b text-sm",
                  index % 2 === 0 ? "bg-background" : "bg-muted/10",
                  row && onRowClick ? "cursor-pointer hover:bg-muted/30" : ""
                )}
                onClick={() => row && onRowClick?.(row)}
              >
                <div style={{ display: "contents" }}>
                  <div style={{ display: "contents" }}>
                    <div className="grid gap-2 min-w-0" style={{ gridTemplateColumns }}>
                      {visibleOrderedColumns.map((col) => {
                        const align =
                          col.align === "right"
                            ? "text-right"
                            : col.align === "center"
                              ? "text-center"
                              : "text-left";
                        return (
                          <div
                            key={`${col.id}:${key}`}
                            className={cn("min-w-0", align, col.cellClassName)}
                          >
                            {isLoaded && row ? col.cell({ row, rowIndex: index }) : "…"}
                          </div>
                        );
                      })}
                    </div>
                  </div>
                </div>
              </div>
            );
          }}
        />
      </div>

      <Dialog open={settingsOpen} onOpenChange={setSettingsOpen}>
        <DialogContent className="sm:max-w-[520px]">
          <DialogHeader>
            <DialogTitle>Colonnes</DialogTitle>
          </DialogHeader>

          <div className="text-sm text-muted-foreground mb-3">
            Masquer / afficher, réordonner et redimensionner les colonnes (persisté sur ce navigateur).
          </div>

          <div className="space-y-2 max-h-[50vh] overflow-y-auto pr-2">
            {columns.map((col) => {
              const checked = !hidden.has(col.id);
              const disabled = col.disableHiding;
              return (
                <label key={col.id} className="flex items-center justify-between gap-3 py-1">
                  <div className="min-w-0 flex items-center gap-2">
                    <span className="font-mono text-xs text-muted-foreground">{col.id}</span>
                    <span className="truncate">{col.label}</span>
                  </div>
                  <input
                    type="checkbox"
                    checked={checked}
                    disabled={disabled}
                    onChange={() => toggleColumn(col.id)}
                    className="h-4 w-4 accent-sky-600"
                  />
                </label>
              );
            })}
          </div>

          <DialogFooter className="sm:justify-between">
            <Button variant="outline" onClick={resetColumns}>
              Réinitialiser
            </Button>
            <Button onClick={() => setSettingsOpen(false)}>OK</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

