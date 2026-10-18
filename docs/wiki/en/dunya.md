<!--
seviyeler:
-->
# World: build your own circuit

**World** is the free-play version of the level editor. No goal, no test:
you place any parts you want, connect them with wires, and watch the circuit
run. Click switches on and off and watch the lamps light up.

## Placing parts

**Drag** a part from the **Parts** list on the right onto the board. Or click
a part once, then click wherever you want on the board. The part stays in
your hand until you press **Esc**, so you can place several in a row.

- **Switch**: turns on or off when clicked.
- **Lamp**: lights up if the wire connected to it is not 0.
- **NOT, AND, OR, XOR, NAND, NOR**: ready-made logic gates.
- **Clock**: blinks on and off by itself. Select it to set its period.
- **Input / Output**: named pins. Clicking an input changes its value.
  When you make a chip, these become the chip's pins.
- **Constant, Split, Join**: same as in the levels, for working with buses.
- **Devices**: RAM, ROM, register, a 4096-byte Program ROM, screen, keyboard,
  and number display.
- **Your chips**: every level you've solved and every free chip you've made.

## Drawing wires

Drag from a part's **output pin** (the dot on the right) to an **input pin**
(the dot on the left). One output can have as many wires as you like; an input
takes only one wire.

- To cut a wire, **right-click** it, or hover over it and press **Delete**.
- Grab a wire's input end and drop it on another input to move the wire there;
  drop it in empty space to cut it.
- A wire's color shows its value: a lit wire is **1**, a dim wire is **0**.
  Blue wires are buses that carry more than one bit.
- Right-click a wire and choose **Edit wire name / path**: give the bus a name
  in the right panel. With **Add bend**, click on the board to set the path;
  finish with Enter or Esc. **Auto path** turns it back into a curved wire.
  Renaming a wire doesn't change what it's connected to.

## Selecting and moving

- **Drag** a part to move it. Dragging empty space pans the board.
- **Shift+drag** selects all the parts in an area.
- With a selection, **Ctrl+C / Ctrl+V** copies, **Ctrl+D** duplicates, and
  **Delete** removes. Wires between copied parts come along too.
- **Align top / Align left** lines up the selected parts.
- The **minimap** keeps you from getting lost in a big circuit; click the map
  to jump to another area. **Home / Center** shows the whole circuit.

## Modules and signal watching

Select an area, give it a name in the right panel, and press **Make module**.
Double-click a module to **go inside**: only its contents are shown, and its
connections to the outside appear as labels on the left (incoming) and right
(outgoing). Get back out with the path bar at the top or with **Esc**. A
closed module's pins lead to the real connections inside it. Part positions
and the simulation state are kept. Modules can be nested.

Right-click a wire and choose **Watch signal**. The signal history shows the
last 256 simulation steps. **Stop on change** pauses the simulation when that
signal changes. **Step** moves the clock forward one step; **Clock cycle** runs
until the next rising edge of the first clock.

Details: [Modules](moduller.md), [Signal watching](sinyaller.md).

## Try the 4004 workshop

In the right panel, go to **Example projects → Open 4004 workshop**. Your
current world stays; the example opens in a new world. It is an arithmetic and
control core made of real gates, not a full MCS-4 system.
[Read what works and see the example program](4004.md).

## Making a chip

1. Place **Input** parts for the chip's inputs and **Output** parts for its
   outputs, and name them (in the right panel, with the part selected).
2. Build the circuit and try it out.
3. Select everything with **Shift+drag**.
4. Type a name in the **Turn into chip** section of the right panel and press
   **Make chip**.

The chip's pins are the Inputs and Outputs in the selection, in top-to-bottom
order. The new chip is added to the **Your chips** list: you can use it as a
single part in the level editor or in another world.

## Building together

In the **Build together** section you can share your world with friends on
the same network. Everyone works on the same board and sees each other's
mouse pointers. Module names, members, and wire paths are shared; whether you
see a module open or closed, and which signals you are watching, belong only
to your own view.

### Over the internet

If you are in different homes, you need a **relay server**: both of you connect
to it, so nobody has to open a port on their router. The server is the small
`makine-relay` program that comes with makine; run it on a computer everyone
can reach (for example a cheap cloud server):

```
makine-relay              # listens on port 7879
```

Then both of you type the same address into **Build together → Server**. The
one sharing presses **🌐 Share on the internet** and gets a six-letter **room
code**; the other types the code and presses **Join**. The server only passes
bytes along; it does not read or keep the worlds.
