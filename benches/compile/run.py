"""Compare compilation of the working tree with a Git revision."""

import argparse
import os
from pathlib import Path
import shutil
import statistics
import subprocess
import tempfile
import time


def positive_integer(value):
    number = int(value)
    if number <= 0:
        raise argparse.ArgumentTypeError("must be greater than zero")
    return number


def command(args, **kwargs):
    return subprocess.run(args, check=True, capture_output=True, text=True, **kwargs)


def prepare(repo, directory, revision, source):
    macro = directory / "macro"
    macro.mkdir(parents=True)
    if revision:
        paths = command(
            ["git", "ls-tree", "-r", "--name-only", revision, "--", "Cargo.toml", "src"],
            cwd=repo,
        ).stdout.splitlines()
        for path in paths:
            destination = macro / path
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_text(
                command(["git", "show", f"{revision}:{path}"], cwd=repo).stdout
            )
    else:
        shutil.copy2(repo / "Cargo.toml", macro / "Cargo.toml")
        shutil.copytree(repo / "src", macro / "src")

    fixture = directory / "fixture"
    (fixture / "src").mkdir(parents=True)
    (fixture / "Cargo.toml").write_text(
        '[package]\nname = "compile-bench"\nversion = "0.0.0"\nedition = "2021"\n'
        '[dependencies]\nnum-lazy-proc = { path = "../macro" }\n'
        'num-traits = { version = "0.2.19", default-features = false, features = ["libm"] }\n'
    )
    # Start both variants from the same dependency versions.
    shutil.copy2(repo / "Cargo.lock", fixture / "Cargo.lock")
    (fixture / "src/lib.rs").write_text(source)
    return fixture


def check(fixture, source, iteration, env):
    # Force the fixture to compile again while retaining compiled dependencies.
    (fixture / "src/lib.rs").write_text(source + f"\n// iteration {iteration}\n")
    started = time.perf_counter()
    command(
        ["cargo", "check", "--offline", "--quiet", "--manifest-path", str(fixture / "Cargo.toml")],
        cwd=fixture,
        env=env,
    )
    return time.perf_counter() - started


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", default="HEAD", help="Git revision to compare (default: HEAD)")
    parser.add_argument("--runs", type=positive_integer, default=5)
    parser.add_argument("--functions", type=positive_integer, default=1000)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[2]
    revision = command(
        ["git", "rev-parse", "--verify", "--end-of-options", f"{args.baseline}^{{commit}}"],
        cwd=repo,
    ).stdout.strip()
    template = Path(__file__).with_name("workload.rs").read_text()
    imports, function = template.split("// The runner repeats this function with distinct names.\n")
    source = imports + "\n".join(
        function.replace("fn sample<", f"fn sample_{index}<")
        for index in range(args.functions)
    )

    print(f"{args.functions:,} functions; baseline {revision[:12]}; {args.runs} runs", flush=True)
    with tempfile.TemporaryDirectory(prefix="num-lazy-proc-compile-") as temporary:
        directory = Path(temporary)
        env = dict(os.environ, CARGO_TARGET_DIR=str(directory / "target"), CARGO_INCREMENTAL="0")
        fixtures = {
            "baseline": prepare(repo, directory / "baseline", revision, source),
            "current": prepare(repo, directory / "current", None, source),
        }
        for label, fixture in fixtures.items():
            check(fixture, source, -1, env)
            print(f"Warmed {label}", flush=True)

        results = {label: [] for label in fixtures}
        for iteration in range(args.runs):
            order = list(fixtures) if iteration % 2 == 0 else list(reversed(fixtures))
            for label in order:
                elapsed = check(fixtures[label], source, iteration, env)
                results[label].append(elapsed)
                print(f"{label} run {iteration + 1}: {elapsed:.3f}s", flush=True)

        medians = {label: statistics.median(values) for label, values in results.items()}
        change = (medians["current"] / medians["baseline"] - 1) * 100
        print(f'Median: baseline {medians["baseline"]:.3f}s; current {medians["current"]:.3f}s ({change:+.1f}%)')


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as error:
        raise SystemExit(error.stderr or str(error)) from error
