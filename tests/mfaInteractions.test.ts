import assert from "node:assert/strict";
import { test } from "node:test";
import { createRenderer, h, markRaw, nextTick, reactive } from "vue";
import MfaManagerApp from "../src/components/MfaManagerApp.vue";
import MfaAccountRow from "../src/components/MfaAccountRow.vue";
import { generateTotp } from "../src/lib/mfa";

const state = globalThis.__mfaTest;
const fixtureAccounts = [
  { id: "a", name: "a-very-long-account-name@example.com", issuer: "GitHub", description: "Work account", secret: "JBSWY3DPEHPK3PXP", digits: 8, period: 30, algorithm: "SHA1" },
  { id: "b", name: "bob@example.com", issuer: "Example", description: "Personal", secret: "JBSWY3DPEHPK3PXP", digits: 6, period: 60, algorithm: "SHA1" },
];
const flush = () => new Promise(resolve => setImmediate(resolve));
class HostNode {
  props = {};
  children: HostNode[] = [];
  parent: HostNode | null = null;
  constructor(public tag = "", public text = "") { markRaw(this); }
  get tagName() { return this.tag.toUpperCase(); }
  get options() { return this.children.filter(child => child.tag === "option"); }
  getBoundingClientRect() { return { width: state.listWidth ?? 300 }; }
  addEventListener() {}
  removeEventListener() {}
  setAttribute(key, value) { this.props[key] = value; }
  removeAttribute(key) { delete this.props[key]; }
  matches(selector) {
    return selector.split(",").some(part => {
      const value = part.trim();
      if (value.startsWith(".")) return value.slice(1).split(".").every(name => String(this.props.class || "").split(" ").includes(name));
      return value === this.tag;
    });
  }
  closest(selector) { return this.matches(selector) ? this : this.parent?.closest(selector) ?? null; }
  querySelector(selector) { return this.children.flatMap(child => all(child, el => el.matches(selector)))[0] ?? null; }
  focus() { document.activeElement = this; }
  scrollIntoView() {}
}
globalThis.HTMLElement = HostNode;
const renderer = createRenderer({
  createElement: tag => new HostNode(tag), createText: text => new HostNode("#text", text), createComment: () => new HostNode("#comment"),
  setText: (el, text) => { el.text = text; }, setElementText: (el, text) => { el.text = text; el.children = []; },
  patchProp: (el, key, _, value) => { el.props[key] = value; },
  insert(el, parent, anchor) {
    if (el.parent) el.parent.children.splice(el.parent.children.indexOf(el), 1);
    el.parent = parent;
    const index = anchor ? parent.children.indexOf(anchor) : -1;
    if (index < 0) parent.children.push(el); else parent.children.splice(index, 0, el);
  },
  remove(el) { if (el.parent) el.parent.children.splice(el.parent.children.indexOf(el), 1); },
  parentNode: el => el.parent, nextSibling: el => el.parent?.children[el.parent.children.indexOf(el) + 1] ?? null,
});
function all(root, predicate) { return [...(predicate(root) ? [root] : []), ...root.children.flatMap(child => all(child, predicate))]; }
function text(root) { return root.text + root.children.map(text).join(""); }
function event(target, extras = {}) { return { target, currentTarget: target, preventDefault() { this.defaultPrevented = true; }, stopPropagation() {}, ...extras }; }
function key(target, value, extras = {}) { state.listeners.get("keydown")(event(target, { key: value, ...extras })); }

async function fixture(accounts = fixtureAccounts) {
  const writes = [];
  state.api = {
    listMfaAccounts: async () => accounts,
    copyTextEphemeral: async code => { writes.push(["copy", code]); },
    applyTextEphemeral: async code => { writes.push(["paste", code]); },
    touchMfaAccount: async id => accounts.find(account => account.id === id),
  };
  const props = reactive({ search: "" });
  const root = new HostNode("root");
  const navigation = { backCount: 0 };
  let manager;
  const app = renderer.createApp({ render: () => h(MfaManagerApp, {
    ...props,
    ref: value => { manager = value; },
    onBack: () => { navigation.backCount++; },
  }) });
  app.mount(root);
  for (let i = 0; i < 20 && all(root, el => el.tag === "article").length < 2; i++) await flush();
  await nextTick();
  return { root, props, writes, app, manager, navigation, rows: () => all(root, el => el.tag === "article") };
}

test("native panel keys select and paste MFA accounts without browser focus", async () => {
  const f = await fixture();
  try {
    assert.equal(f.manager.handleNativePanelKey("ArrowDown"), true);
    await nextTick();
    assert.equal(f.rows()[1].props.tabindex, 0);
    assert.equal(f.manager.handleNativePanelKey("Enter"), true);
    for (let i = 0; i < 20 && !f.writes.length; i++) await flush();
    assert.deepEqual(f.writes[0], ["paste", await generateTotp(fixtureAccounts[1])]);
    assert.equal(f.manager.handleNativePanelKey("Escape"), false);
    const add = all(f.root, el => el.tag === "button" && text(el).includes("apps.mfa.add"))[0];
    add.props.onClick();
    await nextTick();
    f.manager.handleNativePanelKey("Enter");
    await flush();
    assert.equal(f.writes.length, 1);
    assert.equal(f.manager.handleNativePanelKey("Escape"), true);
    await nextTick();
    assert.equal(f.rows().length, 2);
    const count = all(f.root, el => el.matches(".mfa-account-count"))[0];
    assert.ok(count.parent.matches(".mfa-list-footer"));
    assert.equal(all(f.root, el => el.matches(".mfa-toolbar-icon")).length, 0);
    all(f.root, el => el.tag === "button" && text(el).includes("apps.mfa.add"))[0].props.onClick();
    await nextTick();
    const editor = all(f.root, el => el.matches(".mfa-editor"))[0];
    assert.equal(all(editor, el => el.tag === "button" && (text(el) === "common.cancel" || el.props["aria-label"] === "common.close")).length, 0);
    const save = all(editor, el => el.tag === "button" && el.props.type === "submit")[0];
    assert.equal(save.props.form, all(editor, el => el.tag === "form")[0].props.id);
    assert.equal(save.closest(".mfa-editor-body"), null);
    assert.ok(save.parent.parent.matches(".mfa-editor"));
    const back = all(f.root, el => el.matches(".app-center-back-button"))[0];
    assert.equal(back.props["aria-label"], "apps.mfa.backToList");
    back.props.onClick(event(back));
    await nextTick();
    assert.equal(f.rows().length, 2);
    assert.equal(f.navigation.backCount, 0);
    back.props.onClick(event(back));
    assert.equal(f.navigation.backCount, 1);
  } finally { f.app.unmount(); }
});

test("MFA keyboard navigation uses filtered accounts and ignores IME, forms and native button Enter", async () => {
  const f = await fixture();
  try {
    assert.equal(f.rows().length, 2);
    key(f.rows()[0], "ArrowDown");
    await nextTick();
    assert.equal(f.rows()[1].props.tabindex, 0);
    key(f.rows()[1], "Enter", { isComposing: true });
    key(f.rows()[1], "Enter", { repeat: true });
    key(all(f.root, el => el.tag === "button")[0], "Enter");
    await flush();
    assert.equal(f.writes.length, 0);
    key(f.rows()[1], "Enter");
    for (let i = 0; i < 20 && !f.writes.length; i++) await flush();
    assert.deepEqual(f.writes[0], ["paste", await generateTotp(fixtureAccounts[1])]);
    f.props.search = "github work";
    await nextTick();
    assert.equal(f.rows().length, 1);
    assert.equal(f.rows()[0].props.tabindex, 0);
    key(f.rows()[0], "c", { ctrlKey: true });
    for (let i = 0; i < 20 && f.writes.length < 2; i++) await flush();
    assert.deepEqual(f.writes[1], ["copy", await generateTotp(fixtureAccounts[0])]);
    assert.match(f.writes[1][1], /^\d{8}$/);
    const add = all(f.root, el => el.tag === "button" && text(el).includes("apps.mfa.add"))[0];
    add.props.onClick();
    await nextTick();
    key(all(f.root, el => el.tag === "input")[0], "Enter");
    await flush();
    assert.equal(f.writes.length, 2);
  } finally { f.app.unmount(); }
});

test("failed paste is reported and the action unlocks for a retry", async () => {
  const f = await fixture();
  try {
    state.api.applyTextEphemeral = async () => { throw new Error("Paste unavailable"); };
    key(f.rows()[0], "Enter");
    for (let i = 0; i < 20 && !text(f.root).includes("Paste unavailable"); i++) await flush();
    assert.match(text(f.root), /Paste unavailable/);
    state.api.applyTextEphemeral = async code => { f.writes.push(code); };
    key(f.rows()[0], "Enter");
    for (let i = 0; i < 20 && !f.writes.length; i++) await flush();
    assert.equal(f.writes.length, 1);
  } finally { f.app.unmount(); }
});

test("cards have no buttons, preserve full content and retain double-click paste and context menus", async () => {
  let pasted = 0, contextOpened = 0;
  const props = reactive({ account: fixtureAccounts[0], code: "01234567", nowMs: 26000, selected: true, busy: false, invalid: false, menuOpen: false });
  const root = new HostNode("root");
  const app = renderer.createApp({ render: () => h(MfaAccountRow, { ...props, onPaste: () => pasted++, onContextMenu: () => contextOpened++ }) });
  app.mount(root);
  try {
    assert.match(text(root), /a-very-long-account-name@example.com/);
    const groups = all(root, el => el.matches(".mfa-code-value"))[0];
    assert.equal(text(groups), "01234567");
    assert.equal(groups.children.filter(el => el.tag === "span").length, 0);
    assert.equal(text(all(root, el => el.matches(".mfa-code-countdown"))[0]), "4s");
    assert.ok(all(root, el => el.matches(".mfa-code-countdown"))[0].matches(".is-danger"));
    props.nowMs = 30000;
    await nextTick();
    assert.equal(text(all(root, el => el.matches(".mfa-code-countdown"))[0]), "30s");
    assert.ok(all(root, el => el.matches(".mfa-code-countdown"))[0].matches(".is-safe"));
    assert.equal(all(root, el => el.matches(".mfa-countdown-progress"))[0].props["stroke-dashoffset"], 0);
    props.nowMs = 45000;
    await nextTick();
    assert.ok(all(root, el => el.matches(".mfa-code-countdown"))[0].matches(".is-warning"));
    assert.equal(all(root, el => el.matches(".mfa-countdown-progress"))[0].props["stroke-dashoffset"], 50);
    assert.equal(all(root, el => el.tag === "button").length, 0);
    const card = all(root, el => el.tag === "article")[0];
    card.props.onClick(event(card));
    assert.equal(pasted, 0);
    card.props.onDblclick(event(card));
    assert.equal(pasted, 1);
    card.props.onContextmenu(event(card));
    assert.equal(contextOpened, 1);
    props.invalid = true;
    await nextTick();
    card.props.onDblclick(event(card));
    assert.equal(pasted, 1);
    assert.equal(all(root, el => el.tag === "svg").length, 0);
  } finally { app.unmount(); }
});

test("two-column selection follows the grid for DOM and native keys", async () => {
  state.listWidth = 600;
  const f = await fixture([...fixtureAccounts, ...fixtureAccounts.map(account => ({ ...account, id: account.id + "2" }))]);
  try {
    key(f.rows()[0], "ArrowDown");
    await nextTick();
    assert.equal(f.rows()[2].props.tabindex, 0);
    f.manager.handleNativePanelKey("ArrowRight");
    await nextTick();
    assert.equal(f.rows()[3].props.tabindex, 0);
    key(f.rows()[3], "ArrowUp");
    await nextTick();
    assert.equal(f.rows()[1].props.tabindex, 0);
    f.manager.handleNativePanelKey("ArrowLeft");
    await nextTick();
    assert.equal(f.rows()[0].props.tabindex, 0);
  } finally { f.app.unmount(); state.listWidth = 300; }
});
