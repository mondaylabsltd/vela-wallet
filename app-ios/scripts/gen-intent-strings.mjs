#!/usr/bin/env node
// The system surfaces' words, from the corpus (native parity N-I2).
//
// Siri phrases, a Shortcut's title, a widget's name in the gallery and the
// Control Center's label are read by iOS from string catalogs at build time —
// before the app runs, so `Loc` (the corpus engine) cannot answer them. This
// writes those catalogs from the same compiled corpus the app bundles
// (`assets/i18n/<lng>.json`), for the 15 languages, so the system's words
// and the app's are the same words:
//
//   VelaWallet/VelaWallet/AppShortcuts.xcstrings   Siri / Shortcuts phrases
//   VelaWallet/VelaWallet/Localizable.xcstrings    intent titles (app)
//   VelaWallet/VelaWidgets/Localizable.xcstrings   widget + control names
//
// No new words are invented: every value is a corpus string, or two of them
// joined. Each door has two phrases: the corpus's own sentence for it
// (`componentsUi.shortcuts.*`, "Send with {{app}}" — `{{app}}` is written as
// Siri's `${applicationName}`), and the app's name beside the dock's verb.
//
//   node app-ios/scripts/gen-intent-strings.mjs           write
//   node app-ios/scripts/gen-intent-strings.mjs --check   exit 1 when stale

import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const here = fileURLToPath(new URL(".", import.meta.url));
const iosRoot = join(here, "..");
const repo = join(iosRoot, "..");
const catalogs = join(repo, "assets", "i18n");

// The corpus's 15 tags → Apple's (the app's CFBundleLocalizations).
const LOCALES = {
  en: "en",
  zh: "zh-Hans",
  "zh-TW": "zh-Hant",
  "zh-HK": "zh-HK",
  ja: "ja",
  ko: "ko",
  vi: "vi",
  id: "id",
  tr: "tr",
  "es-MX": "es-MX",
  "pt-BR": "pt-BR",
  fr: "fr",
  de: "de",
  ru: "ru",
  it: "it",
};

const corpus = Object.fromEntries(
  Object.keys(LOCALES).map((tag) => [tag, JSON.parse(readFileSync(join(catalogs, `${tag}.json`), "utf8"))]),
);

function t(tag, key) {
  const pick = (tree) => key.split(".").reduce((node, part) => (node && typeof node === "object" ? node[part] : undefined), tree);
  const value = pick(corpus[tag]) ?? pick(corpus.en);
  if (typeof value !== "string" || value.length === 0) throw new Error(`corpus has no ${key} (${tag})`);
  return value;
}

const APP = "${applicationName}";

// Each door: the corpus key of its title, and the English phrases the
// provider declares (the catalog's source keys, as written in Swift).
const DOORS = [
  { id: "send", title: "componentsUi.dock.send", phrases: [`${APP} send`, `Send with ${APP}`] },
  { id: "receive", title: "componentsUi.dock.receive", phrases: [`${APP} receive`, `Receive with ${APP}`] },
  { id: "scan", title: "componentsUi.dock.scan", phrases: [`${APP} scan`, `Scan with ${APP}`] },
];

function unit(value) {
  return { stringUnit: { state: "translated", value } };
}

function catalog(entries) {
  const strings = {};
  for (const [key, valueFor] of Object.entries(entries).sort(([a], [b]) => a.localeCompare(b))) {
    const localizations = {};
    for (const [tag, apple] of Object.entries(LOCALES)) localizations[apple] = unit(valueFor(tag));
    strings[key] = { extractionState: "manual", localizations };
  }
  return `${JSON.stringify({ sourceLanguage: "en", strings, version: "1.0" }, null, 2)}\n`;
}

// The corpus's sentence for a door, with its `{{app}}` as Siri's
// `${applicationName}` — every phrase must name the app, so one that lost it
// is refused here rather than shipped.
function phrase(tag, key) {
  const text = t(tag, key);
  if (!text.includes("{{app}}")) throw new Error(`${key} (${tag}) does not name the app`);
  return text.replaceAll("{{app}}", APP);
}

// Siri phrases. The "with" phrase is the corpus's own sentence in every
// language (English's is the provider's source key, and the same words); the
// other says the app's name and the corpus's verb (the word order a spoken
// command takes in the language does not matter to Siri's matcher, which
// hears both orders).
const shortcuts = {};
for (const door of DOORS) {
  const [named, withApp] = door.phrases;
  shortcuts[named] = (tag) => (tag === "en" ? named : `${APP} ${t(tag, door.title)}`);
  shortcuts[withApp] = (tag) => phrase(tag, `componentsUi.shortcuts.${door.id}`);
}
// The provider's source keys are the English sentences: a corpus that says
// otherwise would leave Swift declaring a phrase no catalog carries.
for (const door of DOORS) {
  const english = phrase("en", `componentsUi.shortcuts.${door.id}`);
  if (english !== door.phrases[1]) {
    throw new Error(`componentsUi.shortcuts.${door.id} is "${english}", but VelaShortcuts declares "${door.phrases[1]}"`);
  }
}

const appStrings = {};
for (const door of DOORS) appStrings[`intent.${door.id}.title`] = (tag) => t(tag, door.title);

const widgetStrings = {
  "widget.balance.name": (tag) => t(tag, "home.totalBalance"),
  "widget.balance.description": (tag) => t(tag, "componentsUi.widget.balanceDescription"),
  "widget.scan.name": (tag) => t(tag, "componentsUi.scanner.title"),
  "widget.scan.description": (tag) => t(tag, "componentsUi.scanner.hint"),
  "control.scan.title": (tag) => t(tag, "componentsUi.dock.scan"),
};
// The intents are in the extension too (the control's action), so their
// titles are there as well.
Object.assign(widgetStrings, appStrings);

const outputs = [
  [join(iosRoot, "VelaWallet", "VelaWallet", "AppShortcuts.xcstrings"), catalog(shortcuts)],
  [join(iosRoot, "VelaWallet", "VelaWallet", "Localizable.xcstrings"), catalog(appStrings)],
  [join(iosRoot, "VelaWallet", "VelaWidgets", "Localizable.xcstrings"), catalog(widgetStrings)],
];

const check = process.argv.includes("--check");
let stale = 0;
for (const [path, text] of outputs) {
  const current = existsSync(path) ? readFileSync(path, "utf8") : "";
  if (current === text) continue;
  if (check) {
    console.error(`stale: ${path} — run node app-ios/scripts/gen-intent-strings.mjs`);
    stale += 1;
  } else {
    writeFileSync(path, text);
    console.log(`wrote ${path}`);
  }
}
if (check && stale) process.exit(1);
if (check) console.log(`ok: ${outputs.length} catalogs match the corpus`);
