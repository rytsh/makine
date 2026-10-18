<!--
seviyeler:
-->
# Signal watching: how a circuit behaves over time

Seeing that a wire is 1 right now doesn't always explain why it's 1.
Especially with registers, **which clock edge** it changed on matters.
That's why the signal watcher keeps the history, not just the latest value.

## Watching a signal

Right-click a wire and choose **Watch signal**. Or select a part and click a
pin in the **Watch outputs** section. You can watch an output even if it isn't
connected.

- A single bit is drawn as a stepped 0/1 line.
- For a bus, value ranges are shown in hex; it is not an analog voltage graph.
- **Show values in binary** lets you read the individual bits of the current
  value.
- **?** means the simulation couldn't read that signal. A deleted part or a
  circuit that isn't running can cause this.

You can watch up to 16 signals, each with its last 256 simulation steps. Even
at high speed, every step is sampled, not just the values on frames drawn to
the screen. While paused, the same step isn't added again and again.

## A step and a clock cycle are not the same thing

If the clock's period is 3, its output is 0 for three steps and 1 for three
steps. So **Step** doesn't always run one instruction.

- **Step:** moves simulation time forward by one.
- **Clock cycle:** pauses, then runs until the next rising edge of the first
  clock. With very long periods, this can spread over several screen frames.
- **Back to start / Restart:** resets the clock counter and the simulator's
  internal state. It doesn't erase the circuit drawing or the ROM program.

If there are several independent clocks, the cycle button doesn't turn all of
them into a single "processor instruction." You need to know which clock is in
charge. The 4004 workshop has a single clock; one rising edge runs one
instruction. This is a simplified version of the real 4004's historical
eight-phase timing.

## Stop on change

If you choose **Stop on change** on a watched signal, the simulation stops
when the value changes between two valid samples. For example, you can catch
the moment the carry bit becomes 1 for the first time. This is a signal-change
trigger, not a breakpoint on an instruction address. The first sample or a
**?** doesn't trigger it on its own.

## Experiment: the rising edge

1. Place an **Input**; set its bit count to 4 and its value to 5.
2. Place a **Register**; set its bit count to 4.
3. Connect the input to `d`, a switched-on Switch to `load`, and the Clock to
   `clk`.
4. Watch the `q` output. Stop the clock and press **Clock cycle**: q becomes 5.
5. Change the input to 9. See that q is still 5 while the clock hasn't moved.
6. On the next rising edge, q becomes 9.

This experiment is the behavior that the [4-bit Register](atolye.md) exercise
measures.
