import { describe, expect, it } from "vitest";
import { csPerMinute, duration, importEtaMinutes, kda, phaseLabel, positionLabel, relativeDate } from "./format";

describe("format", () => {
  it("translates client phases", () => {
    expect(phaseLabel("ChampSelect")).toBe("Seleção de campeões");
    expect(phaseLabel("InProgress")).toBe("Em partida");
    expect(phaseLabel(null)).toBe("No cliente");
    expect(phaseLabel("NewUnknownPhase")).toBe("NewUnknownPhase");
  });

  it("translates positions", () => {
    expect(positionLabel("UTILITY")).toBe("Suporte");
    expect(positionLabel("")).toBe("—");
  });

  it("computes KDA", () => {
    expect(kda(5, 2, 7)).toBe("6,00");
    expect(kda(3, 0, 1)).toBe("Perfeito");
  });

  it("formats duration and CS/min", () => {
    expect(duration(1805)).toBe("30:05");
    expect(csPerMinute(192, 1800)).toBe("6,4");
    expect(csPerMinute(10, 0)).toBe("0,0");
  });

  it("formats relative dates", () => {
    const now = Date.UTC(2026, 10, 17);
    expect(relativeDate(now - 1000, now)).toBe("hoje");
    expect(relativeDate(now - 86_400_000, now)).toBe("ontem");
    expect(relativeDate(now - 5 * 86_400_000, now)).toBe("há 5 dias");
  });

  it("estimates import time with the dev key budget", () => {
    // 100 matches * 2 requests at 90 req / 120 s ≈ 4.4 min
    expect(importEtaMinutes(100)).toBe(5);
    expect(importEtaMinutes(0)).toBe(0);
  });
});
