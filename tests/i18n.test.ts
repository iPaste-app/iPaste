import assert from "node:assert/strict";
import { test } from "node:test";
import { createRenderer, h, markRaw, nextTick, ref } from "vue";
import LanguageSelect from "../src/components/LanguageSelect.vue";
import { cleanLanguage, currentLanguage, currentLocale, languageOptions, setLanguage, t } from "../src/i18n";
import { catalogs, getCatalog, type Language } from "../src/i18n/catalogs";

const storageKey = "ipaste.language";
const englishKeys = Object.keys(catalogs.en.messages).sort();
const placeholders = (message: string) => (message.match(/\{\w+\}/g) ?? []).sort();
const addedLabels = {
  "zh-TW": "繁體中文", "pt-BR": "Português (Brasil)", it: "Italiano", ru: "Русский", pl: "Polski",
  tr: "Türkçe", nl: "Nederlands", id: "Bahasa Indonesia", vi: "Tiếng Việt", th: "ไทย",
};
const addedLanguages = Object.keys(addedLabels) as (keyof typeof addedLabels)[];

test("initialization restores the saved locale before the interface is first used", () => {
  assert.equal(currentLanguage.value, "zh-TW");
  assert.equal(document.documentElement.lang, "zh-TW");
  assert.equal(t("topBar.searchPlaceholder"), "搜尋剪貼簿");
  setLanguage("pt-BR", { persist: false });
  assert.equal(currentLanguage.value, "pt-BR");
  assert.equal(localStorage.getItem(storageKey), "zh-TW");
});

test("all 17 languages can be selected, persisted, and used for calendar formatting", () => {
  assert.equal(new Set(languageOptions.map(({ value }) => value)).size, 17);
  for (const [value, label] of Object.entries(addedLabels)) {
    assert.equal(languageOptions.find(option => option.value === value)?.label, label);
  }
  for (const { value } of languageOptions) {
    setLanguage(value);
    const messages = getCatalog(value).messages;
    assert.equal(cleanLanguage(value), value);
    assert.equal(localStorage.getItem(storageKey), value);
    assert.equal(document.documentElement.lang, value);
    assert.ok(Intl.DateTimeFormat.supportedLocalesOf([currentLocale.value]).length);
    assert.equal(t("settings.language.title"), messages["settings.language.title"]);
  }
  for (const invalid of ["unsupported", "constructor", "__proto__", "", null]) {
    assert.equal(cleanLanguage(invalid), "en");
  }
});

test("each new catalog covers the English keys and preserves runtime interpolation", () => {
  for (const language of addedLanguages) {
    const messages = getCatalog(language).messages;
    assert.deepEqual(Object.keys(messages).sort(), englishKeys, language);
    setLanguage(language);
    for (const [key, english] of Object.entries(catalogs.en.messages) as [keyof typeof messages, string][]) {
      const message = messages[key];
      const tokens = placeholders(english);
      assert.ok(message.trim(), `${language}/${key} is empty`);
      assert.deepEqual(placeholders(message), tokens, `${language}/${key}`);
      const params = Object.fromEntries(tokens.map(token => [token.slice(1, -1), "42"]));
      assert.ok(!/\{\w+\}/.test(t(key, params)), `${language}/${key} did not substitute parameters`);
    }
    assert.notEqual(t("settings.language.title"), catalogs.en.messages["settings.language.title"]);
  }
  setLanguage("pt-BR");
  const dynamicValue = "$& $$ $1 {next}";
  assert.equal(t("update.versionLine", { current: dynamicValue, next: "2.0" }), `Versão atual: ${dynamicValue} · Nova versão: 2.0`);
  assert.match(t("update.versionLine", { current: 0 }), /0.*\{next\}/);
});

class HostNode {
  parent: HostNode | null = null;
  children: HostNode[] = [];
  props: Record<string, any> = {};

  constructor(public tag: string) {}

  focus() {
    focused = this;
    this.props.onFocus?.();
  }
}
let focused: HostNode | undefined;

function detach(node: HostNode) {
  const parent = node.parent;
  if (parent) parent.children.splice(parent.children.indexOf(node), 1);
  node.parent = null;
}

function descendants(node: HostNode): HostNode[] {
  return [node, ...node.children.flatMap(descendants)];
}

test("keyboard navigation can reach and select the end of the scrollable language list", async () => {
  globalThis.HTMLButtonElement = HostNode as unknown as typeof HTMLButtonElement;
  const renderer = createRenderer<HostNode, HostNode>({
    createElement: tag => markRaw(new HostNode(tag)),
    createComment: () => new HostNode("#comment"),
    createText: () => new HostNode("#text"),
    insert(node, parent, anchor) {
      detach(node);
      node.parent = parent;
      parent.children.splice(anchor ? parent.children.indexOf(anchor) : parent.children.length, 0, node);
    },
    remove: detach,
    nextSibling: node => node.parent?.children[node.parent.children.indexOf(node) + 1] ?? null,
    parentNode: node => node.parent,
    patchProp(node, key, _oldValue, value) { node.props[key] = value; },
    setText() {},
    setElementText() {},
  });
  const root = new HostNode("root");
  const selected = ref<Language>("pt-BR");
  const app = renderer.createApp({
    render: () => h(LanguageSelect, {
      modelValue: selected.value,
      options: languageOptions,
      "onUpdate:modelValue": (value: Language) => { selected.value = value; },
    }),
  });
  app.mount(root);
  const trigger = descendants(root).find(node => node.props["aria-haspopup"] === "listbox")!;
  const options = () => descendants(root).filter(node => node.props.role === "option");
  function keydown(node: HostNode | undefined, key: string) {
    assert.ok(node);
    node.props.onKeydown({ key, preventDefault() {}, stopPropagation() {} });
    return nextTick();
  }

  try {
    await keydown(trigger, "ArrowDown");
    assert.equal(options().length, 17);
    assert.equal(focused?.props["aria-selected"], true);
    assert.equal(focused?.children[0].props.lang, "pt-BR");

    await keydown(focused, "End");
    assert.equal(focused, options().at(-1));
    assert.equal(focused?.children[0].props.lang, "th");
    assert.equal(options().filter(node => node.props.tabindex === 0).length, 1);
    await keydown(focused, "ArrowDown");
    assert.equal(focused, options()[0]);
    await keydown(focused, "End");

    focused?.props.onClick();
    await nextTick();
    assert.equal(selected.value, "th");
    assert.equal(focused, trigger);
    assert.equal(options().length, 0);

    await keydown(trigger, "Enter");
    assert.equal(focused, options().at(-1));
    await keydown(focused, "Home");
    assert.equal(focused, options()[0]);
    await keydown(focused, "Escape");
    assert.equal(selected.value, "th");
    assert.equal(focused, trigger);
    assert.equal(options().length, 0);
  } finally {
    app.unmount();
  }
});
