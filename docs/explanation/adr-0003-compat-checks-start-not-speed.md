ADR 0003: the compatibility matrix checks that nihilurk starts, not that it is fast
===================================================================================

    Status         Accepted
    Audience       Anyone changing `compat/`, tempted to add a load test
                   to it, or wondering why the matrix grades no machine
                   on speed.
    Supersedes     --
    Related        cross-platform-testing.md

Every machine in the matrix is asked two things: does its image have a shell, and does `nihilurk -content` run on it. Nothing asks how fast.


Context
-------

`gdd.md` claims nihilurk runs on anything with a terminal. The matrix exists to make that claim fail loudly.

The first version of the matrix checked it under load. It drove the game inside a resource-capped container: a real dungeon floor for the gate, Bad Apple at eight hundred motes a frame for a ceiling, and a pty-attached run to prove the terminal stack actually draws. The game-load and reel runs sat behind an opt-in `--exec-emulated` path, because they made ioctl calls a qemu-user-static translation layer could get wrong. The rig was borrowed wholesale from a stress rig built to find performance cliffs on a workstation.

It worked. Bad Apple ran through the particle system with no trouble on the maintainer's workstation. The rig was added in `6fdb716` and removed in `e18db33`, and `git show 8efa8ec:perf/bad-apple` still has it. The video file is why a clone weighs what it does: 10.7 MB raw, about 685 KiB packed, and the largest object in a 3.4 MiB repository.


Decision
--------

**Check that the target builds, that its image has a shell, and that `nihilurk -content` runs. Do not check under load.**

  1. `cross_build.sh` cross-compiles a full Rust toolchain for the target. That is a heavier job, by time, memory and code paths exercised, than nihilurk's own frame loop will ever ask of the machine. A machine that can do it can run nihilurk.
  2. The two things the build cannot prove are the shell and the start. `run_check.sh` checks exactly those, with `nihilurk -content`: a headless print-and-exit that never touches the terminal.
  3. No CPU cap, no memory cap, no pty sizing, no `--exec-emulated`.

Rejected:

  4. **A stress rig for performance cliffs.** nihilurk has no workload that asks a machine to be fast. Every feel-layer effect is bounded: a dozen sparks when something dies, not a music video. A Pi that cannot keep up with a synthetic ceiling might still play nihilurk at a comfortable thirty frames a second, so grading it on that ceiling answers a question nobody asked. It also costs CPU caps, memory caps, qemu interpreter plumbing and pty sizing to keep working.


Consequences
------------

Accepted costs:

  * **No performance claim.** A machine can pass the matrix and still play badly. The size of the shipped binary is on the report per target, and that is the only number it gives.
  * **No proof that the terminal stack draws on a target.** The check avoids the terminal on purpose.

Benefits realised:

  * The check needs nothing from the environment beyond a shell, so it runs unattended under qemu-user-static.
  * The video file and its rig are out of the tree.
  * One verdict per row (`runs`, `no shell`, `does not run`, `-`), not a performance band, so there is one answer to defend.


When to revisit
---------------

Reopen this decision if any of these becomes true:

  * **A feel-layer effect becomes unbounded.** Something that scales with input, not with a fixed budget.
  * **A real workload appears.** Large floors, many creatures, anything that asks a machine to be fast.
  * **A player reports a slow machine the matrix passed.**


See also
--------

  cross-platform-testing.md                 the matrix as it is built
  ../how-to/run-the-compat-pipeline.md      running it
  the-feel-layer.md                         why every effect is bounded
