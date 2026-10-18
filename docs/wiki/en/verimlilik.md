<!--
seviyeler:
-->
# Efficiency: the same job with fewer gates

Passing a circuit's test shows that it is **correct**. Another circuit that
does the same job might be built with fewer gates. In a real chip, every gate
takes up space, uses energy, and costs money. That's why engineers work hard
to shrink a circuit that already works.

## The numbers in the level panel

Every level has an **Efficiency** section in its right panel:

- **Your circuit:** how many NAND gates your circuit comes to when every chip
  inside it is opened up. Even with Fast simulation on, the count is in NANDs.
- **Example solution:** the smallest solution the game knows for this level.
- **Known minimum:** for small levels, it's known that fewer is impossible
  (e.g. 4 NANDs for XOR). For big circuits no such exact limit is known; there,
  the example solution is the yardstick.

If your count matches the target you get a ✔; if it's smaller than the
example a message highlights it: you've found something better than the game knows.

## Where do extra gates come from?

An 8-bit adder is made of eight full adders. If your full adder is 15 gates,
the adder comes to 120 gates; if it's 9, it's 72. So the extra gates might
not belong to this level, but to **an earlier chip you used inside it**. The
panel tells you this separately:

> 48 of these gates come from earlier chips you used inside it.

Then go back to the earlier level and shrink the chip there. When your saved
solution changes, every circuit that uses it gets smaller too.

## Three ways to save gates

**1. Share intermediate results.** A gate's output can go to as many places as
you want. In XOR, `NAND(a, b)` is used in two different places. In a half
adder, the inverse of that same NAND gives the carry; you don't need a
separate AND.

**2. Remove double inversions.** AND = NAND + NOT, OR = NOT + NOT + NAND.
If an AND's output goes into an OR, the NOT at the end of the AND and the NOT
at the start of the OR cancel each other out. A MUX built from AND/OR chips
costs 8 gates; built directly from NANDs, it costs 4.

**3. Use a signal you already have.** In the D latch, instead of adding a NOT
for `NOT d`, you can use the output of `NAND(d, en)`: when en = 1, that's
already `NOT d`.

## Are fewer gates always faster?

No. How fast a circuit runs depends on the number of gates on the **longest
path** from input to output. A ripple-carry adder has few gates, but the carry
moves one step at a time through 32 digits. Real processors spend more gates
to compute the carry ahead of time (carry-lookahead) and add faster. There's a
**trade-off** between gate count and speed.

In this game we measure gate count, because it's easy to count and it shows
well how roundabout your design is.

## Seeing the example solution

The **💡 See the solution** button opens a separate **Example** tab.
Switch between your named circuit and **Example** in one click. Your own layout, zoom,
inputs and undo history stay intact. Experiment with the example's inputs
and parts; these temporary changes are not saved to your profile.

Drag the example tab down to detach
it into an independent window. Drag its system title bar to another monitor
to see your circuit and the example simultaneously. The example stays open
while you visit another level or the Wiki; its title identifies its level.

**Dock as tab**, or the detached window's close button, returns the example
to a tab without closing the application or interrupting your current work.
**Close example tab** closes the temporary example workspace. Environments
without native multi-window support use an in-app floating window instead.
**Fit example circuit** brings a large example back into view; normal zoom
and pan still work.

Other levels and free chips open named tabs on the same strip. Drag circuit
tabs to reorder them, or down to detach them just like the example. On tabs opened
later, **×** closes the workspace, not its saved circuit; the first tab stays open.
Change window frames under **Home → Settings**; root and detached windows use
the same preference.

If you want to look at the solution before passing the test, the game asks
you once more. Try it yourself first: when you get stuck, the
[Gates](kapilar.md) and [Selector](mux.md) pages give hints. On levels you've
passed, the example solution is always available; it's a good way to compare
with your own solution.
