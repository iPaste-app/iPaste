import { computed, ref, shallowRef, watch } from "vue";
import { matchesClipQuery } from "../lib/clipSearch";
import { ipasteApi } from "../lib/ipasteApi";
import { useIpasteStore } from "../stores/ipasteStore";
import type { ClipViewItem } from "../types";

const RECENT_LIMIT = 8;
const SEARCH_PAGE_SIZE = 20;
const SEARCH_DELAY_MS = 120;

export function useClipSearch() {
  const store = useIpasteStore();
  const query = ref("");
  const searchTerm = computed(() => query.value.trim());
  const historyQuery = computed(() => store.selectedCategoryId === "history" ? searchTerm.value : "");
  const historyResults = shallowRef<ClipViewItem[]>([]);
  const hasMore = ref(false);
  const isSearching = ref(false);
  const searchFailed = ref(false);
  let session: { query: string; offset: number } | null = null;

  const results = computed(() => {
    if (!searchTerm.value) return store.visibleItems.slice(0, RECENT_LIMIT);
    if (historyQuery.value) return historyResults.value;
    const term = searchTerm.value.toLowerCase();
    return store.visibleItems.filter((item) => matchesClipQuery(item, term));
  });

  watch(historyQuery, (term, _, onCleanup) => {
    historyResults.value = [];
    hasMore.value = false;
    searchFailed.value = false;
    isSearching.value = !!term;
    if (!term) return;

    const current = { query: term, offset: 0 };
    session = current;
    const timer = window.setTimeout(() => void fetchPage(current), SEARCH_DELAY_MS);
    onCleanup(() => {
      window.clearTimeout(timer);
      session = null;
    });
  }, { flush: "sync" });

  async function fetchPage(current: NonNullable<typeof session>) {
    isSearching.value = true;
    try {
      const page = await ipasteApi.listClips(current.offset, SEARCH_PAGE_SIZE, current.query);
      if (session !== current) return;

      const seen = new Set(historyResults.value.map((item) => item.id));
      const matches = page.clips.filter((item) => !seen.has(item.id))
        .map((item) => ({ ...item, collection: "history" as const }));
      historyResults.value = [...historyResults.value, ...matches];
      current.offset += page.clips.length;
      hasMore.value = page.hasMore && page.clips.length > 0;
    } catch {
      if (session === current) searchFailed.value = true;
    } finally {
      if (session === current) isSearching.value = false;
    }
  }

  function loadMore() {
    if (session && hasMore.value && !isSearching.value && !searchFailed.value) void fetchPage(session);
  }

  return { query, results, isSearching, searchFailed, loadMore };
}
