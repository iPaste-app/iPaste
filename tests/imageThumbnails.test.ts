import assert from "node:assert/strict";
import { test } from "node:test";
import { effectScope, nextTick, ref, type Ref } from "vue";
import { useImageThumbnail } from "../src/composables/useImageThumbnail";

type ThumbnailRequest = {
  path: string;
  resolve: (value: string) => void;
  reject: (reason?: unknown) => void;
};

const state = (globalThis as typeof globalThis & {
  __imageThumbnailTest: {
    requests: ThumbnailRequest[];
    convertedPaths: string[];
  };
}).__imageThumbnailTest;

function mountThumbnail(path: string | null) {
  const imagePath = ref(path);
  const scope = effectScope();
  const source = scope.run(() => useImageThumbnail(imagePath));
  if (!source) throw new Error("Could not create image thumbnail scope");

  return {
    imagePath,
    source: source as Ref<string>,
    stop: () => scope.stop(),
  };
}

async function flushThumbnailUpdate() {
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
  await nextTick();
}

function latestRequest(path: string) {
  const matching = state.requests.filter((entry) => entry.path === path);
  assert.ok(matching.length > 0, `expected a thumbnail request for ${path}`);
  return matching.at(-1)!;
}

test("null and data URL sources never request a thumbnail", async () => {
  state.requests.length = 0;
  const card = mountThumbnail(null);

  try {
    await flushThumbnailUpdate();
    assert.equal(state.requests.length, 0);

    card.imagePath.value = "data:image/png;base64,large-image-bytes";
    await flushThumbnailUpdate();
    assert.equal(state.requests.length, 0);
    assert.equal(card.source.value, "");
  } finally {
    card.stop();
  }
});

test("same image paths share a request and reuse a successful cached thumbnail", async () => {
  state.requests.length = 0;
  state.convertedPaths.length = 0;
  const path = "/images/shared.png";
  const firstCard = mountThumbnail(path);
  const secondCard = mountThumbnail(path);

  try {
    assert.equal(state.requests.length, 1);
    latestRequest(path).resolve("/thumbnails/shared.png");
    await flushThumbnailUpdate();

    assert.equal(firstCard.source.value, "asset:///thumbnails/shared.png");
    assert.equal(secondCard.source.value, "asset:///thumbnails/shared.png");
    assert.deepEqual(state.convertedPaths, ["/thumbnails/shared.png", "/thumbnails/shared.png"]);

    firstCard.stop();
    secondCard.stop();
    const cachedCard = mountThumbnail(path);
    try {
      await flushThumbnailUpdate();
      assert.equal(state.requests.length, 1);
      assert.equal(cachedCard.source.value, "asset:///thumbnails/shared.png");
    } finally {
      cachedCard.stop();
    }
  } finally {
    firstCard.stop();
    secondCard.stop();
  }
});

test("source changes and unmounting discard old thumbnail results", async () => {
  state.requests.length = 0;
  const oldPath = "/images/old.png";
  const nextPath = "/images/new.png";
  const card = mountThumbnail(oldPath);

  try {
    card.imagePath.value = nextPath;
    await flushThumbnailUpdate();
    assert.equal(state.requests.length, 2);

    latestRequest(oldPath).resolve("/thumbnails/old.png");
    await flushThumbnailUpdate();
    assert.equal(card.source.value, "");

    latestRequest(nextPath).resolve("/thumbnails/new.png");
    await flushThumbnailUpdate();
    assert.equal(card.source.value, "asset:///thumbnails/new.png");
  } finally {
    card.stop();
  }

  const unmountedPath = "/images/unmounted.png";
  const unmountedCard = mountThumbnail(unmountedPath);
  unmountedCard.stop();
  latestRequest(unmountedPath).resolve("/thumbnails/unmounted.png");
  await flushThumbnailUpdate();
  assert.equal(unmountedCard.source.value, "");
});

test("a failed thumbnail request is retried by a later card", async () => {
  state.requests.length = 0;
  const path = "/images/retry.png";
  const failedCard = mountThumbnail(path);

  try {
    latestRequest(path).reject(new Error("thumbnail unavailable"));
    await flushThumbnailUpdate();
    assert.equal(failedCard.source.value, "");
  } finally {
    failedCard.stop();
  }

  const retriedCard = mountThumbnail(path);
  try {
    assert.equal(state.requests.length, 2);
    latestRequest(path).resolve("/thumbnails/retry.png");
    await flushThumbnailUpdate();
    assert.equal(retriedCard.source.value, "asset:///thumbnails/retry.png");
  } finally {
    retriedCard.stop();
  }
});
