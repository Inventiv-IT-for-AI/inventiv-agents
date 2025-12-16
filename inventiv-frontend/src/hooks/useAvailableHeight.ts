"use client";

import { useEffect, useState, useRef, RefObject } from "react";

/**
 * Hook pour calculer la hauteur disponible dynamiquement pour les tableaux virtualisés.
 * Calcule la hauteur en fonction de la position réelle de l'élément sur la page pour éviter
 * que la liste dépasse du bas de la page.
 * 
 * @param offset - Hauteur à soustraire pour les éléments fixes (header, padding, etc.). Par défaut: 200px
 * @param minHeight - Hauteur minimale à retourner. Par défaut: 300px
 * @param containerRef - Référence optionnelle vers l'élément conteneur pour calculer la position réelle
 * @param minRows - Nombre minimum de lignes à afficher si la page est trop petite. Par défaut: 5
 * @param rowHeight - Hauteur d'une ligne pour calculer la hauteur minimale en lignes. Par défaut: 50px
 * @returns Un objet contenant la hauteur disponible et une fonction pour définir la ref du conteneur
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
        // Calculer la position de l'élément sur la page
        // getBoundingClientRect() retourne la position relative à la fenêtre visible
        const rect = element.getBoundingClientRect();
        const elementTop = rect.top; // Position relative au viewport
        const viewportHeight = window.innerHeight;
        
        // Hauteur disponible = distance du haut de l'élément jusqu'au bas de la fenêtre
        // Moins un petit padding pour éviter de toucher exactement le bas
        const padding = 16; // 16px de marge en bas
        const availableHeight = viewportHeight - elementTop - padding;
        
        // Si la page est trop petite, limiter à minRows lignes
        const minHeightInRows = minRows * rowHeight;
        const constrainedHeight = Math.max(availableHeight, minHeightInRows);
        
        // Ne jamais dépasser la hauteur minimale configurée
        const finalHeight = Math.max(constrainedHeight, minHeight);
        
        setHeight(finalHeight);
      } else {
        // Fallback si pas de ref : utiliser l'ancienne méthode avec offset
        const availableHeight = Math.max(window.innerHeight - offset, minHeight);
        setHeight(availableHeight);
      }
    };

    // Mise à jour initiale avec un petit délai pour s'assurer que le DOM est rendu
    const timeoutId = setTimeout(updateHeight, 0);

    // Écouter les changements de taille de fenêtre et de scroll
    window.addEventListener("resize", updateHeight);
    window.addEventListener("scroll", updateHeight, { passive: true });

    // Observer les changements de taille de l'élément parent si possible
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

/**
 * Hook pour obtenir à la fois la hauteur disponible et une ref callback pour le conteneur.
 * Version plus pratique qui retourne aussi la ref à utiliser.
 * 
 * @param offset - Hauteur à soustraire pour les éléments fixes (header, padding, etc.). Par défaut: 200px
 * @param minHeight - Hauteur minimale à retourner. Par défaut: 300px
 * @param minRows - Nombre minimum de lignes à afficher si la page est trop petite. Par défaut: 5
 * @param rowHeight - Hauteur d'une ligne pour calculer la hauteur minimale en lignes. Par défaut: 50px
 * @returns Un objet contenant la hauteur disponible et une ref callback pour le conteneur
 */
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

