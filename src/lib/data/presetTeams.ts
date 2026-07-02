/**
 * Preset teams — app-bundled, curated squads of agents you can deploy as a unit.
 *
 * These are starting points: a "team" is a hand-picked set pulled from across
 * divisions for a goal (ship a mobile app, run growth, etc.). Each references
 * corpus agent slugs (`<division>-<name>`); slugs not present in the active
 * catalog are silently skipped at deploy time, so a preset degrades gracefully.
 *
 * Curated by hand and bundled (no catalog/network dependency). To add one:
 * append an entry with a Lucide icon, a brand color, and verified slugs.
 */

import type { Component } from "svelte";
import Rocket from "@lucide/svelte/icons/rocket";
import Code from "@lucide/svelte/icons/code";
import TrendingUp from "@lucide/svelte/icons/trending-up";
import Compass from "@lucide/svelte/icons/compass";
import Sparkles from "@lucide/svelte/icons/sparkles";

export interface PresetTeam {
  slug: string;
  label: string;
  description: string;
  icon: Component;
  /** Brand color (hex) for the icon tint. */
  color: string;
  /** Corpus agent slugs that make up the team. */
  agents: string[];
}

type PresetLocale = "en" | "ru";

export const PRESET_TEAMS: PresetTeam[] = [
  {
    slug: "mobile-launch",
    label: "Mobile Launch",
    description: "Take a mobile app from prototype to the App Store.",
    icon: Rocket,
    color: "#6366f1",
    agents: [
      "engineering-mobile-app-builder",
      "engineering-rapid-prototyper",
      "design-ui-designer",
      "design-ux-researcher",
      "marketing-app-store-optimizer",
    ],
  },
  {
    slug: "ship-web-app",
    label: "Ship It (Web)",
    description: "Build, deploy, and keep a web app healthy in production.",
    icon: Code,
    color: "#0ea5e9",
    agents: [
      "engineering-frontend-developer",
      "engineering-backend-architect",
      "engineering-devops-automator",
      "engineering-sre",
      "testing-api-tester",
    ],
  },
  {
    slug: "growth-squad",
    label: "Growth Squad",
    description: "Drive acquisition across content, search, and social.",
    icon: TrendingUp,
    color: "#ec4899",
    agents: [
      "marketing-growth-hacker",
      "marketing-content-creator",
      "marketing-seo-specialist",
      "marketing-tiktok-strategist",
      "marketing-instagram-curator",
    ],
  },
  {
    slug: "product-discovery",
    label: "Product Discovery",
    description: "Find what to build next and sequence the work.",
    icon: Compass,
    color: "#f59e0b",
    agents: [
      "product-sprint-prioritizer",
      "product-trend-researcher",
      "product-feedback-synthesizer",
      "design-ux-researcher",
    ],
  },
  {
    slug: "ai-builders",
    label: "AI Builders",
    description: "Ship AI features on a solid data + prompt foundation.",
    icon: Sparkles,
    color: "#8b5cf6",
    agents: [
      "engineering-ai-engineer",
      "engineering-prompt-engineer",
      "engineering-data-engineer",
      "engineering-backend-architect",
    ],
  },
];

const PRESET_TEAM_RU: Record<string, { label: string; description: string }> = {
  "mobile-launch": {
    label: "Мобильный запуск",
    description: "Провести мобильное приложение от прототипа до App Store.",
  },
  "ship-web-app": {
    label: "Web-релиз",
    description: "Собрать, развернуть и поддерживать web-приложение в production.",
  },
  "growth-squad": {
    label: "Growth-команда",
    description: "Растить acquisition через контент, поиск и социальные каналы.",
  },
  "product-discovery": {
    label: "Product Discovery",
    description: "Найти, что строить дальше, и разложить работу по очереди.",
  },
  "ai-builders": {
    label: "AI Builders",
    description: "Выпускать AI-функции на прочной базе данных и промптов.",
  },
};

export function presetTeamText(team: PresetTeam, locale: PresetLocale): { label: string; description: string } {
  if (locale !== "ru") return { label: team.label, description: team.description };
  return PRESET_TEAM_RU[team.slug] ?? { label: team.label, description: team.description };
}
