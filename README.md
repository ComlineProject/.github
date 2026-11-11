# ComlineProject/.github

Organization-wide defaults and shared CI for the ComlineProject repositories.

- [`SECURITY.md`](SECURITY.md): the default security policy, shown on every
  repository that has none of its own.
- [`renovate-config.json`](renovate-config.json): the shared Renovate preset.
- [`.github/workflows/`](.github/workflows): reusable workflows
  ([`rust-ci.yml`](#rust-ciyml), [`bench.yml`](#benchyml),
  [`fuzz.yml`](#fuzzyml)).

## Renovate

In a repository's `renovate.json`:

```json
{
  "$schema": "https://docs.renovatebot.com/renovate-schema.json",
  "extends": ["github>ComlineProject/.github:renovate-config"]
}
```

The preset runs weekly (before 6am on Monday, UTC) and opens at most five pull
requests at a time. It pins GitHub Actions and Docker images by digest, and
groups what it can: all GitHub Actions in one pull request, the
`ghcr.io/comlineproject/ci-rust` digest in another (never automerged), crates.io
and npm dependencies in one each, updating the lockfile only. Vulnerability
alerts are on and labelled `security`. Git dependencies in `Cargo.toml` (the
cross-repo `rev =` pins) are not updated: bump them by hand.

A workflow that passes the CI image to `bench.yml` or `fuzz.yml` as
`image: ghcr.io/comlineproject/ci-rust:<tag>@sha256:<digest>` is found by the
preset's regex manager, so its digest is bumped too.

## Calling the reusable workflows

A caller references them by commit, not by branch or tag:

```yaml
uses: ComlineProject/.github/.github/workflows/rust-ci.yml@<full commit sha> # <this repo's tag or date>
```

Each workflow asks for `contents: read` at the top and `timeout-minutes` on
every job. Concurrency belongs to the caller: none of them sets any, so put a
`concurrency:` group on the caller's workflow or job. A called workflow cannot
hold more permissions than the caller's job gives it; each section below says
what to grant. Required status checks are named after the caller's job and the
called job (`ci / test (stable)`), so they change when a repository moves to
these workflows.

### rust-ci.yml

Builds and tests a Cargo project: a toolchain matrix on `ubuntu-24.04`, stable
on `ubuntu-24.04-arm`, `macos-15` and `windows-2025`, and stable on
`ubuntu-26.04` as a canary that may fail without failing the run. Toolchains
are installed with `dtolnay/rust-toolchain` and exported as `RUSTUP_TOOLCHAIN`,
so they win over a `rust-toolchain.toml`. Jobs that are special to one
repository (other targets, generated-file checks, harnesses) stay in its own
workflow, next to the call. Every job's first step is `step-security/harden-runner`
in audit mode (it supports Linux, macOS and Windows hosted runners; audit only
on the last two).

| Input | Type | Default | Meaning |
| --- | --- | --- | --- |
| `toolchains` | string (JSON) | `["stable","beta","nightly"]` | Toolchains of the matrix job. |
| `features` | string | empty | Features for one more `cargo test --features` run; empty for none. |
| `features-package` | string | empty | `-p <package>` for that run; empty for the root package. |
| `build-args` | string | empty | Extra arguments for every `cargo build` (e.g. `--locked`). |
| `test-args` | string | empty | Extra arguments for every `cargo test` (e.g. `--locked`). |
| `targets` | string | empty | Comma-separated extra rustup targets to install. |
| `cross-platform` | boolean | `true` | Run the other-OS jobs. |
| `canary` | boolean | `true` | Run the `ubuntu-26.04` canary job. |
| `working-directory` | string | `.` | Where the Cargo workspace is. |
| `extra-setup` | string | empty | A bash script run in every job before the build. |

```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:
permissions:
  contents: read
concurrency:
  group: ci-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: true
jobs:
  ci:
    uses: ComlineProject/.github/.github/workflows/rust-ci.yml@<sha>
    with:
      features: tokio
      features-package: comline-runtime
      test-args: --locked
```

### bench.yml

The instruction-count gate (iai-callgrind). On a pull request it benches the
base and then the head in one job, on one machine, and fails when a benchmark's
count grows by more than `limit-percent` percent unless the repository's ledger
accepts it. The job runs in the CI image, so nothing is installed: the image
needs the toolchain, valgrind, `iai-callgrind-runner`, bash, git and python3.
The repository keeps its own report script and ledger; the script is called as
before: `--targets` lists `<package> <bench>` per line, and the gate passes
`--home --commit --toolchain --limit --pr --base-home --ledger --base-ledger
--report --markdown` (plus `--criterion` and `--run` with `wall-time-command`).

Run it from a plain `pull_request` trigger, never `pull_request_target`: a
fork's pull request then has a read-only token and no secrets. The gate job asks
for `contents: read` and nothing else.

With `publish`, a push also benches the head and commits the report to the
`reports` branch with the Observatory's `publish-report` action. The caller's
job then needs `permissions: contents: write`; only the publish job uses it, and
never on a pull request.

| Input | Type | Default | Meaning |
| --- | --- | --- | --- |
| `image` | string | required | The CI image, with its digest. |
| `ledger` | string | required | The ledger of accepted regressions, from the repository root. |
| `report-path` | string | required | Where the script writes the report, from the repository root. Its file name is the artifact's name. |
| `bench-command` | string | `cargo bench` | Run as `<bench-command> -p <package> --bench <bench> -- <iai args>`. |
| `working-directory` | string | `.` | Where the Cargo workspace is. |
| `container-options` | string | `--security-opt seccomp=unconfined` | Docker options of the job's container. iai-callgrind's runner calls `setarch -R`, which Docker's default seccomp profile refuses. |
| `report-script` | string | `.github/scripts/bench-report.py` | The report script, from the repository root. |
| `limit-percent` | string | `1` | The most a count may grow, in percent. |
| `wall-time-command` | string | empty | A Criterion run on the head, not gated. |
| `publish` | boolean | `false` | Add the publish job (not on a pull request). |
| `default-branch` | string | `main` | The branch published from; a run from another one does a dry run. |
| `reports-readme` | string | `.github/reports-branch.md` | The reports branch's README. |

```yaml
name: Benchmarks
on:
  pull_request:
  push:
    branches: [main]
permissions:
  contents: write # for `publish`; the gate job reads only
concurrency:
  group: bench-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: true
jobs:
  bench:
    uses: ComlineProject/.github/.github/workflows/bench.yml@<sha>
    with:
      image: ghcr.io/comlineproject/ci-rust:<tag>@sha256:<digest>
      ledger: runtime/core/benches/accepted-regressions.toml
      report-path: runtime/conformance/reports/rust.bench.json
      publish: true
```

### fuzz.yml

cargo-fuzz, one job per target, in the CI image (nightly toolchain, cargo-fuzz,
bash, git, `gh` and python3). A pull request runs each target for
`pr-duration` seconds. Any other run (the nightly, a manual one) runs each for
`duration` seconds on every core, from a corpus cached between runs, and opens
an issue on a failure (one per target while one is open). The fuzz job holds
`issues: write` and nothing else does. A report is combined from the targets'
results with the Observatory's `report-fuzz` action, uploaded and summarized;
with `publish`, a run on the default branch commits it to the `reports` branch.

The caller's job needs `contents: read` and `issues: write`, and `contents:
write` with `publish`. The targets come from the caller, so a repository that
keeps them in one list computes them in a job of its own and passes the output
(`targets: ${{ needs.targets.outputs.list }}`). A caller's own jobs (property
tests) stay in its file; ones that upload a `fuzz-stats-*` artifact are combined
into the report when the fuzz job `needs` them.

| Input | Type | Default | Meaning |
| --- | --- | --- | --- |
| `image` | string | required | The CI image, with its digest. |
| `targets` | string (JSON) | required | An array of target names, or of `{"target": ..., "dict": ...}` objects (`dict` is a file in `fuzz-dir`). |
| `fuzz-dir` | string | required | The cargo-fuzz directory, from the repository root. |
| `report-path` | string | required | Where the report is written, from the repository root. |
| `duration` | number | `600` | Seconds per target when not a pull request. |
| `pr-duration` | number | `60` | Seconds per target on a pull request. |
| `working-directory` | string | `.` | Where cargo-fuzz is run from. |
| `libfuzzer-args` | string | empty | Extra libFuzzer arguments (e.g. `-max_len=1024`). |
| `seed-script` | string | `<fuzz-dir>/seed-corpus.sh` | A `sh` script that seeds the corpus, if it exists. |
| `crash-format` | string | `text` | `text` or `hex`, for the crash in the issue. |
| `report-subject` | string | empty | The report's `runtime` field. |
| `publish` | boolean | `false` | Publish the report from the default branch. |
| `default-branch` | string | `main` | The branch published from. |
| `reports-readme` | string | `.github/reports-branch.md` | The reports branch's README. |

The job timeout is 30 minutes plus the longer of the two durations.

```yaml
name: Fuzz
on:
  pull_request:
    paths: ["core/**", "Cargo.toml"]
  schedule:
    - cron: "47 3 * * *"
  workflow_dispatch:
permissions:
  contents: read
jobs:
  fuzz:
    uses: ComlineProject/.github/.github/workflows/fuzz.yml@<sha>
    permissions:
      contents: write
      issues: write
    with:
      image: ghcr.io/comlineproject/ci-rust:<tag>@sha256:<digest>
      fuzz-dir: core/fuzz
      targets: '[{"target":"ids_parse","dict":"ids.dict"},{"target":"unverified_oracle"}]'
      libfuzzer-args: -max_len=1024
      report-path: reports/core.fuzz.json
      publish: true
```

## Checks

`lint.yml` runs actionlint on every workflow here and validates
`renovate-config.json`. `selftest.yml` calls `rust-ci.yml` on the fixture crate
in `selftest/`. `bench.yml` and `fuzz.yml` need the CI image and a repository's
own targets, so only actionlint checks them here.

### lint.yml

`lint.yml` is also reusable: a repository calls it to run actionlint on its own
workflows, and optionally to validate a Renovate config.

| Input | Type | Default | Meaning |
| --- | --- | --- | --- |
| `renovate-config` | string | empty | A Renovate config to validate with `--strict`, from the repository root; empty skips the check. |

```yaml
jobs:
  lint:
    uses: ComlineProject/.github/.github/workflows/lint.yml@<sha>
    with:
      renovate-config: renovate.json
```
