(async () => {
  const api = globalThis.browser || globalThis.chrome;
  if (globalThis.__rustyVaultListener) return;
  const access = await api.runtime.sendMessage({ action: "siteAccess", origin: location.origin }).catch(() => null);
  if (!access?.allowed || globalThis.__rustyVaultListener) return;
  globalThis.__rustyVaultListener = true;
  document.addEventListener("submit", event => {
    if (!event.isTrusted || location.protocol !== "https:" || !(event.target instanceof HTMLFormElement)) return;
    const form = event.target;
    const action = event.submitter?.hasAttribute("formaction")
      ? event.submitter.formAction
      : Object.getOwnPropertyDescriptor(HTMLFormElement.prototype, "action").get.call(form);
    if (new URL(action, location.href).origin !== location.origin) return;
    const visible = input => !input.disabled && input.getClientRects().length && getComputedStyle(input).visibility !== "hidden";
    const fields = [...document.querySelectorAll("input")].filter(input => input.form === form && visible(input));
    const passwords = fields.filter(input => input.type === "password");
    const password = passwords.find(input => input.autocomplete === "new-password") || passwords.find(input => input.autocomplete === "current-password") || passwords[0];
    if (!password?.value || password.value.length > 16384) return;
    const user = fields.find(input => input.autocomplete === "username") || fields.find(input => input.type === "email") || fields.find(input => input.type === "text");
    if ((user?.value.length || 0) > 2048) return;
    api.runtime.sendMessage({ action: "submitted", origin: location.origin, username: user?.value || "", password: password.value }).catch(() => {});
  }, true);
})().catch(() => {});
