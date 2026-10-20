<!--
seviyeler: Simd4, Edge, Triangle, ScanCounter, Fill, Shade, DepthTest, FrameScan, Raster3
-->
# A graphics processor: how does a GPU work?

A processor (CPU) is a general-purpose worker that can do any job. It runs
instructions one after another, very fast. A graphics processor (GPU)
answers a different question: how do we do **the same small job millions of
times, all at once**?

Think of one frame on a screen. A 1920 × 1080 screen has over two million
pixels, and 60 frames are drawn every second. Every pixel's colour has to be
worked out: more than 120 million pixels a second. But none of these
calculations waits for another: the top-left pixel's colour does not affect
the bottom-right one. The GPU exploits this independence.

## CPU and GPU

| | CPU | GPU |
|---|---|---|
| Cores | a few to a few dozen | thousands |
| One core | big and clever (branch prediction, out-of-order) | small and simple |
| Good at | steps that come in order and depend on each other | independent, identical operations |
| Memory | low latency (caches) | high bandwidth |

Apple M1's GPU has 8 cores. Each core has 128 arithmetic units; in total
1024 units work at the same time.

## 1. SIMD: single instruction, multiple data

The GPU's basic building block is **SIMD** (Single Instruction, Multiple
Data). A wide register is split into **lanes**, and the same operation is
applied to every lane at once.

In the **Simd4** level you split a 32-bit register into four 8-bit lanes.
Each lane could be one colour channel of a pixel. The important rule: a
lane's carry must **not** spill into its neighbour. With a single 32-bit
adder, 0xFF + 1 corrupts the next pixel.

**Saturating addition** matters in graphics too. Brightening a colour,
200 + 100 = 300, which does not fit in 8 bits. Dropping the overflow gives
44: a pixel that should be white turns almost black. Saturating addition
keeps the result at 255.

## 2. Triangles

Everything in a game, characters, buildings, trees, is made of triangles.
A modern game can have millions of them in one frame. A GPU's main job is to
turn these triangles into pixels. This is called **rasterization**.

### The edge function

To find which side of an edge a point is on, use the **edge function**. For
an edge from a to b and a point p:

```
E = (bx − ax) × (py − ay) − (by − ay) × (px − ax)
```

The sign of E gives the answer. On screen, y grows downwards, so when
E ≥ 0, p is on the **right** of the edge (or exactly on it).

In the **Edge** level you build this from gates. It takes two
multiplications, but the factors are small (−31 to 31), so the big
multiplier is not needed: six rows of shift-and-add are enough. For the sign
bit, subtract instead of adding in the last row, because in two's complement
the top bit's value is negative.

### The triangle test

A point is inside a triangle if it is on the **right of all three edges**
(with the corners in clockwise order). In the **Triangle** level you combine
three edge functions with AND.

If the corners come in the other order, no point is on the right of all
three edges and the triangle comes out empty. Real GPUs put this to good
use: counter-clockwise triangles are the **back faces** of an object and do
not need to be drawn (back-face culling).

## 3. Scanning the screen

A screen draws its pixels in a fixed order: left to right, top to bottom. In
the **ScanCounter** level you build the counter that produces this order. x
goes up every clock; after 31 it wraps to 0 and y goes up by one.

A frame of the 32 × 32 screen takes 1024 clocks. For 60 frames a second the
clock must run at 61,440 Hz. For a 1920 × 1080 screen that is about 124 MHz.
That is why a real GPU does not compute pixels one by one, but thousands at
a time.

## 4. The rasterizer

In the **Fill** level you put the pieces together: the counter scans the
screen, and the triangle test answers "inside?" for each pixel.

Once you solve the level, its panel shows a **🖥 In World: GPU: draw a
triangle** button. It opens a World where your Fill chip is wired to a
32 × 32 screen and draws the triangle. Click the corners' inputs to change
it.

The example triangle has corners (2,3), (29,8) and (9,27). In one frame,
309 pixels light up.

## 5. Colour, depth and many triangles

A scene is not a single black and white triangle. Four more levels add this:

- **Coloured Pixel (Shade):** each pixel is now a 12-bit colour: four bits of
  red, four of green, four of blue. Far things look darker: each channel is
  multiplied by ¼, ½, ¾ or full brightness depending on nearness. The three
  channels do the same sum separately, like the lanes in Simd4.
- **Depth Test (DepthTest):** when triangles overlap, which one shows? For
  every pixel the nearness of the nearest thing is kept in a memory, the
  **depth buffer** (z-buffer). A new pixel is drawn only if it is closer. So
  the order the triangles are drawn in stops mattering.
- **Triangle by Triangle (FrameScan):** the scanner gets one more digit:
  when a frame is done, the triangle counter t goes up. Each frame draws one
  triangle.
- **Scene Rasterizer (Raster3):** all of it together: scan, test inside,
  compare depth, work out the colour.

Once Raster3 is solved, a **🖥 In World: GPU: a colour scene** button
appears. There t selects a triangle's corners, colour and nearness from four
ROMs; a RAM is the depth buffer; the **colour screen** paints the pixels that
are written. After four frames the scene is complete: the yellow triangle in
front, the blue one at the back and darkest. Change the ROMs to build your own
scene. The colour screen is also in World's parts list.

## 6. The road to a real GPU

The rasterizer you built computes pixels one by one. Real GPUs go faster
with these ideas:

- **Parallel rasterization:** the screen is split into small tiles (say
  16 × 16). The pixels of each tile are tested at the same time on separate
  cores. The edge function suits this well: moving from one pixel to the
  next changes E by a constant, so not even a multiplication is needed, just
  an addition.
- **Shaders:** a program runs for every pixel and works out its colour,
  lighting and texture. Thousands of pixels run the same program on
  different data. This is Simd4's idea, made big.
- **Depth at every pixel:** here a triangle is equally near everywhere.
  Real GPUs blend nearness between the corners, using the ratios of the edge
  functions; that is how slanted surfaces are drawn.
- **Textures and caches:** images (textures) are read through special
  caches.
- **High-bandwidth memory:** moving the data of millions of pixels takes a
  very wide memory path.

The parts you built in this section are the smallest cells of these huge
machines: lanes, the edge function, the triangle test and the scanner. A GPU
is thousands of them working side by side.

To try your design on real hardware see the [A real chip](silikon.md) page;
for the ideas that make processors fast, the [A fast processor](hizli.md)
page.
