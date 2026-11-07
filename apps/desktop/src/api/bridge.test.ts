import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";

function source(relative: string) {
  return readFileSync(new URL(relative, import.meta.url), "utf8");
}

describe("Tauri command bridge", () => {
  it("implements and registers every command invoked by React", () => {
    const frontend = source("./commands.ts");
    const commands = source("../../src-tauri/src/commands.rs");
    const registration = source("../../src-tauri/src/lib.rs");
    const invoked = new Set(
      [...frontend.matchAll(/invoke(?:<[^>]+>)?\("([a-z0-9_]+)"/g)].map((match) => match[1]),
    );
    const implemented = new Set(
      [...commands.matchAll(/pub (?:async )?fn ([a-z0-9_]+)\(/g)].map((match) => match[1]),
    );
    const registered = new Set(
      [...registration.matchAll(/commands::([a-z0-9_]+)/g)].map((match) => match[1]),
    );

    expect([...invoked].filter((name) => !implemented.has(name))).toEqual([]);
    expect([...invoked].filter((name) => !registered.has(name))).toEqual([]);
  });
});
