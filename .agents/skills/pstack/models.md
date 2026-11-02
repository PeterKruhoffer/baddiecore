# pstack model configuration

One line per role, as `role: value`. The `setup-pstack` skill rewrites this file. Delete a line to fall back to the skill's default.

This repository uses two model families, both on subscriptions:

- `claude` is `external-agent:claude-code` with `model:claude-opus-5-5,effort:xhigh`. It bills to the Claude subscription. Amp's other modes never do.
- `openai` is `openai/gpt-6-astra`. It bills to the ChatGPT subscription.

Values:

- `thread` runs the role as a `Task` subagent on this thread's model. It sees this workspace, including uncommitted work.
- `claude` or `openai` runs the role on that family. When it matches this thread's family, run it as `thread` instead. Otherwise it runs in its own orb thread, started with `create_thread`. See the Subagents section of the **poteto-mode** skill. Orbs bill to Amp credits, not to either subscription.
- A panel role (`arena runners`, `architect runners`, `interrogate reviewers`) takes a list. One seat runs per entry, so the list length sets the panel size.

```
feature, refactoring: thread
bug-fix: thread
perf-issue: thread
hillclimb: thread
judgment and prose: thread
hardest tasks: thread
how explorer: thread
how explainer: thread
why investigators: thread
why synthesizer: thread
reflect tooling: thread
reflect judgment, divergent, synthesizer: thread
arena runners: claude, openai
arena cross-judge pool: claude, openai
swarm workers: thread
architect runners: claude, openai
interrogate reviewers: claude, openai
```
