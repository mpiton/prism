import { useEffect } from "react";
import { useDashboardStore } from "../stores/dashboard";
import type { NavigableItem } from "../stores/dashboard";

export function useRegisterNavigableItems(
  items: readonly NavigableItem[],
): void {
  const setNavigableItems = useDashboardStore((s) => s.setNavigableItems);

  useEffect(() => {
    setNavigableItems(items);
  }, [items, setNavigableItems]);

  useEffect(() => {
    return () => setNavigableItems([]);
  }, [setNavigableItems]);
}
