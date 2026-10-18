<!--
seviyeler:
-->
# Modules: building a big circuit

Drawing an adder gate by gate teaches you a lot. But when sixteen adders, a
register bank, and control logic are spread across the same board, it gets
hard to see which part does what. A **module** lets you name a group of parts
that do one job together, and open up its details when you need them.

## Your first module: a buffer

1. Place a **Switch**, two **NOT**s, and a **Lamp**.
2. Connect the switch to the first NOT, its output to the second NOT, and the
   second NOT to the lamp.
3. Turn the switch on and off. The two inversions cancel out, so the lamp
   follows the switch.
4. Select only the two NOTs with **Shift+drag**.
5. In the right panel, type **Buffer** as the module name and press
   **Make module**.

The two NOTs are now inside a single module boundary. The wire coming in from
outside shows up at the module's input, and the wire going to the lamp at its
output. A pin on the boundary is not a new electrical part: it's a visible
stand-in for the real pin inside.

## Opening, closing, and moving

- **Double-click** the module, or press **Go inside** in the right panel: only
  the module's contents stay on the board. Signals coming in from outside
  appear on the left and signals going out appear on the right, with their
  names and live values. The path bar at the top shows where you are
  (*World › CPU › ALU*); click a name to go to that level, or press **Esc** to
  go up one level.
- Parts you place or paste while inside are added to that module.
- If you want to see the module in place with its surroundings, use the
  **Open / Close** button in the right panel.
- Dragging the title moves all members by the same amount; their layout is
  kept.
- **Ungroup** only removes the module boundary. The gates and wires stay.
- When you select members and press **Ctrl+C / Ctrl+V**, modules that are
  fully inside the selection are copied too. The copy's parts get new IDs.

Closing a module doesn't stop the simulation, empty it out, or swap it for an
emulator. The circuit inside is the same circuit. Opening and closing doesn't
reset RAM, registers, or the screen.

## Nested modules

You can select all the parts of two small modules and make a new module. For
example, **Adder** and **Result selector** together can become **ALU**. When
the outer module is closed you see only the outer boundary; open it and the
inner modules appear.

Modules can't partly overlap. If you want to put half of a module into another
module, first ungroup it or select all of it. That way it's never unclear
which structure a gate belongs to.

## Module vs. chip

| Module | Chip |
|---|---|
| A group of parts in this world | A circuit definition added to the library |
| Keeps the positions inside it | Can be used as a part in other circuits |
| Can also group devices like RAM, clocks, and registers | Some built-in devices can't be packed into a chip yet |
| Opened and closed while running | Shows up on the world as a single library part |

There is no "module type" system yet that updates every copy when you edit
one module: copies are independent. For reusable logic, place **Input /
Output** pins and use **Make chip**.

## While you build

1. Try each small part on its own.
2. Give its inputs and outputs clear names.
3. When it works, close the module and move on to the bigger structure.
4. When something goes wrong, open the related module first, not the whole
   processor.

Continue: [Signal watching](sinyaller.md) and [the 4004 workshop](4004.md).
