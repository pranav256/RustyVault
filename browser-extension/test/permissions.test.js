import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

test("Firefox and Chromium share exact-origin authorization without storing passwords", async () => {
  const stored = {};
  let granted = true;
  let url = "https://example.com/login";
  let nativeCalls = 0;
  const listeners = [];
  const api = {
    storage: { local: {
      get: async key => ({ [key]: stored[key] }),
      set: async values => Object.assign(stored, values),
    } },
    permissions: { contains: async () => granted },
    tabs: { query: async () => [{ id: 1, url }], onRemoved: { addListener() {} } },
    runtime: { id: "test", getURL: path => "moz-extension://test/" + path,
      onMessage: { addListener: listener => listeners.push(listener) },
      sendNativeMessage: async () => { nativeCalls++; return { ok: true, logins: [] }; },
    },
    scripting: { getRegisteredContentScripts: async () => [], registerContentScripts: async () => {}, executeScript: async () => [] },
    action: { setBadgeText: async () => {} },
  };
  globalThis.browser = api;
  // The Firefox namespace must win even when a chrome compatibility namespace exists.
  globalThis.chrome = {};
  try {
    const { popupMessage, contentMessage } = await import("../background.js?permissions");
    const sender = { tab: { id: 1 }, frameId: 0, url };
    const access = origin => contentMessage({ action: "siteAccess", origin }, sender);
    assert.equal((await popupMessage({ action: "context" })).allowed, false);
    await assert.rejects(popupMessage({ action: "list" }), /Enable/);
    assert.equal((await access("https://example.com")).allowed, false);
    await assert.rejects(popupMessage({ action: "authorize", origin: "https://elsewhere.test" }), /changed/);
    await popupMessage({ action: "authorize", origin: "https://example.com" });
    assert.equal((await access("https://example.com")).allowed, true);
    await popupMessage({ action: "list" });
    assert.equal(nativeCalls, 1);
    url = sender.url = "https://example.com:444/login";
    assert.equal((await popupMessage({ action: "context" })).allowed, false);
    assert.equal((await access("https://example.com:444")).allowed, false);
    assert.equal((await contentMessage({ action: "submitted", origin: "https://example.com:444", username: "user", password: "synthetic" }, sender)).ok, false);
    await assert.rejects(popupMessage({ action: "fill", id: "id" }), /Enable/);
    url = sender.url = "https://example.com/login";
    granted = false;
    assert.equal((await access("https://example.com")).allowed, false);
    await assert.rejects(popupMessage({ action: "authorize", origin: "https://example.com" }), /declined/);
    assert.equal(nativeCalls, 1);
    assert.deepEqual(stored, { "site:https://example.com": true });
    assert.equal(listeners.length, 1);
  } finally {
    delete globalThis.browser;
    delete globalThis.chrome;
  }
});

test("shared manifest includes both background environments and a fixed Firefox ID", async () => {
  const manifest = JSON.parse(await readFile(new URL("../manifest.json", import.meta.url), "utf8"));
  assert.equal(manifest.background.service_worker, "background.js");
  assert.deepEqual(manifest.background.scripts, ["background.js"]);
  assert.equal(manifest.background.type, "module");
  assert.equal(manifest.browser_specific_settings.gecko.id, "rustyvault@pranav256.com");
  assert.ok(manifest.permissions.includes("storage"));
  assert.deepEqual(manifest.optional_host_permissions, ["https://*/*"]);
  assert.equal(manifest.host_permissions, undefined);
});
