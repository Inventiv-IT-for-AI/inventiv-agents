"use client";

import { useEffect, useRef, useState, type RefObject } from "react";

function getScrollParent(el: HTMLElement | null): HTMLElement | Window {
  if (!el) return window;
  let cur: HTMLElement | null = el;
  while (cur) {
    const style = window.getComputedStyle(cur);
    const overflowY = style.overflowY;
    if (overflowY === "auto" || overflowY === "scroll") return cur;
    cur = cur.parentElement;
  }
  return window;
}

/**
 * Hook pour calculer la hauteur disponible dynamiquement pour les tableaux virtualisés.
 */
export function useAvailableHeight(
  offset: number = 200,
  minHeight: number = 300,
  containerRef?: RefObject<HTMLElement | null>,
  minRows: number = 5,
  rowHeight: number = 50
): number {
  const internalRef = useRef<HTMLElement | null>(null);
  const ref = containerRef || internalRef;

  const [height, setHeight] = useState<number>(() => {
    if (typeof window === "undefined") return minHeight;
    return Math.max(window.innerHeight - offset, minHeight);
  });

  useEffect(() => {
    if (typeof window === "undefined") return;

    const updateHeight = () => {
      const element = ref?.current;
      if (element) {
        const rect = element.getBoundingClientRect();
        const elementTop = rect.top;
        const viewportHeight = window.innerHeight;

        const padding = 16;
        const availableHeight = viewportHeight - elementTop - padding;

        const minHeightInRows = minRows * rowHeight;
        const constrainedHeight = Math.max(availableHeight, minHeightInRows);
        const finalHeight = Math.max(constrainedHeight, minHeight);

        setHeight(finalHeight);
      } else {
        const availableHeight = Math.max(window.innerHeight - offset, minHeight);
        setHeight(availableHeight);
      }
    };

    const timeoutId = setTimeout(updateHeight, 0);

    window.addEventListener("resize", updateHeight);
    // In this app, the main scroll container is often NOT the window (e.g. <main className="overflow-y-auto">).
    // Listen to the nearest scroll parent too, otherwise height won't update when the page scrolls.
    const scrollParent = getScrollParent(ref?.current ?? null);
    if (scrollParent !== window) {
      (scrollParent as HTMLElement).addEventListener("scroll", updateHeight, { passive: true });
    } else {
      window.addEventListener("scroll", updateHeight, { passive: true });
    }

    let resizeObserver: ResizeObserver | null = null;
    const element = ref?.current;
    if (element && typeof ResizeObserver !== "undefined") {
      resizeObserver = new ResizeObserver(updateHeight);
      resizeObserver.observe(element);
    }

    return () => {
      clearTimeout(timeoutId);
      window.removeEventListener("resize", updateHeight);
      window.removeEventListener("scroll", updateHeight);
      if (scrollParent !== window) {
        (scrollParent as HTMLElement).removeEventListener("scroll", updateHeight);
      }
      resizeObserver?.disconnect();
    };
  }, [offset, minHeight, ref, minRows, rowHeight]);

  return height;
}

export function useAvailableHeightWithRef(
  offset: number = 200,
  minHeight: number = 300,
  minRows: number = 5,
  rowHeight: number = 50
): { height: number; containerRef: (node: HTMLElement | null) => void } {
  const ref = useRef<HTMLElement | null>(null);

  const setRef = (node: HTMLElement | null) => {
    ref.current = node;
  };

  const height = useAvailableHeight(offset, minHeight, ref, minRows, rowHeight);
  return { height, containerRef: setRef };
}


