---
name: setup-pstack
description: Configure which model family pstack uses per role. Checks the user's subscriptions and rewrites pstack's models.md. Use for /setup-pstack, "configure pstack models", or changing pstack's model choices.
---

# Setup pstack

Rewrite `../../models.md`, the file that sets pstack's model per role.

## Steps

### 1. Check the families

The values are `thread`, `claude`, and `openai`. `models.md` defines the agent mode behind each family. Run `manage_amp` with topic `model_providers` and operation `list` to confirm the user still has a Claude subscription, which serves `claude` through `external-agent:claude-code`, and a ChatGPT subscription, which serves `openai`. If one is gone, say so and offer `thread` for roles that used it. To change a family's model, such as a newer Claude or GPT, edit its definition at the top of `models.md`. Check an OpenAI key with `amp.list_agent_modes`.

### 2. Load current state

Read `../../models.md`. Its role lines are the current choices. A line whose role is not in the block in step 4 is from a retired role. Drop it.

### 3. Show the roles and confirm

Show every role with its value, and mark any value whose subscription is missing as needing a choice. Also list each line step 2 dropped. Ask with `ask_user_choice` whether to accept as-is or change specific roles. Explain the tradeoff once. `thread` is the cheapest and sees uncommitted work. A family other than the thread's starts an orb thread, which bills to Amp credits, sees only pushed work, and is the only way to get a second model family. For panel roles (`arena runners`, `architect runners`, `interrogate reviewers`) the value is a list, and one seat runs per entry, so the list length sets the count. `arena cross-judge pool` is also a list, but Arena runs one judge. It tries entries from a family other than the parent's, in list order, so order it by preference. `thread` never judges. `swarm workers` is the default for every worker unless a race or comparison assigns another mode per arm.

### 4. Write the file

Overwrite `../../models.md` with everything above the role block unchanged, except a family definition the user changed, then the role block with the chosen values. Keep this role order:

```
feature, refactoring
bug-fix
perf-issue
hillclimb
judgment and prose
hardest tasks
how explorer
how explainer
why investigators
why synthesizer
reflect tooling
reflect judgment, divergent, synthesizer
arena runners
arena cross-judge pool
swarm workers
architect runners
interrogate reviewers
```

### 5. Confirm

Tell the user the file changed and that it takes effect the next time a skill reads it. Commit it if the user wants the choices shared with everyone who uses this repository.

### 6. Offer a verification skill (optional)

Check whether the project has a way to drive the real app for proof (a `verify-*` skill, or an existing harness). If not, offer once: "want a project-local verification skill, so agents can drive the app the way a user does and prove changes work? I can generate one with /create-verification-skill." On yes, load the `create-verification-skill` skill. On no, move on without pushing.

### 7. Offer a formal verification skill (optional)

Check whether the project already has a formal verification skill (a project skill that runs a model checker, a proof, or another verifier against the code). If not, offer once: "want a formal verification skill, so agents can prove a key property of your code holds in every case in scope, not only the cases tests reach? I can generate one with /create-formal-verification-skill." On yes, load the `create-formal-verification-skill` skill. On no, move on without pushing.
