import test from "node:test";
import assert from "node:assert/strict";
import { fillLogin, readLogin, siteOrigin } from "../background.js";

test("site access requires a clean HTTPS origin", () => {
  assert.equal(siteOrigin("https://example.com/path?q=secret"), "https://example.com");
  for (const value of ["http://example.com", "https://user@example.com"]) {
    assert.throws(() => siteOrigin(value));
  }
  assert.equal(siteOrigin("https://example.com.evil.test"), "https://example.com.evil.test");
});

globalThis.HTMLInputElement = class {
  constructor({ type = "text", value = "", autocomplete = "", readonly = false, visible = true, form = null } = {}) {
    Object.assign(this, { type, _value: value, autocomplete, disabled: false, readOnly: readonly, visible, form, events: [] });
  }
  get value() { return this._value; }
  set value(value) { this._value = value; }
  getClientRects() { return this.visible ? [{}] : []; }
  dispatchEvent(event) { this.events.push(event.type); }
  focus() { this.focused = true; }
};
globalThis.getComputedStyle = () => ({ visibility: "visible" });
globalThis.HTMLFormElement = class {
  get action() { return this._action ?? Object.getOwnPropertyDescriptor(this, "action")?.value; }
  set action(value) { this._action = value; }
};

function fakeInput({ type = "text", value = "", autocomplete = "", readonly = false, visible = true, form = null } = {}) {
  return new HTMLInputElement({ type, value, autocomplete, readonly, visible, form });
}

test("save reads only visible inputs from a same-origin HTTPS form", () => {
  const password = fakeInput({ type: "password" });
  const username = fakeInput({ autocomplete: "username" });
  const hidden = fakeInput({ type: "email", visible: false });
  const form = { action: "https://example.com/session", querySelectorAll: () => [username, password, hidden] };
  password.form = form;
  username.form = form;
  globalThis.location = { origin: "https://example.com", protocol: "https:" };
  globalThis.document = { querySelectorAll: selector => selector === 'input[type="password"]' ? [password] : selector === 'input' ? [username, password, hidden] : [] };
  username.value = "alice";
  password.value = "secret";
  assert.deepEqual(readLogin("https://example.com"), { username: "alice", password: "secret" });
  form.action = "https://attacker.test/collect";
  assert.equal(readLogin("https://example.com"), null);
  delete globalThis.location;
  delete globalThis.document;
});

test("fill is same-origin, dispatches input events and never submits the form", () => {
  const password = fakeInput({ type: "password" });
  const username = fakeInput({ autocomplete: "username" });
  let submissions = 0;
  const form = { action: "https://example.com/login", querySelectorAll: () => [username, password], submit() { submissions++; } };
  password.form = form;
  username.form = form;
  globalThis.location = { origin: "https://example.com", protocol: "https:" };
  globalThis.HTMLInputElement = class {};
  Object.defineProperty(globalThis.HTMLInputElement.prototype, "value", { set(value) { this._value = value; } });
  globalThis.Event = class { constructor(type) { this.type = type; } };
  globalThis.document = { querySelectorAll: selector => selector === 'input[type="password"]' ? [password] : selector === 'input' ? [username, password] : [] };
  assert.equal(fillLogin("https://example.com", "alice", "secret"), true);
  assert.equal(username.value, "alice");
  assert.equal(password.value, "secret");
  assert.deepEqual(password.events, ["input", "change"]);
  assert.equal(submissions, 0);
  assert.equal(fillLogin("https://example.com.evil.test", "alice", "secret"), false);
  delete globalThis.location;
  delete globalThis.HTMLInputElement;
  delete globalThis.Event;
  delete globalThis.document;
});

test("save and fill reject submit-button cross-origin overrides", () => {
  globalThis.HTMLInputElement = class {
    constructor(options) { Object.assign(this, options); this._value = options.value; this.events = []; }
    get value() { return this._value; }
    set value(value) { this._value = value; }
    getClientRects() { return [{}]; }
    dispatchEvent(event) { this.events.push(event.type); }
    focus() {}
  };
  const password = fakeInput({ type: "password" });
  const username = fakeInput({ autocomplete: "username" });
  const submitter = { type: "submit", formAction: "/continue" };
  const form = {
    action: "https://example.com/login",
    elements: [username, password, submitter],
    querySelectorAll: () => [username, password],
  };
  password.form = form;
  username.form = form;
  submitter.form = form;
  password.value = "secret";
  username.value = "alice";
  globalThis.location = { origin: "https://example.com", protocol: "https:", href: "https://example.com/login" };
  globalThis.HTMLInputElement = class {};
  Object.defineProperty(globalThis.HTMLInputElement.prototype, "value", { set(value) { this._value = value; } });
  globalThis.Event = class { constructor(type) { this.type = type; } };
  globalThis.document = { querySelectorAll: selector => selector === 'input[type="password"]' ? [password] : selector === 'input' ? [username, password] : [submitter] };
  for (const type of ["submit", "image"]) {
    submitter.type = type;
    for (const action of ["https://example.com/continue", "https://example.com:443/continue"]) {
      submitter.formAction = action;
      assert.deepEqual(readLogin("https://example.com"), { username: "alice", password: "secret" });
      assert.equal(fillLogin("https://example.com", "alice", "secret"), true);
    }
    for (const action of [
      "https://attacker.test/collect",
      "//attacker.test/collect",
      "javascript:alert(1)",
      "data:text/html,exfiltrate",
    ]) {
      submitter.formAction = action;
      assert.equal(readLogin("https://example.com"), null, `${type} accepted ${action}`);
      assert.equal(fillLogin("https://example.com", "alice", "secret"), false, `${type} accepted ${action}`);
    }
  }
  delete globalThis.location;
  delete globalThis.HTMLInputElement;
  delete globalThis.Event;
  delete globalThis.document;
});

test("submission listener ignores a cross-origin submit-button override", async () => {
  const messages = [];
  let submit;
  globalThis.browser = {
    runtime: { sendMessage: async message => { messages.push(message); return { allowed: true }; } },
  };
  globalThis.location = { origin: "https://example.com", protocol: "https:", href: "https://example.com/login" };
  globalThis.HTMLFormElement = class {
    get action() { return this._action; }
    set action(value) { this._action = value; }
  };
  globalThis.getComputedStyle = () => ({ visibility: "visible" });
  globalThis.document = { addEventListener: (type, listener) => { if (type === "submit") submit = listener; } };
  globalThis.__rustyVaultListener = false;
  try {
    await import(`../content.js?submitter-${Date.now()}`);
    const password = { type: "password", autocomplete: "current-password", value: "secret", disabled: false, getClientRects: () => [{}] };
    const form = new HTMLFormElement();
    form.action = "https://example.com/login";
    form.querySelectorAll = () => [password];
    password.form = form;
    document.querySelectorAll = () => [password];
    submit({ isTrusted: true, target: form, submitter: { hasAttribute: () => true, formAction: "https://attacker.test/collect" } });
    assert.deepEqual(messages, [{ action: "siteAccess", origin: "https://example.com" }]);
    submit({ isTrusted: true, target: form, submitter: { hasAttribute: () => true, formAction: "https://example.com:443/submit" } });
    assert.deepEqual(messages, [
      { action: "siteAccess", origin: "https://example.com" },
      { action: "submitted", origin: "https://example.com", username: "", password: "secret" },
    ]);
  } finally {
    delete globalThis.browser;
    delete globalThis.location;
    delete globalThis.HTMLFormElement;
    delete globalThis.getComputedStyle;
    delete globalThis.document;
    delete globalThis.__rustyVaultListener;
  }
});
