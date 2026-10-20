//! English titles and descriptions of the levels, by chip name. Turkish
//! is in the level definitions themselves.

/// (title, description) of a level in English.
pub(crate) fn level(chip: &str) -> Option<(&'static str, &'static str)> {
    Some(match chip {
        "Not" => (
            "NOT",
            "Output the opposite of the input: 1 if it is 0, 0 if it is 1.\n\n\
            All you have is NAND. What happens if you give both inputs of a NAND the same signal?",
        ),
        "And" => (
            "AND",
            "The output is 1 when both inputs are 1.\n\n\
            NAND already means 'NOT AND'. To invert it, use the NOT you just built.",
        ),
        "Or" => (
            "OR",
            "The output is 1 when at least one input is 1.\n\n\
            Hint: De Morgan's law. a OR b = NOT(NOT a AND NOT b).",
        ),
        "Xor" => (
            "XOR",
            "The output is 1 when the inputs differ.\n\n\
            XOR is the heart of an adder: 1 + 1 = 10, the low digit is 0.",
        ),
        "Mux" => (
            "Selector (MUX)",
            "Output a when sel = 0, b when sel = 1.\n\n\
            All over the computer, the MUX answers the question 'which data goes through?'.",
        ),
        "Demux" => (
            "Distributor (DEMUX)",
            "The opposite of a MUX: send the input to a when sel = 0, to b when sel = 1.\n\
            The other output stays 0.",
        ),
        "Mux32" => (
            "32-bit Selector",
            "Now we work with 32-bit numbers (buses) instead of single bits.\n\n\
            Output a when sel = 0, b when sel = 1. You don't need to place 32 MUXes: \
            select a part and set its 'Repeat' count to 32, and 32 copies work side by side. \
            If you connect a 1-bit wire to a wide input, that bit goes to every copy.",
        ),
        "HalfAdder" => (
            "Half Adder",
            "Add two bits. sum is the low digit, carry is the carry.\n\n\
            0+0=00, 0+1=01, 1+0=01, 1+1=10",
        ),
        "FullAdder" => (
            "Full Adder",
            "Add three bits: a, b and the carry from the previous digit (c).\n\n\
            Chain them side by side and you can add numbers of any width.",
        ),
        "Adder8" => (
            "8-bit Adder",
            "Add two 8-bit numbers and the incoming carry (c).\n\n\
            Use the Split part to break a bus into bits, and the Join part to turn bits \
            back into a bus. Chain 8 full adders: \
            each one's carry goes to the next. Bit 0 is the lowest digit.",
        ),
        "Adder32" => (
            "32-bit Adder",
            "Chain four 8-bit adders.\n\n\
            Your RISC-V processor will work with numbers of exactly this width.",
        ),
        "Sub32" => (
            "32-bit Subtractor",
            "Compute a - b.\n\n\
            In two's complement, -b = NOT(b) + 1. So a - b = a + NOT(b) + 1. \
            For the '+1' you can use the adder's carry input: the Const part \
            always outputs the same value.",
        ),
        "Latch" => (
            "SR Latch",
            "Where memory is born! When set = 1, q becomes 1; when reset = 1, it becomes 0.\n\
            While both are 0, q KEEPS its old value.\n\n\
            Hint: connect the output of each of two NANDs to an input of the other (feedback).",
        ),
        "DLatch" => (
            "D Latch",
            "While en = 1, q follows d. When en = 0, it holds the last value.\n\n\
            Make the SR latch's set and reset inputs from d and en.",
        ),
        "DFF" => (
            "D Flip-Flop",
            "Store d at the MOMENT the clock (clk) goes from 0 to 1; at all other times q stays fixed.\n\n\
            Everything in a computer moves forward step by step with these clock ticks. \
            Hint: put two D latches one after the other (master-slave). The first is open while \
            clk = 0, the second while clk = 1.",
        ),
        "Bit" => (
            "1-bit Register",
            "When load = 1, store d on the clock tick; when load = 0, keep the old value.\n\n\
            Hint: put a MUX in front of the DFF's input: either the new value or its own output.",
        ),
        "Register32" => (
            "32-bit Register",
            "Put 32 Bits side by side: every register in the processor is something like this.\n\n\
            Remember the 'Repeat' setting.",
        ),
        "Counter32" => (
            "Program Counter",
            "On every clock tick: if reset = 1, go back to 0; otherwise if load = 1, load d; \
            otherwise if inc = 1, add 4 (a RISC-V instruction is 4 bytes); otherwise stay the same.\n\n\
            The processor answers 'where is the next instruction?' with this counter.",
        ),
        "Ram8" => (
            "RAM (8 words)",
            "Memory: 8 words of 32 bits. addr selects which word. \
            q always shows the word at addr; on the clock tick, if we = 1, d is written there.\n\n\
            8 Register32s, a distributor (Demux tree) that selects the one to write, and a \
            selector (MUX tree) that selects the one to read. Your computer's main memory is \
            the same idea grown to millions of words; we simulate that in Rust.",
        ),
        "Zero32" => (
            "Is it zero?",
            "The output is 1 when the input is 0.\n\n\
            OR the 32 bits together: if any of them is 1, the number is not zero. \
            Splitting the bus in two with Split and repeating the same job at half the width \
            (like a tree) means both fewer parts and a faster circuit.",
        ),
        "Compare32" => (
            "Comparator",
            "Three questions: is a = b (eq)? Is a < b as signed numbers (lt)? \
            Is a < b as unsigned numbers (ltu)?\n\n\
            Hint: compute a - b. For equality, check whether the difference is zero. Unsigned \
            less-than comes from the adder's carry: in a + NOT(b) + 1, if the carry is 0 then a < b. \
            For signed less-than, look at the top bits: if the signs of a and b differ, \
            the negative one is smaller; if they are the same, the sign of the difference is the answer.\n\n\
            The branch instructions (beq, blt, bltu...) rely on this chip.",
        ),
        "ShiftLeft32" => (
            "Left Shifter",
            "Shift a left by n (0..31) places; 0s come in from the right.\n\n\
            The idea (barrel shifter): each bit of n is one stage. If bit 0 = 1, shift by 1; \
            if bit 1 = 1, shift by 2; bit 2 is 4, then 8 and 16. Each stage is a 32-bit MUX: \
            either leave it as is or pick the shifted version. Shifting by a fixed amount is just \
            wiring: shift the bits to one side with Split and Join, and fill the gap with Const 0. \
            Only the low 5 bits of n matter.",
        ),
        "ShiftRight32" => (
            "Right Shifter",
            "Shift a right by n places. When arith = 0, 0s come in from the left (logical); \
            when arith = 1, a's top bit (the sign) comes in (arithmetic).\n\n\
            Arithmetic shifting is how negative numbers are divided by 2: -8 >> 1 = -4.",
        ),
        "Alu" => (
            "ALU",
            "The processor's calculator. Operate on a and b according to op:\n\
            0 add: a + b       8 sub: a - b\n\
            1 sll: a << b       5 srl: a >> b       13 sra: a >> b (signed)\n\
            2 slt: 1 if a < b (signed)       3 sltu: 1 if a < b (unsigned)\n\
            4 xor     6 or     7 and\n\n\
            The low 3 bits of op are the funct3 field of RISC-V instructions, and bit 3 is the \
            bit that selects sub/sra. Shifts use only the low 5 bits of b. You can compute \
            every result and pick the right one with op.",
        ),
        "RegFile" => (
            "Register File",
            "RISC-V's 32 registers: x0..x31. Read two of them at once (the ones numbered rs1 and \
            rs2, into a and b); on the clock tick, if we = 1, write d into the one numbered rd.\n\n\
            Special rule: x0 is always 0; writing to it does nothing.\n\n\
            Hint: 31 Register32s (x0 needs no register), a 'decoder' that selects which register \
            to write (it turns rd into 32 load signals; it can be built from Demuxes) and two big \
            MUX trees for reading. Reading does not wait for the clock: when rs1 changes, \
            a changes immediately.",
        ),
        "Decoder" => (
            "Instruction Decoder",
            "Split a RISC-V instruction into its parts. The instruction is 32 bits; the fields:\n\
            opcode = bits 0..6, rd = 7..11, funct3 = 12..14, rs1 = 15..19, rs2 = 20..24.\n\n\
            imm is the 'immediate' number gathered from different places depending on the \
            instruction type, and sign-extended to 32 bits (filled with copies of its top bit; \
            this is always bit 31 of the instruction):\n\
            • I type (opcode 0x03 load, 0x13 arithmetic, 0x67 jalr): imm = instr[31:20]\n\
            • S type (0x23 store): imm = instr[31:25] and instr[11:7] side by side\n\
            • B type (0x63 branch): imm[12|10:5|4:1|11] = instr[31|30:25|11:8|7], imm[0] = 0\n\
            • U type (0x37 lui, 0x17 auipc): imm = instr[31:12] with 12 zeros below\n\
            • J type (0x6F jal): imm[20|10:1|11|19:12] = instr[31|30:21|20|19:12], imm[0] = 0\n\n\
            For other opcodes imm does not matter. Looking at only the relevant bits of the \
            opcode is enough.",
        ),
        "Control" => (
            "Control Unit",
            "The processor's boss: it looks at the instruction's opcode (and funct3/funct7) and \
            tells every part what to do. Outputs:\n\
            • reg_we: is the result written to a register (lui, auipc, jal, jalr, load, \
            arithmetic: 1; branch and store: 0)\n\
            • alu_b_imm: is the ALU's b input imm (1) or rs2 (0)? (arithmetic-imm, load, \
            store, jalr: 1; register-register arithmetic: 0)\n\
            • alu_op[4]: the ALU operation. For arithmetic instructions {funct7 bit 5, funct3}; but \
            for arithmetic-imm, bit 3 is used only for srai (there is no 'sub' for addi!). Load, \
            store and jalr add (0).\n\
            • wb_sel[2]: what is written to the register: 0 = ALU result, 1 = value read from memory, \
            2 = pc + 4 (jal/jalr return address), 3 = imm (lui).\n\
            • mem_read, mem_write: is it a load or a store.\n\
            • branch, jal, jalr: which kind of jump.\n\
            • auipc: is the ALU's a input pc?\n\
            • mul: the M extension (opcode 0x33 and funct7 = 1): take the result from the \
            multiply/divide unit.\n\n\
            For invalid instructions the outputs do not matter.",
        ),
        "BranchUnit" => (
            "Branch Unit",
            "Is the branch taken? According to funct3:\n\
            0 beq: equal   1 bne: not equal   4 blt: less than (signed)   5 bge: greater or equal \
            (signed)   6 bltu: less than (unsigned)   7 bgeu: greater or equal (unsigned)\n\n\
            The inputs are the Comparator's outputs. Bit 0 of funct3 inverts the result: \
            bne = NOT beq, bge = NOT blt...",
        ),
        "LoadUnit" => (
            "Load Unit",
            "Memory always gives an aligned 4-byte word. But the instruction may have asked \
            for a byte (lb, lbu), a half word (lh, lhu) or a word (lw).\n\n\
            off = the low 2 bits of the address: where in the word is the requested byte? First \
            shift word right by off × 8 bits, then cut it according to funct3:\n\
            0 lb: low 8 bits, sign-extend   1 lh: low 16 bits, sign-extend\n\
            2 lw: all of it   4 lbu: low 8 bits, zero-extend   5 lhu: low 16 bits, zero-extend",
        ),
        "StoreUnit" => (
            "Store Unit",
            "The opposite of loading: write only part of a word with sb/sh/sw.\n\n\
            Shift data left by off × 8 bits and output it as wdata. mask[4] tells which bytes \
            are written (bit 0 = the byte at the lowest address):\n\
            0 sb: 1 byte → 0001 << off   1 sh: 2 bytes → 0011 << off   2 sw: 4 bytes → 1111\n\n\
            Unaligned accesses (e.g. sh with off = 3) are not supported on this processor; \
            in those cases the output does not matter.",
        ),
        "MulDiv" => (
            "Multiply and Divide",
            "RISC-V's M extension. According to funct3:\n\
            0 mul: low 32 bits of a×b   1 mulh: high 32 bits of a×b (both signed)\n\
            2 mulhsu: high 32 bits (a signed, b unsigned)   3 mulhu: high 32 bits (unsigned)\n\
            4 div: a÷b (signed)   5 divu: a÷b (unsigned)   6 rem: remainder (signed)   \
            7 remu: remainder (unsigned)\n\n\
            Division by zero is not an error: the quotient is all 1s and the remainder is a. \
            -2³¹ ÷ -1 = -2³¹, remainder 0.\n\n\
            Multiplication: as you learned at school, for each bit of b shift a and add (32 adders \
            for a 64-bit result). Division: long division, at each step check whether the divisor \
            fits into the remainder. For signed operations, take absolute values first, then fix the sign.\n\n\
            This is the biggest circuit; fast simulation (⚡) helps a lot here.",
        ),
        "Cpu" => (
            "RISC-V Processor",
            "Put it all together! One instruction per clock tick:\n\
            1. The instruction at pc comes in on the instr input. Split it with the Instruction Decoder.\n\
            2. Read rs1 and rs2 from the Register File.\n\
            3. Let the ALU compute (b = rs2 or imm; a = rs1, or pc for auipc).\n\
            4. Load/store: address = the ALU result. addr = the aligned address (low 2 bits 0), \
            off = the low 2 bits. The word read comes in on rdata.\n\
            5. Write the result (chosen by wb_sel) to rd.\n\
            6. The next pc: normally pc + 4; jal: pc + imm; jalr: (rs1 + imm) & ~1; \
            if a branch is taken, pc + imm.\n\n\
            Memory is outside: the instruction at the address on the pc output comes in on instr, \
            and the word at addr comes in on rdata. While we = 1, on the clock tick wdata is written \
            to the bytes in wmask.\n\n\
            When reset = 1, pc becomes 0 on the clock tick.\n\n\
            The test runs real programs on your processor and after every instruction compares \
            the registers and memory writes with a reference processor.",
        ),
        "Mux4" => (
            "4-bit Selector",
            "Pick one of two 4-bit buses. a when sel=0, b when sel=1.\n\n\
            Four copies of the one-bit MUX can share the same select signal. These selectors \
            decide which value a processor writes.",
        ),
        "Adder4" => (
            "4-bit Adder and Carry",
            "Compute a+b+c; output the low four bits as sum and the fifth bit as carry.\n\n\
            For 15+1, sum=0 and carry=1. 4-bit processors like the Intel 4004 add wider numbers \
            piece by piece using the carry bit.",
        ),
        "Register4" => (
            "4-bit Register",
            "While load=1, store d on the rising clk edge; at other times q does not change.\n\n\
            The ALU only computes. The accumulator is the register that keeps the computed \
            result until the next instruction.",
        ),
        "Counter12" => (
            "12-bit Program Counter",
            "On every rising clk edge: 0 if reset; otherwise d if load; otherwise add one if inc; \
            otherwise keep the value.\n\n\
            12 bits select 4096 addresses. A branch loads a different address instead of \
            incrementing the counter. The address after 4095 is 0.",
        ),
        "AddSub8" => (
            "Add / Subtract",
            "Give a + b when sub = 0 and a - b when sub = 1 (8 bits; what overflows is dropped).\n\n\
             Subtracting needs no separate circuit: a - b = a + (NOT b) + 1. XOR b with sub \
             and it is inverted while sub = 1; the adder's carry input gives the +1.",
        ),
        "MiniDecode" => ("Instruction Fields", "Split an 8-bit instruction into the upper four bits op and lower four bits arg.\n\nFor example 0x47 = 0100 0111: op=4 (ADD), arg=7 (a RAM address). This stage does not compute; it only routes bits. Split groups start at the low bit: 4,4."),
        "MiniZero8" => ("Zero Flag", "Set zero=1 only when all eight bits of a are 0.\n\nConditional jumps use this bit. Both 00000001 and 10000000 are nonzero: checking one bit is not enough. OR all bits together and invert. A balanced tree shortens the longest path."),
        "MiniPc4" => ("Instruction Address and Jump", "At each rising edge choose the next pc: 0 on reset; otherwise unchanged on hold; otherwise target on jump; otherwise pc+1.\n\nFour bits hold 0–15; 15+1 wraps to 0. Mini addresses count instructions, unlike RISC-V's pc+4. Changing target between clock edges must not change pc. Reset overrides hold too."),
        "MiniAcc8" => ("Accumulator and Write Enable", "An 8-bit acc register: at the rising edge reset=1 clears it; otherwise load=1 stores d; otherwise keep its value.\n\nThe ALU can compute continuously, but not every instruction writes a register. ST and JMP must preserve acc. With d=7, load=1, clk=0 the register does not change; clk=1 stores 7. With load=0 new d is ignored. Reset has highest priority."),
        "MiniControl" => (
            "Mini Control",
            "The mini processor's boss: look at the instruction's op (and whether acc is 0) \
             and tell each part what to do.\n\n\
             op 1 LDI: load, imm. op 2 LD: load. op 3 ST: we. op 4 ADD: load, alu. \
             op 5 SUB: load, alu, sub. op 6 JMP: jump. op 7 JZ: jump if zero = 1. \
             op 15 HLT: hold. Others: all 0.",
        ),
        "MiniCpu" => (
            "Mini Processor",
            "Put it all together: a small processor with an 8-bit accumulator. One \
             instruction per clock tick.\n\n\
             An instruction is one byte: the top 4 bits are op, the low 4 bits arg. addr is \
             always arg; rdata is the byte in RAM at that address. On the rising edge acc and \
             pc update; if we = 1, acc is written to RAM. While reset = 1, the edge sets pc \
             and acc to 0.\n\n\
             The test runs three programs on your processor (countdown, multiply, Fibonacci) \
             and compares every step with a reference.",
        ),
        "AddressDecode2" => (
            "Address Decoder: Select One Cell",
            "With en=1, set only the output selected by the 2-bit addr to 1. With en=0, all outputs must be 0.\n\nAddresses 0,1,2,3 select s0,s1,s2,s3 respectively. This circuit will route RAM write enable to exactly one cell. An address is not the data itself.",
        ),
        "Rom4x8" => (
            "Build ROM: 4 Addresses × 8 Bits",
            "For addr=0,1,2,3 output 0x12, 0xA5, 0x3C, 0x80 respectively. q is eight-bit data.\n\nBuild your ROM from four Constants and a MUX tree. There is no clock or write input: changing the address changes the data. Programming this ROM means choosing constants and wiring; unlike RAM, it is not writable while running.",
        ),
        "Ram4x8" => (
            "Byte Memory: 4 Addresses × 8 Bits RAM",
            "q must always show the byte selected by addr. Only at a rising clk edge with we=1, write d to that address; preserve the other cells.\n\nUse four Bit×8 registers, the Address Decoder and MUX×8 selectors. Unlike ROM constants, RAM changes while running. Changing address or data while the clock stays high is not another write.",
        ),
        "ProgramRom" => (
            "Encode a Program in ROM",
            "Build a 16-address, 8-bit instruction ROM: 0→0x13, 1→0x30, 2→0x40, 3→0x31, 4→0xF0; addresses 5–15 contain 0x00.\n\nThese encode the Mini CPU program LDI 3; ST 0; ADD 0; ST 1; HLT. The upper four bits are the opcode, the lower four the argument. The program writes 3 to RAM[0] and 6 to RAM[1]. Here you store instructions; you do not execute them yet.",
        ),
        "RomSequencer" => (
            "Fetch Instructions from ROM",
            "Connect a 4-bit pc register to Program ROM; instr must always equal ROM[pc]. At each rising edge, priority is: reset→pc=0; load→pc=d; if run=1 and instr≠0xF0, pc=pc+1; otherwise hold. 15+1 wraps to 0.\n\nStep through addresses and instruction bytes; stop at HLT, load another address, reset to the start. This is an instruction-fetch circuit: it does not execute ADD or ST. To run the program, connect ALU, control and RAM in the Mini processor path next.",
        ),
        "Simd4" => (
            "Four Lanes: SIMD",
            "a and b each carry four 8-bit numbers side by side (byte 0 in the low bits). Add \
            each byte on its own: byte k of out = byte k of a + byte k of b. With sat = 0 the \
            overflow is dropped (200 + 100 = 44); with sat = 1 the result stops at 255 \
            (200 + 100 = 255).\n\n\
            This is the core idea of a GPU: single instruction, multiple data (SIMD). The same \
            operation that brightens one pixel is applied to four at once. Careful: a lane's \
            carry must not spill into the next; every lane is an adder of its own.",
        ),
        "Edge" => (
            "The Edge Function",
            "An edge from a to b and a point p (coordinates 0..31). Compute \
            E = (bx − ax) × (py − ay) − (by − ay) × (px − ax); right = 1 if E ≥ 0.\n\n\
            The sign of E says which side of the edge p is on. In screen coordinates y grows \
            downwards; E ≥ 0 means p is on the right of the edge (or on it). The differences \
            are −31..31 and the products are signed: Sub32 already gives the two's complement \
            difference. You can use MulDiv for the multiplication, but it is huge; a 6-bit \
            signed multiplication takes six rows of add-and-select.",
        ),
        "Triangle" => (
            "Inside a Triangle",
            "A triangle with corners a, b, c and a point p. inside = 1 if p is on the right \
            of (or on) all three edges (a→b, b→c, c→a).\n\n\
            If the corners go clockwise on screen, the inside lies on the right of all three \
            edges. This is how GPUs draw triangles: they run this test for every pixel of the \
            screen, each on its own and all at the same time. It is called rasterization.",
        ),
        "ScanCounter" => (
            "Scan the Screen",
            "A counter that walks the pixels of a 32 × 32 screen row by row. At every rising \
            clk edge: if reset = 1, x = 0 and y = 0. Otherwise x goes up by one; at 31 it \
            wraps to 0 and y goes up by one. y wraps to 0 after 31 too.\n\n\
            This is how screens draw a picture: left to right, top to bottom, dozens of times \
            a second. One frame of the screen takes 32 × 32 = 1024 clocks.",
        ),
        "Fill" => (
            "The Rasterizer",
            "Scan the screen and say for each pixel whether it is inside the triangle. x and y \
            advance as in ScanCounter (reset, clk); on = 1 when (x, y) is inside the \
            triangle.\n\n\
            This is a GPU's most basic job. In World, wire on to the screen's d input and x and \
            y to its coordinates, and the triangle gets drawn. A real GPU does the same with \
            thousands of cores, on thousands of pixels at once.",
        ),
        "Shade" => (
            "Coloured Pixel",
            "color is a 12-bit colour: bits 8..11 red, 4..7 green, 0..3 blue; each channel \
            0..15. near is how close the pixel is to the eye (0 farthest, 255 nearest). Far \
            things should look darker: let k = the top two bits of near (0..3); for each \
            channel out = channel × (k + 1) ÷ 4 (remainder dropped).\n\n\
            Each pixel of the screen is now a colour, not a bit. GPUs work out colours channel \
            by channel: the same operation on three channels separately, like the lanes in \
            Simd4. Channel × 3 = channel + channel × 2; × 2 and × 4 are only shifts. So is ÷ 4.",
        ),
        "DepthTest" => (
            "Depth Test",
            "on = 1 means the pixel is inside the triangle. near is how close the triangle is at \
            this pixel; old is the nearness stored for this pixel in the depth buffer. write = 1 \
            when on = 1 and near > old: the pixel is closer than what is in front, so draw it.\n\n\
            When triangles overlap, which one shows? Drawing from far to near like a painter \
            does not always work. GPUs keep, for every pixel, how near the nearest thing is, in \
            a memory called the depth buffer (z-buffer). A new pixel is drawn only if it is \
            closer, and the buffer is updated. The buffer starts at 0: everything infinitely far.",
        ),
        "FrameScan" => (
            "Triangle by Triangle",
            "Scan x and y as in ScanCounter, but also keep a triangle counter t (0..3). At every \
            rising clk edge: if reset = 1, x = y = t = 0. Otherwise x goes up by one; when x \
            wraps from 31 to 0, y goes up; when y also wraps from 31 to 0 (a frame is done), t \
            goes up. t wraps from 3 to 0.\n\n\
            A scene is made of many triangles. The simplest way is to draw them in turn: one \
            triangle per frame. t says which triangle's corners to read from memory. Real GPUs \
            process millions of triangles at once, in a pipeline.",
        ),
        "Raster3" => (
            "Scene Rasterizer",
            "Put it all together. x, y, t advance as in FrameScan. The corners, near and color \
            on the inputs are triangle t's (World supplies them from memory). old is the depth \
            buffer's value at (x, y). write = 1: (x, y) is inside t and near > old. c = color, \
            darkened by near as in Coloured Pixel.\n\n\
            In World: t selects a triangle's corners, colour and nearness from a ROM; a RAM is \
            the depth buffer; the colour screen paints the pixels with write = 1 in colour c. \
            Four coloured triangles, in front of and behind each other, show in the right order.",
        ),
        "FastAdd32" => (
            "Fast Adder",
            "Add a + b + c like Adder32, but the longest gate path must be at most 60% of \
            your own Adder32's.\n\n\
            In a ripple adder the carry walks from one bit to the next: 68 to 130 gates for \
            32 bits, depending on how you built the full adder. The clock can only tick once \
            that path has settled. A carry-select adder adds the upper bytes twice in advance, \
            for carry 0 and for carry 1; when the real carry arrives, a MUX picks the right \
            result. More gates, but a shorter path.",
        ),
        "PipeAdd32" => (
            "Pipeline: a Two-Stage Adder",
            "sum is a + b as it was at the previous rising clk edge (32 bits, overflow \
            dropped). Every edge starts a new addition. The longest gate path must be at most \
            60% of your own Adder32's.\n\n\
            Split the work in two: add the low 16 bits and put the result, the carry and the \
            upper halves of a and b in a register. After the edge, add the upper half. Each \
            stage is half as long, so the clock can tick twice as fast. One addition's result \
            comes out a clock later (latency), but a result comes out every clock \
            (throughput). This is the pipeline in processors.",
        ),
        "Forward" => (
            "Forwarding",
            "An instruction in a pipeline may need the result of the instruction before it, \
            not yet written to the register file. fa (for rs1) and fb (for rs2) say:\n\
            1: the result is in the previous instruction, in the EX stage (ex_we = 1, \
            ex_rd = rs, ex_rd ≠ 0)\n\
            2: otherwise in the one before that, in the MEM stage (mem_we = 1, mem_rd = rs, \
            mem_rd ≠ 0)\n\
            0: the value read from the register file is right.\n\n\
            EX is newer, so if both match, 1 wins. Nothing written to x0 is forwarded: x0 is \
            always 0.",
        ),
        "Cache4" => (
            "Cache",
            "A 4-line direct-mapped cache. The low 2 bits of addr pick the line, the upper 6 \
            bits are the tag. If the line's valid bit is 1 and its tag matches, hit = 1 and q \
            is the line's data. At a rising clk edge with fill = 1, d is written to the line: \
            valid = 1, tag = the upper 6 bits of addr. At power-on every line is invalid.\n\n\
            Main memory is slow: a processor waiting for it on every access stalls. A cache \
            keeps the words used last next to the processor. On a hit the data comes at once; \
            on a miss it is fetched from memory and written to the line. Real caches are the \
            same idea with thousands of lines.",
        ),
        "SeqMulDiv" => (
            "Multiply and Divide, Step by Step",
            "Do what MulDiv does, but one step per clock. funct3 is as in MulDiv.\n\
            While go = 1, every rising clk edge moves the work one step on. When the result is \
            ready, ready = 1 and out shows it. The edge with go = 1 while ready = 1 finishes \
            the work: ready goes back to 0, and the next edge starts a new operation. An edge \
            with go = 0 drops the work. From go = 1 until ready, a, b and funct3 do not change. \
            It may take up to 64 edges; the longest gate path must be at most 1.5 times your \
            own Adder32's.\n\n\
            MulDiv does 32 additions and 32 subtractions one after another: that is the \
            processor's longest path. Here one adder works at each edge. Multiplying: shift the \
            pair {A, Q} right at each step, first adding the multiplicand to A when Q's lowest \
            bit is 1. Dividing: shift {A, Q} left, subtract the divisor from A if it fits and \
            set the quotient bit. Fix the signs in one step each at the start and the end.",
        ),
        "CpuSeq" => (
            "A Fast-Clocked Processor",
            "Build your RISC-V processor again, this time with SeqMulDiv instead of MulDiv. The \
            pins are the same as in the Processor level. For a multiply or divide, go = 1; \
            until ready comes, the pc must not move on and no register is written (the \
            processor waits). When ready comes, write the result to rd and go on. Other \
            instructions still take one clock. The longest gate path must be at most 3 times \
            your own Adder32's.\n\n\
            The clock can only tick once the longest path has settled. In the single-cycle \
            processor that path goes through the divider and slows every other instruction \
            too. Dividing now takes 35 clocks, but the clock can tick about 20 times faster; \
            division is rare in programs, so they all get faster. The FPGA computer runs much \
            faster with this processor.",
        ),
        _ => return None,
    })
}

/// English name of a section, by its Turkish name.
pub(crate) fn section(tr: &str) -> Option<&'static str> {
    Some(match tr {
        "1. Mantık" => "1. Logic",
        "2. Aritmetik" => "2. Arithmetic",
        "3. Hafıza" => "3. Memory",
        "4. ALU" => "4. ALU",
        "5. İşlemci" => "5. Processor",
        "6. İşlemci atölyesi" => "6. Processor workshop",
        "Mini işlemci" => "Mini processor",
        "Bellek ve ROM" => "Memory and ROM",
        "7. Hızlı işlemci" => "7. A fast processor",
        "8. Grafik işlemci" => "8. A graphics processor",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_level_and_section_has_english() {
        for l in crate::levels() {
            assert!(level(l.chip).is_some(), "no English for level {}", l.chip);
            assert!(
                section(l.section).is_some(),
                "no English for section {}",
                l.section
            );
        }
    }

    #[test]
    fn english_keeps_paragraphs() {
        for l in crate::levels() {
            let (_, d) = level(l.chip).unwrap();
            assert_eq!(
                l.description.matches("\n\n").count(),
                d.matches("\n\n").count(),
                "{}",
                l.chip
            );
        }
    }
}
