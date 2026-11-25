const api = globalThis.browser || globalThis.chrome;
const $ = id => document.getElementById(id);
let currentOrigin;

async function send(action, extra = {}) {
  const response = await api.runtime.sendMessage({ action, ...extra });
  if (!response?.ok) throw new Error(response?.error || "The extension did not respond.");
  return response;
}

async function run(task) {
  for (const button of document.querySelectorAll("button")) button.disabled = true;
  try { await task(); }
  catch (error) { $("status").textContent = error.message; }
  finally { for (const button of document.querySelectorAll("button")) button.disabled = false; }
}

async function refresh() {
  $("logins").replaceChildren();
  $("save").hidden = true;
  $("enable").hidden = true;
  const context = await send("context");
  currentOrigin = context.origin;
  $("site").textContent = currentOrigin;
  $("enable").hidden = context.allowed;
  if (!context.allowed) {
    $("status").textContent = "Allow RustyVault to read and fill login forms on this site.";
    return;
  }
  $("save").hidden = false;
  $("save").textContent = context.pending ? "Review submitted login" : "Save login on this page";
  const { logins } = await send("list");
  $("status").textContent = context.pending ? "A submitted login is ready to save. Review it within one minute." : logins.length ? "Choose a login to fill." : "No matching logins. Enter your login on the page, then save it here.";
  for (const login of logins) {
    const button = document.createElement("button");
    const title = document.createElement("span");
    title.textContent = login.title;
    const username = document.createElement("small");
    username.textContent = login.username;
    button.append(title, username);
    button.addEventListener("click", () => run(async () => {
      await send("fill", { id: login.id });
      $("status").textContent = "Login filled. Submit the website form when you are ready.";
    }));
    $("logins").append(button);
  }
}

$("enable").addEventListener("click", () => run(async () => {
  const url = new URL(currentOrigin);
  const granted = await api.permissions.request({ origins: [`${url.protocol}//${url.hostname}/*`] });
  if (!granted) throw new Error("Site access was declined. You can enable it later.");
  await send("authorize", { origin: currentOrigin });
  await refresh();
}));
$("save").addEventListener("click", () => run(async () => {
  await send("save");
  $("status").textContent = "Review the website and account in RustyVault, then choose Save login.";
}));
$("retry").addEventListener("click", () => run(refresh));
run(refresh);
