import type { Category, CategoryItem, ClipType } from "../types";

export type CloudSnapshot = {
  categories: Category[];
  categoryItems: CategoryItem[];
};

export type CloudCredentials = {
  apiAddress: string;
  apiKey: string;
};

export type CloudCategoryInput = {
  id?: string;
  name: string;
  color: string;
  sortOrder?: number;
  createdAt?: string;
  updatedAt?: string;
};

export type CloudClipType = Exclude<ClipType, "image" | "file">;

export type CloudCategoryItemInput = {
  id?: string;
  categoryId: string;
  clipSnapshotId?: string;
  clipType: CloudClipType;
  contentHash?: string;
  displayName?: string | null;
  previewText?: string;
  text: string;
  sortOrder?: number;
  createdAt?: string;
  updatedAt?: string;
};

type ApiEnvelope<T> = T & {
  ok?: boolean;
  error?: string;
};

export const CLOUD_AUTH_STORAGE_KEY = "ipaste-cloud-auth";

export function normalizeApiAddress(value: string) {
  return value.trim().replace(/\/+$/, "");
}

export function getDefaultApiAddress() {
  if (typeof window === "undefined") return "";
  return window.location.origin;
}

export async function testCloudCredentials(credentials: CloudCredentials) {
  const response = await cloudRequest<{ service: string }>(credentials, "/api/health");
  return response.service === "ipaste-cloud";
}

export async function fetchCloudSnapshot(credentials: CloudCredentials) {
  return cloudRequest<CloudSnapshot>(credentials, "/api/snapshot");
}

export async function createCloudCategory(credentials: CloudCredentials, input: CloudCategoryInput) {
  const response = await cloudRequest<{ category: Category }>(credentials, "/api/categories", {
    method: "POST",
    body: input,
  });
  return response.category;
}

export async function reorderCloudCategories(credentials: CloudCredentials, ids: string[]) {
  const response = await cloudRequest<{ categories: Category[] }>(credentials, "/api/categories/reorder", {
    method: "PUT",
    body: { ids },
  });
  return response.categories;
}

export async function updateCloudCategory(credentials: CloudCredentials, input: Required<Pick<CloudCategoryInput, "id" | "name" | "color">>) {
  const response = await cloudRequest<{ category: Category }>(
    credentials,
    `/api/categories/${encodeURIComponent(input.id)}`,
    {
      method: "PUT",
      body: input,
    },
  );
  return response.category;
}

export async function deleteCloudCategory(credentials: CloudCredentials, id: string) {
  await cloudRequest(credentials, `/api/categories/${encodeURIComponent(id)}`, {
    method: "DELETE",
  });
}

export async function createCloudCategoryItem(credentials: CloudCredentials, input: CloudCategoryItemInput) {
  const payload = await prepareCategoryItemInput(input);
  const response = await cloudRequest<{ item: CategoryItem }>(
    credentials,
    `/api/categories/${encodeURIComponent(payload.categoryId)}/items`,
    {
      method: "POST",
      body: payload,
    },
  );
  return response.item;
}

export async function reorderCloudCategoryItems(credentials: CloudCredentials, categoryId: string, ids: string[]) {
  const response = await cloudRequest<{ items: CategoryItem[] }>(
    credentials,
    `/api/categories/${encodeURIComponent(categoryId)}/items/reorder`,
    {
      method: "PUT",
      body: { ids },
    },
  );
  return response.items;
}

export async function updateCloudCategoryItem(credentials: CloudCredentials, input: CloudCategoryItemInput & { id: string }) {
  const payload = await prepareCategoryItemInput(input);
  const response = await cloudRequest<{ item: CategoryItem }>(
    credentials,
    `/api/items/${encodeURIComponent(input.id)}`,
    {
      method: "PUT",
      body: payload,
    },
  );
  return response.item;
}

export async function deleteCloudCategoryItem(credentials: CloudCredentials, id: string) {
  await cloudRequest(credentials, `/api/items/${encodeURIComponent(id)}`, {
    method: "DELETE",
  });
}

export async function prepareCategoryItemInput(input: CloudCategoryItemInput) {
  const timestamp = new Date().toISOString();
  const text = input.text.trim();
  const contentHash = input.contentHash ?? (await sha256(text));
  return {
    id: input.id,
    categoryId: input.categoryId,
    clipSnapshotId: input.clipSnapshotId ?? input.id ?? crypto.randomUUID(),
    clipType: input.clipType,
    contentHash,
    displayName: input.displayName?.trim() || null,
    previewText: input.previewText?.trim() || previewText(text),
    text,
    ...(input.sortOrder === undefined ? {} : { sortOrder: input.sortOrder }),
    createdAt: input.createdAt ?? timestamp,
    updatedAt: input.updatedAt ?? timestamp,
  };
}

export function detectTextClipType(text: string): CloudClipType {
  const value = text.trim();
  const lower = value.toLowerCase();

  if (isColor(value)) return "color";
  if (lower.startsWith("http://") || lower.startsWith("https://")) return "link";
  if (/<[a-z][\s\S]*>/i.test(value)) return "html";
  return "text";
}

export function previewText(text: string) {
  return text.split(/\s+/).join(" ").slice(0, 180);
}

export async function cloudRequest<T = unknown>(
  credentials: CloudCredentials,
  path: string,
  options: { method?: string; body?: unknown } = {},
) {
  const apiAddress = normalizeApiAddress(credentials.apiAddress);
  const response = await fetch(`${apiAddress}${path}`, {
    method: options.method ?? "GET",
    headers: {
      Authorization: `Bearer ${credentials.apiKey}`,
      "Content-Type": "application/json",
    },
    body: options.body === undefined ? undefined : JSON.stringify(options.body),
  });
  const payload = (await response.json().catch(() => ({}))) as ApiEnvelope<T>;

  if (!response.ok) {
    throw new Error(payload.error || `请求失败：${response.status}`);
  }

  return payload as T;
}

async function sha256(text: string) {
  const bytes = new TextEncoder().encode(text);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest))
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

function isColor(text: string) {
  if (/^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/.test(text)) return true;
  return /^rgba?\([^)]+\)$/i.test(text);
}
