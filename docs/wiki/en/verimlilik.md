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
example you get a ★: you've found something better than the game knows.

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

The **💡 See the solution** button opens the example circuit on the canvas.
Your own circuit isn't deleted; you come back with **Back to my circuit**. You
can change the example circuit's inputs and watch how it works, but your
changes aren't saved.

If you want to look at the solution before passing the test, the game asks
you once more. Try it yourself first: when you get stuck, the
[Gates](kapilar.md) and [Selector](mux.md) pages give hints. On levels you've
passed, the example solution is always available; it's a good way to compare
with your own solution.
