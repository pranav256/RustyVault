import test from "node:test";
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { readFile } from "node:fs/promises";
import { fillLogin, readLogin } from "../background.js";

const require = createRequire(import.meta.url);
let chromium;
try { ({ chromium } = require("playwright")); }
catch (error) { if (error.code !== "MODULE_NOT_FOUND") throw error; }

test("real DOM: form destinations, named controls and image submitters", { skip: !chromium }, async t => {
  const browser = await chromium.launch({ headless: true, executablePath: process.env.CHROMIUM_EXECUTABLE_PATH });
  try {
    const page = await browser.newPage();
    await page.route("**/*", route => route.fulfill({ contentType: "text/html", body: "<!doctype html><title>Synthetic form tests</title>" }));
    await page.goto("https://rustyvault.test/login");
    const cases = [
      ["ordinary same-origin form", "", "", "", true],
      ["same-origin relative action", "/session", "", "", true],
      ["normalized default port", "https://rustyvault.test:443/session", "", "", true],
      ["cross-origin form", "https://outside.test/session", "", "", false],
      ["HTTP form", "http://rustyvault.test/session", "", "", false],
      ["different port", "https://rustyvault.test:444/session", "", "", false],
      ["button override", "/session", '<button formaction="https://outside.test/session">Continue</button>', "", false],
      ["relative button override", "/session", '<button formaction="/other-session">Continue</button>', "", true],
      ["protocol-relative override", "/session", '<button formaction="//outside.test/session">Continue</button>', "", false],
      ["HTTP submitter override", "/session", '<button formaction="http://rustyvault.test/session">Continue</button>', "", false],
      ["image override", "/session", '<input type="image" formaction="https://outside.test/session">', "", false],
      ["external image override", "/session", "", '<input form="login" type="image" formaction="https://outside.test/session">', false],
      ["external button override", "/session", "", '<button form="login" formaction="https://outside.test/session">Continue</button>', false],
      ["named action control", "https://outside.test/session", '<input name="action">', "", false],
      ["named elements control", "/session", '<input name="elements"><button formaction="https://outside.test/session">Continue</button>', "", false],
      ["named querySelectorAll control", "/session", '<input name="querySelectorAll">', "", true],
    ];
    for (const [name, action, inner, outer, allowed] of cases) {
      await t.test(name, async () => {
        await page.setContent(`<form id="login" action="${action || "/session"}"><input autocomplete="username" value="synthetic-user"><input type="password" value="synthetic-password">${inner}</form>${outer}`);
        const result = await page.evaluate(({ read, fill }) => {
          const capture = fn => { try { return { value: fn() }; } catch (error) { return { error: error.message }; } };
          const saved = capture(() => (0, eval)(`(${read})`)(location.origin));
          const filled = capture(() => (0, eval)(`(${fill})`)(location.origin, "synthetic-fill", "synthetic-secret"));
          return { saved, filled, passwordAfter: document.querySelector('input[type="password"]').value };
        }, { read: readLogin.toString(), fill: fillLogin.toString() });
        if (allowed) {
          assert.equal(result.saved.value?.password, "synthetic-password", JSON.stringify(result));
          assert.equal(result.filled.value, true, JSON.stringify(result));
          assert.equal(result.passwordAfter, "synthetic-secret");
        } else {
          assert.equal(result.saved.value, null, JSON.stringify(result));
          assert.equal(result.filled.value, false, JSON.stringify(result));
          assert.equal(result.passwordAfter, "synthetic-password");
        }
      });
    }
    const content = await readFile(new URL("../content.js", import.meta.url), "utf8");
    for (const [name, action, extra, override, allowed] of [
      ["ordinary submission", "/session", "", "", true],
      ["cross-origin submission", "https://outside.test/session", "", "", false],
      ["clobbered action submission", "https://outside.test/session", '<input name="action">', "", false],
      ["clobbered method submission", "/session", '<input name="querySelectorAll">', "", true],
      ["cross-origin submitter", "/session", "", 'formaction="https://outside.test/session"', false],
      ["same-origin submitter override", "https://outside.test/session", "", 'formaction="/local-session"', true],
    ]) {
      await t.test(name, async () => {
        await page.goto("https://rustyvault.test/login");
        await page.setContent(`<form action="${action}">${extra}<input type="password" value="synthetic-password"><button ${override}>Continue</button></form>`);
        await page.evaluate(() => {
          window.testMessages = [];
          window.browser = { runtime: { sendMessage: async message => {
            window.testMessages.push(message);
            return { allowed: true };
          } } };
          document.addEventListener("submit", event => event.preventDefault());
        });
        await page.addScriptTag({ content });
        await page.waitForFunction(() => window.__rustyVaultListener === true);
        await page.getByRole("button", { name: "Continue" }).click();
        const submitted = await page.evaluate(() => window.testMessages.filter(message => message.action === "submitted"));
        assert.equal(submitted.length, Number(allowed));
        if (allowed) assert.equal(submitted[0].password, "synthetic-password");
      });
    }
  } finally { await browser.close(); }
});
