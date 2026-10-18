<!--
seviyeler: Latch, DLatch, DFF, Bit
-->
# Memory

The circuits you've made so far had no memory: the output depended only on
the **current** inputs. A computer, though, has to **remember**: numbers, the
program, where it left off.

The secret of memory is **feedback**: connecting a circuit's output back to
its own input.

## SR latch

Cross-connect two NANDs: the output of each goes to an input of the other.

![SR latch](img/sr-mandal.png)

This circuit has **two stable states**: `q = 1` or `q = 0`. Once it enters a
state, it **stays** there while the inputs are at rest:

- `set` → q becomes 1
- `reset` → q becomes 0
- both 0 → q **keeps its old value**

> [!NOTE]
> In the game's level, the `set` and `reset` inputs take effect when they
> are 1. That's why you put a NOT in front of each of the two NANDs. In the
> diagram these NOTs are shown as "NOT set" and "NOT reset".

The simulator notices this loop and recalculates it again and again until the
circuit settles. If you build a loop that never stops flipping (e.g. a NOT
wired to its own output), you'll get a "circuit is oscillating" warning.

## D latch

The SR latch has a problem: giving `set` and `reset` at the same time makes
no sense. The **D latch** fixes this. It has a single data input `d` and an
enable input `en`:

- `en = 1`: q follows d (the latch is "transparent")
- `en = 0`: q holds its last value

```
set   = d AND en
reset = NOT d AND en
```

## The clock and the D flip-flop

In a processor, everything moves **step by step**. These steps are set by a
**clock** signal: a wire that keeps going 0-1-0-1. The clock of the processor
in your phone ticks billions of times per second (GHz).

The **D flip-flop** stores d only at the **moment the clock goes from 0 to 1**
(the rising edge):

![Clock and rising edge](img/saat.png)

Why isn't a D latch enough? While the clock is 1, the latch is transparent. q
changes, the new d computed from q passes through the latch, and that changes
q again… A counter would count many times while the clock is 1! A flip-flop
stores **exactly once** per edge.

### Master-slave

Put two D latches one after the other:

```
master = DLatch(d,      NOT clk)   // open while clock is 0
slave  = DLatch(master, clk)       // open while clock is 1
q = slave
```

- Clock 0: the master follows d, the slave is closed and holds q.
- Clock 0→1: the master closes and holds the last d. The slave opens and
  passes it to q.
- Clock 1: the master is closed, so q doesn't change no matter what d does.

Two gates working together: one doesn't open until the other has closed.
Like a canal lock.

## 1-bit register

A flip-flop stores d on every edge. But sometimes we want to say "**don't
change** this time." For that, put a MUX in front of the flip-flop's input:

```
d' = MUX(q, d, load)    // load=0: its own value, load=1: new value
```

While `load = 0`, the flip-flop stores **its own output** again on every edge,
so the value doesn't change.

Next: [Registers and RAM](register.md).
