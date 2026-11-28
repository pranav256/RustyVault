const api = globalThis.browser || globalThis.chrome;
const HOST = "com.pranav256.rustyvault";
const pending = new Map();

export function siteOrigin(value) {
  const url = new URL(value);
  if (url.protocol !== "https:" || url.username || url.password) throw new Error("Open an HTTPS website first.");
  return url.origin;
}

async function currentTab() {
  const [tab] = await api.tabs.query({ active: true, currentWindow: true });
  if (!tab?.id) throw new Error("No active website.");
  return { tab, origin: siteOrigin(tab.url) };
}

async function native(request) {
  let response;
  try { response = await api.runtime.sendNativeMessage(HOST, request); }
  catch { throw new Error("Connection unavailable. Open RustyVault, unlock it and enable the browser connection in Settings. See extension setup if needed."); }
  if (!response?.ok) throw new Error(response?.error || "RustyVault did not respond.");
  return response;
}

async function listenOnSite(tab, origin) {
  const pattern = `${new URL(origin).protocol}//${new URL(origin).hostname}/*`;
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(pattern));
  const id = [...new Uint8Array(digest)].map(value => value.toString(16).padStart(2, "0")).join("");
  const registration = {
    id: `site-${id}`,
    matches: [pattern],
    js: ["content.js"],
    runAt: "document_idle",
    allFrames: false,
    persistAcrossSessions: true,
  };
  const registrations = await api.scripting.getRegisteredContentScripts();
  if (!registrations.some(script => script.id === registration.id)) {
    await api.scripting.registerContentScripts([registration]);
  }
  await api.scripting.executeScript({ target: { tabId: tab.id }, files: ["content.js"] });
}

function forget(tabId) {
  const candidate = pending.get(tabId);
  if (candidate) candidate.password = "";
  pending.delete(tabId);
  api.action.setBadgeText({ tabId, text: "" }).catch(() => {});
}

export async function popupMessage(message) {
  const { tab, origin } = await currentTab();
  const pattern = `${new URL(origin).protocol}//${new URL(origin).hostname}/*`;
  const key = "site:" + origin;
  const allowed = (await api.storage.local.get(key))[key] === true &&
    await api.permissions.contains({ origins: [pattern] });
  if (message.action === "authorize") {
    if (message.origin !== origin || !await api.permissions.contains({ origins: [pattern] })) {
      throw new Error("Website changed or access was declined. Open the popup and try again.");
    }
    await api.storage.local.set({ [key]: true });
    return { ok: true };
  }
  if (message.action === "context") {
    const candidate = pending.get(tab.id);
    if (candidate && (candidate.expires < Date.now() || candidate.origin !== origin)) forget(tab.id);
    return { ok: true, origin, allowed, pending: pending.has(tab.id) };
  }
  if (!allowed) throw new Error("Enable RustyVault on this site first.");
  await listenOnSite(tab, origin);
  if (message.action === "list") {
    const response = await native({ action: "list", origin });
    return { ok: true, logins: response.logins || [] };
  }
  if (message.action === "fill") {
    if (typeof message.id !== "string") throw new Error("Choose a login.");
    const response = await native({ action: "fill", origin, id: message.id });
    try {
      const result = await api.scripting.executeScript({
        target: { tabId: tab.id },
        func: fillLogin,
        args: [origin, response.username, response.password],
      });
      if (!result[0]?.result) throw new Error("No visible password field with a same-site form was found.");
    } finally { response.password = ""; response.username = ""; }
    return { ok: true };
  }
  if (message.action === "save") {
    let candidate = pending.get(tab.id);
    if (!candidate || candidate.expires < Date.now() || candidate.origin !== origin) {
      const result = await api.scripting.executeScript({ target: { tabId: tab.id }, func: readLogin, args: [origin] });
      candidate = result[0]?.result;
    }
    if (!candidate?.password) throw new Error("Enter a login on this site, then choose Save login.");
    try { await native({ action: "save", origin, username: candidate.username, password: candidate.password }); }
    finally { candidate.password = ""; forget(tab.id); }
    return { ok: true };
  }
  throw new Error("Unknown request.");
}

export function readLogin(expectedOrigin) {
  if (location.origin !== expectedOrigin || location.protocol !== "https:") return null;
  const visible = input => !input.disabled && !input.readOnly && input.getClientRects().length && getComputedStyle(input).visibility !== "hidden";
  const passwords = [...document.querySelectorAll('input[type="password"]')].filter(visible);
  const password = passwords.find(input => input.autocomplete === "new-password") || passwords.find(input => input.autocomplete === "current-password") || passwords[0];
  if (!password) return null;
  const form = password.form;
  // Named controls can shadow form.action; form.elements also excludes image submitters.
  const action = form && Object.getOwnPropertyDescriptor(HTMLFormElement.prototype, "action").get.call(form);
  if (form && (new URL(action || location.href, location.href).origin !== expectedOrigin ||
      [...document.querySelectorAll('button[formaction], input[formaction]')].some(control =>
        control.form === form && ["submit", "image"].includes(control.type) &&
        new URL(control.formAction || action || location.href, location.href).origin !== expectedOrigin))) return null;
  const fields = [...document.querySelectorAll('input')].filter(input => (!form || input.form === form) && visible(input));
  const username = fields.find(input => input.autocomplete === "username") || fields.find(input => input.type === "email") || fields.find(input => input.type === "text");
  return { username: username?.value || "", password: password.value };
}

export function fillLogin(expectedOrigin, username, secret) {
  if (location.origin !== expectedOrigin || location.protocol !== "https:") return false;
  const visible = input => !input.disabled && !input.readOnly && input.getClientRects().length && getComputedStyle(input).visibility !== "hidden";
  const password = [...document.querySelectorAll('input[type="password"]')].filter(visible).find(input => input.autocomplete !== "new-password");
  if (!password) return false;
  const form = password.form;
  const action = form && Object.getOwnPropertyDescriptor(HTMLFormElement.prototype, "action").get.call(form);
  if (form && (new URL(action || location.href, location.href).origin !== expectedOrigin ||
      [...document.querySelectorAll('button[formaction], input[formaction]')].some(control =>
        control.form === form && ["submit", "image"].includes(control.type) &&
        new URL(control.formAction || action || location.href, location.href).origin !== expectedOrigin))) return false;
  const fields = [...document.querySelectorAll('input')].filter(input => (!form || input.form === form) && visible(input));
  const user = fields.find(input => input.autocomplete === "username") || fields.find(input => input.type === "email") || fields.find(input => input.type === "text");
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
  const fill = (input, value) => {
    setter.call(input, value);
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.dispatchEvent(new Event("change", { bubbles: true }));
  };
  if (user) fill(user, username);
  fill(password, secret);
  password.focus();
  return true;
}

export async function contentMessage(message, sender) {
  const origin = siteOrigin(sender.url);
  if (sender.frameId !== 0 || origin !== message.origin) return { ok: false, allowed: false };
  const key = "site:" + origin;
  const pattern = `https://${new URL(origin).hostname}/*`;
  const allowed = (await api.storage.local.get(key))[key] === true &&
    await api.permissions.contains({ origins: [pattern] });
  if (message.action === "siteAccess") return { ok: true, allowed };
  if (!allowed || message.action !== "submitted" ||
      typeof message.username !== "string" || message.username.length > 2048 ||
      typeof message.password !== "string" || !message.password || message.password.length > 16384) {
    return { ok: false };
  }
  forget(sender.tab.id);
  const candidate = { origin, username: message.username, password: message.password, expires: Date.now() + 60000 };
  pending.set(sender.tab.id, candidate);
  api.action.setBadgeText({ tabId: sender.tab.id, text: "Save" }).catch(() => {});
  setTimeout(() => { if (pending.get(sender.tab.id) === candidate) forget(sender.tab.id); }, 60000);
  return { ok: true };
}

if (api) {
  api.runtime.onMessage.addListener((message, sender, reply) => {
    if (sender.id !== api.runtime.id) return;
    if (sender.url === api.runtime.getURL("popup.html")) {
      popupMessage(message).then(reply, error => reply({ ok: false, error: error.message }));
      return true;
    }
    if (sender.tab && sender.frameId === 0 && ["siteAccess", "submitted"].includes(message.action)) {
      contentMessage(message, sender).then(reply, () => reply({ ok: false, allowed: false }));
      return true;
    }
  });
  api.tabs.onRemoved.addListener(forget);
}
