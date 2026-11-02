# Publishing the Unity Extension

This document is the release checklist for submitting the extension to the
[Zed extension registry](https://zed.dev/docs/extensions/publishing/overview).

## Current identity

| Field | Value |
|---|---|
| Extension ID | `unity` |
| Registry submodule | `extensions/unity` |
| Repository | `https://github.com/Dust-Games-Studio/Zed-Unity-Extension` |
| License | MIT, at the repository root |
| Version | `0.1.0` in `extension.toml` |

The ID `unity` is not used by another extension in `zed-industries/extensions`.
`unity-debugger` and `unity-snippets` are separate IDs.

## Before opening the registry PR

Run these from the repository root:

```bash
./scripts/validate.sh all
./scripts/check-publish.sh --strict
```

`validate.sh` checks the grammars, fixtures, language configs, and queries.
`check-publish.sh` checks the registry prerequisites: manifest fields, license,
grammar pins, language directories, tasks files, and the tracked file tree. It
also prints the exact `[unity]` entry for the registry PR.

Then complete the manual Zed check at the exact commit you will submit:

1. Run `zed: install dev extension` and select this repository.
2. Open each file in `tests/fixtures/` and confirm the language mode, highlighting,
   outline, comments, brackets, and indentation.
3. Confirm `.cs` still uses the official C# extension.
4. Run `task: spawn` from a Unity file and confirm all five Unity tasks appear.
5. Run `zed: open log` and confirm there are no grammar, query, or language
   registration errors.
6. Confirm `@error` is visible in your theme for the invalid UXML/USS fixtures.

Do not submit until `main` is clean, pushed, and the checked-out commit is on a
branch. The registry uses the checked-out submodule commit, and detached commits
are rejected.

### Snippets

`snippets/` is currently tracked but is not registered in `extension.toml`. The
snippet workstream can register it later. Before submission, either:

- register the snippets in `extension.toml`, or
- remove `snippets/` from the publishable commit so the submission contains only
  the language extension.

`check-publish.sh` prints a note while this directory is tracked and unregistered.
It does not fail the preflight, but reviewers may question unused resources.

## Submit to `zed-industries/extensions`

1. Fork `https://github.com/zed-industries/extensions` to a personal GitHub
   account (recommended by the Zed maintainers so they can push fixes to the PR).
2. Clone your fork and initialize its submodules:

   ```bash
   git clone https://github.com/YOUR-USERNAME/extensions
   cd extensions
   git submodule init
   git submodule update
   ```

3. Add this repository as a submodule using HTTPS, not SSH:

   ```bash
   git submodule add https://github.com/Dust-Games-Studio/Zed-Unity-Extension extensions/unity
   git add extensions/unity
   ```

4. Add this entry to the top-level `extensions.toml`:

   ```toml
   [unity]
   submodule = "extensions/unity"
   version = "0.1.0"
   ```

   The `version` must exactly match `version` in this repository's
   `extension.toml` at the submitted submodule commit.

5. Sort the registry files:

   ```bash
   pnpm sort-extensions
   ```

6. Commit, push your fork, and open a pull request against
   `zed-industries/extensions`. Follow the registry PR rules:

   - exactly one extension per PR;
   - at most three open PRs at a time;
   - respond to maintainer feedback within three weeks, or the PR is closed.

After the PR is merged, the extension is packaged and published to the registry.
Update the "pending" note in the README installation section at that point.

## Updating an already published extension

1. Bump `version` in `extension.toml` and commit the release to `main`.
2. Push `main` to the public repository.
3. In your `zed-industries/extensions` fork:

   ```bash
   git submodule update --remote extensions/unity
   ```

4. Update the `version` in the registry's `extensions.toml` to match the new
   `extension.toml`.
5. Run `pnpm sort-extensions`, commit, push, and open a PR.

Update PRs follow the same rules as the initial submission.

## Reference

- [Installing extensions](https://zed.dev/docs/extensions/installing-extensions)
- [Publishing overview](https://zed.dev/docs/extensions/publishing/overview)
- [Publishing prerequisites](https://zed.dev/docs/extensions/publishing/prerequisites)
- [Publishing guide](https://zed.dev/docs/extensions/publishing/publishing-guide)
- [Updating an extension](https://zed.dev/docs/extensions/publishing/updating-and-maintenance)
- [License requirements](https://zed.dev/docs/extensions/publishing/license-requirements)
- [Publishing FAQ](https://zed.dev/docs/extensions/publishing/faq)
