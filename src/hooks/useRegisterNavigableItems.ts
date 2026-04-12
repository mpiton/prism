import { useEffect } from "react";
import { useDashboardStore } from "../stores/dashboard";
import type { NavigableItem, NavigableSectionId } from "../stores/dashboard";

export function useRegisterNavigableItems(
  items: readonly NavigableItem[],
  sectionId: NavigableSectionId,
): void {
  const registerNavigableItems = useDashboardStore((s) => s.registerNavigableItems);
  const unregisterNavigableSection = useDashboardStore((s) => s.unregisterNavigableSection);

  useEffect(() => {
    registerNavigableItems(sectionId, items);
  }, [items, registerNavigableItems, sectionId]);

  useEffect(() => {
    return () => unregisterNavigableSection(sectionId);
  }, [sectionId, unregisterNavigableSection]);
}
