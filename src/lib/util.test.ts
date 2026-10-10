import { describe, expect, it } from "vitest";
import type { GameDto } from "./types";
import { droppedNote, formatLastPlayed, groupGames, matchesWhen, mergeStyle, normalizeGameName } from "./util";

describe("util", () => {
  it("normalizeGameName folds case, accents and punctuation", () => {
    expect(normalizeGameName("Pokémon: Let’s Go!")).toBe("pokemon let s go");
  });

  it("groupGames folds the same title across sources, keeping first-seen order", () => {
    const g = (app_id: number, name: string, source: string) => ({ app_id, name, source }) as unknown as GameDto;
    const groups = groupGames([g(1, "Hades", "heroic"), g(2, "Celeste", "steam"), g(3, "HADES", "steam")]);
    expect(groups.map((x) => x.entries.map((e) => e.app_id))).toEqual([[3, 1], [2]]);
  });

  it("mergeStyle joins styles without losing bits-ui's", () => {
    expect(mergeStyle({ style: "pointer-events: auto;" }, "width: 3px")).toBe(
      "pointer-events: auto; width: 3px;",
    );
    expect(mergeStyle({}, false, null)).toBe("");
  });

  it("droppedNote names what an import couldn't keep", () => {
    expect(droppedNote([])).toBe("");
    expect(droppedNote(["strangle", "60"])).toBe(" — 2 parts not imported: strangle 60");
  });

  it("formatLastPlayed is null for never-played", () => {
    expect(formatLastPlayed(null)).toBeNull();
    expect(formatLastPlayed(0)).toBeNull();
  });
});

describe("matchesWhen", () => {
  it("needs every tag, honours !negation, and never matches without a scan", () => {
    const tags = ["dlss", "unreal"];
    expect(matchesWhen(["dlss"], tags)).toBe(true);
    expect(matchesWhen(["dlss", "!anticheat"], tags)).toBe(true);
    expect(matchesWhen(["dlss", "!unreal"], tags)).toBe(false);
    expect(matchesWhen(["fsr"], tags)).toBe(false);
    expect(matchesWhen([], tags)).toBe(false);
    expect(matchesWhen(["dlss"], undefined)).toBe(false);
  });
});
