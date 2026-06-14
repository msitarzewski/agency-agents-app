#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const repo = path.resolve(import.meta.dirname, "../..");

function readJson(rel) {
  return JSON.parse(fs.readFileSync(path.join(repo, rel), "utf8"));
}

function read(rel) {
  return fs.readFileSync(path.join(repo, rel), "utf8");
}

function walk(dir, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (entry.name === "node_modules" || entry.name === "target" || entry.name === ".svelte-kit") continue;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, out);
    else out.push(full);
  }
  return out;
}

const failures = [];
const checks = [];

function check(name, ok, detail = "") {
  checks.push({ name, ok, detail });
  if (!ok) failures.push(`${name}${detail ? `: ${detail}` : ""}`);
}

const shared = readJson("src-tauri/tauri.conf.json");
const macos = readJson("src-tauri/tauri.macos.conf.json");
const cargo = read("src-tauri/Cargo.toml");

const sharedWindow = shared.app?.windows?.[0] ?? {};
const macWindow = macos.app?.windows?.[0] ?? {};

check("shared config disables macOS private API", shared.app?.macOSPrivateApi !== true);
check("shared config uses opaque windows", sharedWindow.transparent === false);
check("shared config uses native decorations", sharedWindow.decorations === true);
check("shared config has no overlay title bar", sharedWindow.titleBarStyle === undefined);
check("shared config has no traffic-light position", sharedWindow.trafficLightPosition === undefined);

check("macOS config enables private API", macos.app?.macOSPrivateApi === true);
check("macOS config keeps transparent window", macWindow.transparent === true);
check("macOS config keeps overlay title bar", macWindow.titleBarStyle === "Overlay");
check("macOS config keeps hidden title", macWindow.hiddenTitle === true);
check("macOS config keeps traffic-light position", typeof macWindow.trafficLightPosition?.x === "number");

const baseTauriLine = cargo.split(/\r?\n/).find((line) => line.startsWith("tauri = { version = \"2\""));
const macTauriLine = cargo
  .split(/\r?\n/)
  .slice(cargo.split(/\r?\n/).findIndex((line) => line.includes("cfg(target_os = \"macos\")")))
  .find((line) => line.startsWith("tauri = { version = \"2\""));

check("base tauri dependency excludes macos-private-api", baseTauriLine && !baseTauriLine.includes("macos-private-api"));
check("macOS target tauri dependency includes macos-private-api", macTauriLine?.includes("macos-private-api") === true);

const hardcodedShortcutHits = [];
for (const file of walk(path.join(repo, "src"))) {
  if (!/\.(svelte|ts|js)$/.test(file)) continue;
  const rel = path.relative(repo, file);
  const lines = fs.readFileSync(file, "utf8").split(/\r?\n/);
  lines.forEach((line, idx) => {
    if (!line.includes("⌘")) return;
    const trimmed = line.trim();
    if (
      trimmed.startsWith("//") ||
      trimmed.startsWith("*") ||
      trimmed.startsWith("/*") ||
      trimmed.includes("shortcut(") ||
      trimmed.includes("modKey")
    ) {
      return;
    }
    hardcodedShortcutHits.push(`${rel}:${idx + 1}: ${trimmed}`);
  });
}
check("no hardcoded command-key UI labels", hardcodedShortcutHits.length === 0, hardcodedShortcutHits.join("\n"));

for (const item of checks) {
  const mark = item.ok ? "PASS" : "FAIL";
  console.log(`${mark} ${item.name}${item.detail && !item.ok ? `\n${item.detail}` : ""}`);
}

if (failures.length > 0) {
  console.error(`\n${failures.length} Phase C config validation check(s) failed.`);
  process.exit(1);
}
