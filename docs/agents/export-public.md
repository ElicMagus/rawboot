# Exporting the public repository

The internal repository (`rawboot-internal`) carries agent instructions, research
notes and tracker tooling. The public GitHub repository
(`https://github.com/ElicMagus/rawboot`) is a clean export: only code,
documentation and the build tooling, squashed into a single commit.

`scripts/export-public.sh` builds that tree. It never performs network
operations; the push is the owner's step.

## What the export contains

The script copies every tracked file except this denylist:

- `AGENTS.md`
- `CODING_STANDARDS.md`
- `docs/agents/`
- `research/`
- `.gitea/`
- `scripts/issue-view.sh`
- `scripts/export-public.sh`
- `rawboot/123.txt` and `rawboot/build.bat` (scratch and a pre-port Windows helper)

It then fails if the tree contains any internal or personal string, or if a
`.rs`/`.sh` file lacks the `CC-BY-4.0` SPDX header. On success it creates one
commit authored and committed by
`ElicMagus <60005217+ElicMagus@users.noreply.github.com>`, runs the exported
tree's `scripts/check.sh`, and prints the push command without running it.

## Runbook

1. Ensure the working tree is clean and the licence/README work is committed.
   The script copies only tracked files.
2. Build and verify the export:

   ```sh
   scripts/export-public.sh
   ```

   The default output is `target/public-repo`; use `--output <dir>` for another
   location. A second run rebuilds the same tree from scratch, so the result is
   idempotent.
3. Check the single commit and the clean history:

   ```sh
   git -C target/public-repo log --format='%H %an <%ae> %cn <%ce> %s'
   git -C target/public-repo rev-list --count HEAD  # must print 1
   ```

4. Run the full public check from that tree:

   ```sh
   (cd target/public-repo && ./scripts/check.sh)
   ```

5. Owner step: create an empty `ElicMagus/rawboot` repository on GitHub (no
   README, no `.gitignore`), then add a Deploy Key with write access (or use an
   existing credential). Do not commit a token.
6. Push from the exported tree:

   ```sh
   cd target/public-repo
   git remote add origin https://github.com/ElicMagus/rawboot.git
   git push -u origin main
   ```

7. Confirm that GitHub shows exactly one commit, the three README files and the
   CC-BY-4.0 `LICENSE`, and that the CI workflow passes. The public
   `.github/workflows/ci.yml` mirrors the internal `.gitea/workflows/ci.yml`;
   update both together.

Do not export the internal history and do not force-push over the public
repository from the internal checkout.
