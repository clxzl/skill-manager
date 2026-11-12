import assert from "node:assert/strict";
import { getAutoSync, setAutoSync } from "../src/autoSync.ts";

const store = new Map<string, string>();
globalThis.localStorage = {
  getItem: (key: string) => store.get(key) ?? null,
  setItem: (key: string, value: string) => void store.set(key, value),
} as Storage;

assert.deepEqual(getAutoSync(), { enabled: false, minutes: 10 });

setAutoSync({ enabled: true, minutes: 30 });
assert.deepEqual(getAutoSync(), { enabled: true, minutes: 30 });

store.set("autoSync", "{broken");
assert.deepEqual(getAutoSync(), { enabled: false, minutes: 10 });

store.set("autoSync", JSON.stringify({ enabled: true, minutes: 7 }));
assert.deepEqual(getAutoSync(), { enabled: true, minutes: 10 });

store.set("autoSync", JSON.stringify({ enabled: "yes", minutes: 5 }));
assert.deepEqual(getAutoSync(), { enabled: false, minutes: 5 });

console.log("autoSync self-check passed");