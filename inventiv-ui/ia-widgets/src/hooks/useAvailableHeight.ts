"use client";

import { useEffect, useState, useRef, type RefObject } from "react";

export function useAvailableHeight(
  offset: number = 200,
  minHeight: number = 300,
  containerRef?: RefObject<HTMLElement | null>,
  minRows: number = 5,
  rowHeight: number = 50,
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
    window.addEventListener("scroll", updateHeight, { passive: true });

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
      resizeObserver?.disconnect();
    };
  }, [offset, minHeight, ref, minRows, rowHeight]);

  return height;
}

export function useAvailableHeightWithRef(
  offset: number = 200,
  minHeight: number = 300,
  minRows: number = 5,
  rowHeight: number = 50,
): { height: number; containerRef: (node: HTMLElement | null) => void } {
  const ref = useRef<HTMLElement | null>(null);
  const setRef = (node: HTMLElement | null) => {
    ref.current = node;
  };
  const height = useAvailableHeight(offset, minHeight, ref, minRows, rowHeight);
  return { height, containerRef: setRef };
}


