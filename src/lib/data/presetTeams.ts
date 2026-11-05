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
import Building2 from "@lucide/svelte/icons/building-2";

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
  {
    // A whole office, not a squad: staffed like a real bookkeeping & tax
    // practice — partner, tax consultant, bookkeeping desk, payables, reporting,
    // compliance, client relations, office operations, and document desk.
    slug: "liyan-mitra-co",
    label: "Liyan Mitra.co",
    description: "A full bookkeeping & tax services office, from client intake to filed returns.",
    icon: Building2,
    color: "#0f766e",
    agents: [
      "chief-financial-officer",
      "finance-tax-strategist",
      "finance-bookkeeper-controller",
      "accounts-payable-agent",
      "finance-financial-analyst",
      "support-legal-compliance-checker",
      "customer-success-manager",
      "operations-manager",
      "specialized-document-generator",
    ],
  },
];
