import { onScopeDispose, ref, toValue, watch, type MaybeRefOrGetter } from "vue";

export function useElementWidth(element: MaybeRefOrGetter<HTMLElement | null | undefined>) {
  const width = ref(0);
  let observer: ResizeObserver | undefined;

  watch(() => toValue(element), (target) => {
    observer?.disconnect();
    if (!target) {
      width.value = 0;
      return;
    }

    const measure = () => { width.value = target.getBoundingClientRect().width; };
    measure();
    observer = new ResizeObserver(measure);
    observer.observe(target, { box: "border-box" });
  }, { immediate: true, flush: "post" });

  onScopeDispose(() => observer?.disconnect());
  return width;
}
