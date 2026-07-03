/**
 * Playbook — the in-app "how to get real work out of your agents" content.
 *
 * Single source of truth: the Playbook modal, the empty-state nudges, the Team
 * detail panel, and the docs page all render from here so the copy never drifts.
 * Pure data, no Svelte — safe to import anywhere (incl. a docs generator).
 */

export interface Practice {
  id: string;
  title: string;
  body: string;
}

/** The five habits that turn a pile of agents into shipped, tested work. */
export const PLAYBOOK_PRACTICES: Practice[] = [
  {
    id: "outcome",
    title: "Name the outcome, not the steps",
    body: "Tell the team what “done” looks like — the work product and how you'll know it's finished — then let them plan the path. The more concrete the done-bar, the better the result.",
  },
  {
    id: "team",
    title: "Cast a team, not a soloist",
    body: "One agent is a contractor; a team is a studio. Deploy a Team (or a whole Division) so a builder and a critic work together — pair a maker with a Code Reviewer and a Reality Checker so nothing gets rubber-stamped.",
  },
  {
    id: "tested",
    title: "Loop until it's tested",
    body: "“It compiles” isn't done. Make the loop exit on evidence: browser-driven, functional, unit, and a pen-test pass where it applies. Ask for the proof, not the promise.",
  },
  {
    id: "context",
    title: "Feed it context",
    body: "Garbage in, garbage out. Point at the repo, the constraints, and the success criteria up front. Five minutes of context beats fifty prompts of correction.",
  },
  {
    id: "scale",
    title: "Start with one, then scale",
    body: "New to this? Install one agent into one project and give it one task. Watch it work — then graduate to a team and the loop.",
  },
];

export interface StarterPromptDef {
  id: string;
  label: string;
  description: string;
  template: string;
}

/** Copy-paste starting points. Swap the [brackets], paste into your tool. */
export const STARTER_PROMPTS: StarterPromptDef[] = [
  {
    id: "build-test-loop",
    label: "Build & test loop",
    description: "The workhorse: assemble a team and don't stop until it's proven.",
    template:
      "Use all available agents as a team and work in a loop until [the work product] is built and tested — browser, functional, unit, and pen-test where applicable. Don't stop until every gate passes; show me the evidence for each.",
  },
  {
    id: "review-pass",
    label: "Review pass",
    description: "A builder, a reviewer, and a skeptic — catch what a solo pass misses.",
    template:
      "Review [the change or file]. Have a Code Reviewer assess correctness, security, and maintainability, then have a Reality Checker independently verify each finding with proof. Give me only the issues that survive scrutiny, ranked by severity.",
  },
  {
    id: "discovery-pass",
    label: "Discovery pass",
    description: "Research and sequence before you build.",
    template:
      "Before building, research [the problem]. Have a Trend Researcher and Feedback Synthesizer map the landscape and user needs, then a Sprint Prioritizer turn it into a sequenced plan with a clear first milestone. Surface assumptions and open questions.",
  },
];

/**
 * Curated example tasks per preset team (keyed by the team's slug in
 * `presetTeams.ts`). Each is a ready starting prompt that shows what that squad
 * is actually for — swap the [brackets] and go.
 */
export const TEAM_EXAMPLES: Record<string, string[]> = {
  "mobile-launch": [
    "Take the prototype in [path] to a TestFlight-ready build: implement the core flow, polish the UI, and loop until it runs clean on a simulator with unit + UI tests passing.",
    "Draft the App Store listing — name, subtitle, keywords, and a screenshots plan — optimized for [category], and validate the copy against ASO best practices.",
  ],
  "ship-web-app": [
    "Build [feature] end to end — API, frontend, and tests — then wire CI and a deploy, and loop until it's green in staging with functional + unit coverage.",
    "Audit production readiness for [app]: health checks, error budgets, and a rollback plan. Report the gaps with fixes, ranked by risk.",
  ],
  "growth-squad": [
    "Audit [landing page URL] and ship 3 A/B test variants with tracking wired in; loop until each variant is live and measurable.",
    "Plan a two-week content + social calendar to drive signups for [product], with SEO-targeted posts and platform-native hooks for TikTok and Instagram.",
  ],
  "product-discovery": [
    "Synthesize [feedback source] into the top 5 opportunities, size each, and produce a sequenced roadmap with a clear first milestone.",
    "Research what competitors ship for [problem] and recommend what we should build next, with assumptions and risks called out.",
  ],
  "ai-builders": [
    "Design and build [AI feature] on a solid data + prompt foundation: schema, retrieval, prompts, and evals — loop until the evals pass on a held-out set.",
    "Harden our prompts for [use case]: write a suite of test cases, measure the pass rate, and iterate until it's reliable.",
  ],
};

/**
 * Domain-relevant starter prompt per division (keyed by division slug). Used by
 * the division overview banner so each division suggests work that actually fits
 * it — the quality gates differ by craft (Engineering tests code; Design tests
 * usability; Finance reconciles numbers). Falls back to a neutral loop for any
 * division without a curated line (see `divisionPrompt`).
 */
export const DIVISION_PROMPTS: Record<string, string> = {
  academic:
    "Use the Academic division as a team to research [topic] and produce a literature-backed [paper/report] — loop until every claim is cited and the argument is peer-review ready.",
  design:
    "Use the Design division as a team to take [feature or screen] from brief to a polished, accessible UI — loop until the visual design, the copy, and a usability pass are all signed off.",
  engineering:
    "Use the Engineering division as a team and work in a loop until [the feature] is built and tested — functional, unit, integration, and a security pass — with every gate green.",
  finance:
    "Use the Finance division as a team to build [the model or budget] — loop until the numbers reconcile, the assumptions are documented, and a scenario analysis holds up.",
  "game-development":
    "Use the Game Development division as a team to prototype [the mechanic] — loop until it's playable, fun-tested, and inside its performance budget.",
  gis:
    "Use the GIS division as a team to build [the map or analysis] from [data] — loop until the data is clean, the projection is correct, and the output is validated.",
  marketing:
    "Use the Marketing division as a team to plan and produce [the campaign] — loop until the messaging, channels, and assets are ready and the success metrics are defined.",
  "paid-media":
    "Use the Paid Media division as a team to build [the ad campaign] across [channels] — loop until targeting, creative, and conversion tracking are wired and a budget/bid plan is set.",
  product:
    "Use the Product division as a team to turn [the problem] into a sequenced plan — loop until the opportunity is sized, scoped, and has a clear first milestone.",
  "project-management":
    "Use the Project Management division as a team to plan [the initiative] — loop until scope, timeline, risks, and owners are defined and dependencies are mapped.",
  sales:
    "Use the Sales division as a team to build [the outbound motion or deal strategy] for [target] — loop until the ICP, messaging, and sequence are ready and the likely objections are handled.",
  security:
    "Use the Security division as a team to assess [the system] — loop until threats are modeled, each finding is verified with proof, and every one has a prioritized remediation.",
  "spatial-computing":
    "Use the Spatial Computing division as a team to prototype [the AR/VR experience] — loop until the interaction model works, it runs within performance budget, and comfort is tested.",
  specialized:
    "Use the Specialized division as a team to deliver [the outcome] — loop until it's built, reviewed against the goal, and verified with evidence.",
  strategy:
    "Use the Strategy division as a team to develop [the strategy] for [goal] — loop until the analysis, the options, and a recommended path with its trade-offs are clear.",
  support:
    "Use the Support division as a team to handle [the support workflow] — loop until the responses are accurate, the tone is on-brand, and the escalation paths are defined.",
  testing:
    "Use the Testing division as a team to build [the test plan or suite] for [system] — loop until coverage hits the critical paths, edge cases are covered, and the suite runs green and deterministic.",
};

/** The division's curated prompt, or a craft-neutral loop fallback. */
export function divisionPrompt(slug: string, label: string): string {
  return (
    DIVISION_PROMPTS[slug] ??
    `Use the ${label} division as a team and work in a loop until [the work product] is done and reviewed against the goal.`
  );
}
