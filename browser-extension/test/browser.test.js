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
  globalThis.document = { querySelectorAll: selector => selector === 'input[type="password"]' ? [password] : [] };
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
  globalThis.document = { querySelectorAll: selector => selector === 'input[type="password"]' ? [password] : [] };
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
