# Compilation benchmark

Run from the repository root with Python 3, Cargo, and Git installed:

```sh
cargo fetch --locked
python3 benches/compile/run.py
```

The runner compares the working tree with `HEAD`. Choose another baseline or workload size with:

```sh
python3 benches/compile/run.py --baseline v0.2.0 --runs 7 --functions 2000
```

`workload.rs` contains a generic function with eight stable constants and a floating-point literal. The runner generates 1,000 copies by default. Both variants use the same workload and start from the working tree's dependency lockfile. Use a baseline with compatible dependencies and macro syntax.

Each variant is warmed before timing. Subsequent runs alternate their order and force the generated crate to compile again, retaining compiled dependencies. Incremental compilation is disabled. The reported median measures `cargo check`, including macro expansion and type checking; it excludes dependency warm-up and measures no executable code generation. Small timing differences may reflect system noise.

The runner works offline after dependencies have been fetched. It builds in a temporary directory that is removed on exit and leaves the repository's build artifacts untouched. Edit `workload.rs` to exercise other syntax, keeping imports above the marked function boundary and retaining the function name `sample`.
