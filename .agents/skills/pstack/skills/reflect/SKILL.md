---
name: reflect
description: Spawn three parallel review subagents over the active transcript, surface learnings, and route each to a concrete edit on an existing skill. Use when the user says reflect.
disable-model-invocation: true
---

# Reflect

Mine the current conversation for durable learnings, then route them into skill edits.

Invoke when the user says "reflect" or "/reflect". Skip when the conversation is trivial, off-topic, or already covered by an existing skill the parent followed correctly. One-offs are not learnings.

## 1. Locate the active transcript

Before fanning out, find this thread's ID. It is in the environment as the Amp Thread URL. Reviewers read the thread with `read_thread`. Do not search other threads with `amp.find_thread`. That crosses workspace boundaries and reads private threads from unrelated projects.

Subagent transcripts are not separately readable. Use their reports in this thread. If `read_thread` fails, pass a tight digest of the session instead.

## 2. Spawn three reviewers in parallel

One message, three spawns.

Each reviewer and the synthesizer name a role line in `<pstack>/models.md`. Spawn each per the Subagents section of the **poteto-mode** skill. If `create_thread` rejects a mode, use `thread` and say so.

| Lens | Role line | Prompt template |
|---|---|---|
| Judgment | `reflect judgment, divergent, synthesizer` | `references/judgment-reviewer.md` |
| Tooling | `reflect tooling` | `references/tooling-reviewer.md` |
| Divergent | `reflect judgment, divergent, synthesizer` | `references/divergent-reviewer.md` |

Pass each template verbatim, substituting the thread ID or digest where marked. Reviewers return findings in their reports.

## 3. Synthesize

One spawn on the `reflect judgment, divergent, synthesizer` line. It spot-verifies citations through MCP. Pass `references/synthesizer.md` verbatim, with each reviewer's full output inlined where marked. It returns an Accepted / Rejected / Backlog list.

## 4. Structural enforcement check

Move any Accepted item that a lint rule, script, metadata flag, or runtime check would enforce more reliably to Backlog. See the **encode-lessons-in-structure** principle skill.

## 5. Apply

Present the synthesizer's full Accepted / Rejected / Backlog output and wait for explicit approval before applying any Accepted edit. The user picks the subset and may redirect routings. Skill changes affect every future agent in the org. Do not auto-apply.

File each Backlog item to your team's devex or backlog tracker without waiting. Only the Accepted list waits for approval.

Before approval, show each edit's destination. A pstack skill is edited in place under `<pstack>`. Any other skill is edited under `.agents/skills/<skill>/`, or as a personal skill via the `building-skills` skill. Never open a PR or push.

Follow each approved row's Routing exactly:

- Trivial existing-skill edit (a one-line bullet, a tightened sentence, a stale fact corrected): the parent does it directly.
- Substantive existing-skill edit (a new section, a new pattern table, more than ~10 lines): hand to Amp's `building-skills` skill.
- `tune description: <skill path>` (the skill exists but didn't trigger when it should have): hand to `building-skills` and follow its description guidance.
- `new skill via building-skills: <kebab-name>`: hand creation to `building-skills`. Do not invent the shape ad hoc.

If your environment ships a SKILL.md validator, run it on every touched skill before declaring done.

## 6. Summarize for the user

Short list, no preamble:

- Edits applied: `<skill path>`. What changed, one line each.
- New skills created: `<skill path>`. One line each (rare).
- Backlog filed to the devex tracker: `<issue title>` (`<tags>`). One line each.
- Dropped: one line per rejected finding + reason from the synthesizer.
