# AGENTS.md

This file gives the rules for work in this project. Read this file before you change code.

## Project purpose

**twigz** is a grammar compiler, scanner generator, and semantic-query library.

The tagline is **Author a grammar. Query any language the same way.** Authors
write `.grammar` files. The compiler emits parsers and scanners. Callers ask
language-neutral questions of the tree.

Do not add a dependency on agent-os. Do not import
community `tree-sitter-*` crates. Do not enable Tree-sitter’s JavaScript grammar
path. Do not write first-party scanners as C files.

## Repository structure

This project uses a bare git repository and worktrees.

```
twigz.git/                bare repository (shared object store, no working tree)
twigz-master/             worktree: master
twigz-develop/            worktree: develop
twigz-<name>/             worktree: feature or hotfix branch (temporary)
```

### Layout rules

- Never work inside the bare repository directory. It has no working tree.
- Each worktree checks out exactly one branch. Two worktrees must not share one branch.

## Tree-sitter pin

Pin Tree-sitter in [`TREE_SITTER_PIN.md`](TREE_SITTER_PIN.md). Do not float on
an untracked tip. Record `ABI_VERSION_MAX` from the fetched crate.

## Build system

Build and test only with **Bazel**, **rules_rust**, and **rules_zig** zig cc.
Do not use the system `CC`. Do not use `cargo test` as the gate.

### Bazel output root

Do not commit a machine-specific output path.

Set the output root in ignored `user.bazelrc`. Use an absolute path. Do not
leave Bazel output under `/tmp` or `~/.cache/bazel`.

```bazelrc
startup --output_user_root=/mnt/workspace/opytai/twigz/bazel-cache
```

The tracked `.bazelrc` imports `user.bazelrc` automatically. Do not add the
startup option to each command. Do not share this output root with other
projects.

Examples:

```bash
bazel build //...
bazel test //...
bazel shutdown
```

Do not delete or expunge a shared local cache unless the user requests it.

## Testing layout

Full rules: **`docs/TESTING.md`**.

Short form:

| Kind | Location |
|------|----------|
| Unit | Co-located `*_test.rs` next to the crate |
| IR / semantics goldens | `data/goldens/{ir,semantics,snapshot}` |
| Source fixtures | `data/fixtures/source/<language>/` |

Production `rust_library` targets must **not** depend on test-only packages.

## Common operations

### Add a worktree

```bash
cd twigz.git
git worktree add ../twigz-<name> <branch>

# Or create a new branch at the same time:
git worktree add ../twigz-<name> -b <new-branch> <start-point>
```

Feature branches start at `develop`. Hotfix branches start at `master`.

### List worktrees

```bash
cd twigz.git
git worktree list
```

### Remove a worktree

```bash
cd twigz.git
git worktree remove ../twigz-<name>
```

### Fetch and pull

```bash
cd twigz.git
git fetch --all
```

Then pull inside a specific worktree:

```bash
cd ../twigz-master
git pull
```

### Prune stale worktree references

If a worktree directory was deleted by hand:

```bash
cd twigz.git
git worktree prune
```

## Branch naming schema

Branches follow a tiered promotion model. Code flows **upward** through each
tier by merge. Never skip a tier.

```
master                    production: tagged releases only
  ↑ merge
develop                   integration: completed feature work lands here first
  ↑ merge
feature/*                 feature work
hotfix/*                  urgent production fixes (from master; merge to master AND develop)
```

### Branch prefixes

| Prefix | Branches from | Merges into | Purpose |
|---|---|---|---|
| `feature/<name>` | `develop` | `develop` | Feature work |
| `hotfix/<name>` | `master` | `master` + `develop` | Urgent production fixes |
| `develop` | — | `master` | Integration branch |
| `master` | — | — | Production |

Use lowercase kebab-case.

```
feature/extract-twigz
hotfix/scanner-serialize
```

### Release tag names

Every release tag must have a human codename. Use an Ubuntu-style
alphabetized pair: `{funny adjective} {animal name}`.

The adjective and the animal must start with the same letter. Advance
alphabetically across releases. You may reuse Ubuntu animal names. Do not
reuse Ubuntu adjectives.

Never choose or apply the tag name alone. First propose a small set of
candidate names. Then wait for the user to select one.

## Concurrent work

- Parallel tasks must own disjoint files.
- Run changes to shared files sequentially.
- Use isolated worktrees for concurrent write tasks.
- Never let two writers share a dirty worktree.

### Git operations

- **Subagents / worker agents must not run git.** No `git add`, `commit`,
  `checkout`, `restore`, `reset`, `switch`, `merge`, `rebase`, `push`,
  `pull`, `clean`, or `worktree` from a worker.
- Only the **orchestrator** (main agent, with the human’s request) may run
  git, and only for the requested operation.
- Workers that “clean up” with `git restore` / `git checkout --` destroy
  other agents’ uncommitted work. That is forbidden.

## Progressive merging workflow

### Feature work to production

```bash
cd twigz.git
git worktree add ../twigz-my-feature -b feature/my-feature develop

cd ../twigz-my-feature
# commit, iterate, run Bazel checks

cd ../twigz-develop
git merge feature/my-feature

cd ../twigz.git
git worktree remove ../twigz-my-feature
git branch -d feature/my-feature

cd ../twigz-master
git merge develop
git tag -a v1.2.0 -m "Release 1.2.0"
```

### Hotfix workflow

Hotfixes branch from master. Merge them into **master** and **develop**.

```bash
cd twigz.git
git worktree add ../twigz-hotfix-xyz -b hotfix/xyz master

cd ../twigz-hotfix-xyz
# fix, commit, run Bazel checks

cd ../twigz-master
git merge hotfix/xyz
git tag -a v1.1.1 -m "Hotfix 1.1.1"

cd ../twigz-develop
git merge hotfix/xyz

cd ../twigz.git
git worktree remove ../twigz-hotfix-xyz
git branch -d hotfix/xyz
```

### Merge direction rules

- Always merge upward: `feature → develop → master`.
- Never merge downward unless you complete a hotfix path.
- Never skip tiers. Do not merge a feature directly into master.
- Always merge from inside the **target** worktree. Change directory into
  the branch that receives the merge. Then run `git merge <source>`.
- Never merge from inside the bare repository. There is no working tree for
  conflict resolution.

## Rules for AI agents

- When the user names a branch, work inside the matching worktree.
- Do not run `git checkout` or `git switch` in a worktree to a branch that
  another worktree already has checked out. That command fails.
- If a worktree for the needed branch does not exist, create it with
  `git worktree add`.
- Run git metadata commands (`log`, `status`, `diff`, `fetch`) from the
  relevant worktree so context is correct.
- Follow the progressive merge order: feature → develop → master. Never skip
  tiers.
- Create new feature branches from `develop`, not from `master`.
- Always merge from inside the **target** worktree.
- After you merge a feature, remove its worktree and delete the branch.
- Keep the long-lived worktrees (`twigz-master`, `twigz-develop`) present.
- Always build and test with Bazel. Honor the local `user.bazelrc` when it
  exists.
- Do not use system `CC` or `cargo test` as a substitute for Bazel checks.
- Do not treat markdown text as a substitute for failing Bazel checks.

## Writing style for new agent docs

Write new project procedure docs in **ASD-STE100** style when practical:

- Use short sentences.
- Use active voice.
- Use simple present tense for descriptions.
- Use imperative mood for procedures.
- Put one main idea in each sentence.
- Use the same technical term for the same thing every time.
- Prefer concrete steps over abstract policy essays.
