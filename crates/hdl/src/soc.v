// makine SoC: the RISC-V CPU exported from makine, with block RAM, a UART
// and a timer, ready for an FPGA.
//
// The CPU is single-cycle and reads instruction and data memory in the same
// cycle, as makine's computer does. FPGA block RAM answers one clock after
// it is asked, so every instruction takes several clocks here:
//
//   FETCH, DECODE:      the RAM reads the word at pc         -> instr
//   SETTLE:             wait for the CPU's gates to settle
//   ADDR, WAIT, READ:   read the word at the CPU's addr      -> rdata
//   SETTLE:             wait again
//   STEP, NEXT:         the CPU's clock rises: registers and pc update;
//                       a store goes to RAM or a device
//
// The waits matter on real hardware: the CPU is built from NAND gates, so
// its longest path (through the divider) takes a few hundred nanoseconds.
// SETTLE clocks must cover it; at 12.5 MHz, 8 clocks are 640 ns.
//
// A store to the console waits until the UART is free, so nothing is lost.
// The memory map is makine's (crates/rv/src/machine.rs):
//
//   0x0000_0000  RAM (RAM_WORDS words)
//   0x1000_0000  console: write = send a byte over the UART; read = next
//                received byte, or 0xFFFF_FFFF if none
//   0x1000_0004  timer: milliseconds since reset
//   0x1000_0018  random number
//
// `ebreak` stops the CPU and lights `halted`.
`default_nettype none

module makine_soc #(
    parameter CLK_HZ    = 25_000_000,
    parameter BAUD      = 115_200,
    parameter RAM_WORDS = 16384,          // 64 KB
    parameter RAM_INIT  = "program.hex",  // one 32-bit hex word per line
    parameter SETTLE    = 8               // clocks to wait for the CPU's gates
) (
    input  wire       clk,
    input  wire       reset,              // active high
    input  wire       uart_rx,
    output wire       uart_tx,
    output wire       halted,
    output wire [7:0] leds                // last byte written to the console
);
    localparam ADDR_BITS = $clog2(RAM_WORDS);
    localparam CONSOLE = 32'h1000_0000;
    localparam TIMER   = 32'h1000_0004;
    localparam RANDOM  = 32'h1000_0018;
    localparam EBREAK  = 32'h0010_0073;

    // ---- RAM: one port, byte writes, initialized with the program ----
    reg [31:0] ram [0:RAM_WORDS-1];
    initial $readmemh(RAM_INIT, ram);
    reg  [ADDR_BITS-1:0] ram_addr = 0;
    reg  [31:0] ram_q = 0;
    reg  [31:0] ram_wdata = 0;
    reg  [3:0]  ram_we = 0;
    always @(posedge clk) begin
        if (ram_we[0]) ram[ram_addr][7:0]   <= ram_wdata[7:0];
        if (ram_we[1]) ram[ram_addr][15:8]  <= ram_wdata[15:8];
        if (ram_we[2]) ram[ram_addr][23:16] <= ram_wdata[23:16];
        if (ram_we[3]) ram[ram_addr][31:24] <= ram_wdata[31:24];
        ram_q <= ram[ram_addr];
    end

    // ---- the CPU from makine ----
    reg         cpu_clk = 1'b0;
    reg         cpu_reset = 1'b1;
    reg  [31:0] instr = 32'h13;
    reg  [31:0] rdata = 0;
    wire [31:0] pc, addr, wdata;
    wire [3:0]  wmask;
    wire        we;
    mk_Cpu cpu (
        .instr(instr), .rdata(rdata), .reset(cpu_reset), .clk(cpu_clk),
        .pc(pc), .addr(addr), .wdata(wdata), .wmask(wmask), .we(we)
    );

    // ---- devices ----
    reg [31:0] millis = 0;
    reg [31:0] tick = 0;
    reg [31:0] rng = 32'h2545F491;
    always @(posedge clk) begin
        if (reset) begin
            millis <= 0;
            tick <= 0;
        end else if (tick == CLK_HZ / 1000 - 1) begin
            tick <= 0;
            millis <= millis + 1;
        end else
            tick <= tick + 1;
    end

    wire       tx_busy;
    reg        tx_start = 0;
    reg  [7:0] tx_byte = 0;
    uart_tx #(.CLKS_PER_BIT(CLK_HZ / BAUD)) utx (
        .clk(clk), .start(tx_start), .data(tx_byte), .tx(uart_tx), .busy(tx_busy)
    );
    wire       rx_valid;
    wire [7:0] rx_byte;
    uart_rx #(.CLKS_PER_BIT(CLK_HZ / BAUD)) urx (
        .clk(clk), .rx(uart_rx), .valid(rx_valid), .data(rx_byte)
    );
    // One received byte waits here until the program reads it.
    reg        rx_full = 0;
    reg  [7:0] rx_hold = 0;
    reg  [7:0] led_reg = 0;
    assign leds = led_reg;

    // ---- sequencer ----
    // The RAM's output ram_q is the word at the ram_addr of one clock
    // earlier, so each read takes two steps: set the address, wait.
    localparam S_BOOT = 0, S_FETCH = 1, S_DECODE = 2, S_SETTLE1 = 3, S_ADDR = 4,
               S_WAIT = 5, S_READ = 6, S_SETTLE2 = 7, S_STEP = 8, S_NEXT = 9,
               S_HALT = 10;
    reg [3:0] state = S_BOOT;
    reg [7:0] wait_count = 0;
    reg [1:0] boot_edges = 0;
    assign halted = state == S_HALT;
    wire in_ram  = addr < RAM_WORDS * 4;
    wire is_load = instr[6:0] == 7'h03;

    function [31:0] xorshift(input [31:0] x);
        reg [31:0] a, b;
        begin
            a = x ^ (x << 13);
            b = a ^ (a >> 17);
            xorshift = b ^ (b << 5);
        end
    endfunction

    always @(posedge clk) begin
        ram_we <= 0;
        tx_start <= 0;
        if (rx_valid && !rx_full) begin
            rx_full <= 1;
            rx_hold <= rx_byte;
        end
        if (reset) begin
            state <= S_BOOT;
            wait_count <= 0;
            boot_edges <= 0;
            cpu_reset <= 1;
            cpu_clk <= 0;
            instr <= 32'h13;
            rdata <= 0;
            rx_full <= 0;
            led_reg <= 0;
            rng <= 32'h2545F491;
        end else case (state)
            // Hold the CPU's reset through a few of its clock edges: pc = 0.
            S_BOOT: begin
                cpu_reset <= 1;
                wait_count <= wait_count + 1;
                if (wait_count == SETTLE - 1) begin
                    wait_count <= 0;
                    cpu_clk <= ~cpu_clk;
                    if (cpu_clk) begin
                        if (boot_edges == 3) begin
                            cpu_reset <= 0;
                            ram_addr <= 0;
                            state <= S_FETCH;
                        end
                        boot_edges <= boot_edges + 1;
                    end
                end
            end
            // ram_addr = pc: wait for the word.
            S_FETCH: state <= S_DECODE;
            S_DECODE: begin
                instr <= ram_q;
                state <= ram_q == EBREAK ? S_HALT : S_SETTLE1;
            end
            S_SETTLE1, S_SETTLE2: begin
                wait_count <= wait_count + 1;
                if (wait_count == SETTLE - 1) begin
                    wait_count <= 0;
                    state <= state == S_SETTLE1 ? S_ADDR : S_STEP;
                end
            end
            // The CPU now computes addr from instr: read it if it is a
            // load from RAM.
            S_ADDR: begin
                ram_addr <= addr[ADDR_BITS+1:2];
                state <= S_WAIT;
            end
            S_WAIT: state <= S_READ;
            S_READ: begin
                if (!(we && addr == CONSOLE && tx_busy)) begin
                    rdata <= in_ram ? ram_q
                           : addr == CONSOLE ? (rx_full ? {24'h0, rx_hold} : 32'hFFFF_FFFF)
                           : addr == TIMER ? millis
                           : addr == RANDOM ? rng
                           : 32'h0;
                    state <= S_SETTLE2;
                end
            end
            // Rising edge of the CPU clock: registers and pc update. The
            // store uses the values from before the edge.
            S_STEP: begin
                cpu_clk <= 1;
                if (we) begin
                    if (in_ram) begin
                        ram_addr <= addr[ADDR_BITS+1:2];
                        ram_wdata <= wdata;
                        ram_we <= wmask;
                    end else if (addr == CONSOLE && wmask[0]) begin
                        tx_byte <= wdata[7:0];
                        tx_start <= 1;
                        led_reg <= wdata[7:0];
                    end
                end else if (is_load && addr == CONSOLE)
                    rx_full <= 0;
                else if (is_load && addr == RANDOM)
                    rng <= xorshift(rng);
                state <= S_NEXT;
            end
            S_NEXT: begin
                cpu_clk <= 0;
                ram_addr <= pc[ADDR_BITS+1:2];
                state <= S_FETCH;
            end
            default: ; // S_HALT
        endcase
    end
endmodule

// 8N1 UART transmitter.
module uart_tx #(parameter CLKS_PER_BIT = 217) (
    input  wire       clk,
    input  wire       start,
    input  wire [7:0] data,
    output reg        tx = 1'b1,
    output wire       busy
);
    reg [9:0]  shift = 0;
    reg [3:0]  bits = 0;
    reg [15:0] count = 0;
    assign busy = bits != 0 || start;
    always @(posedge clk) begin
        if (bits == 0) begin
            tx <= 1'b1;
            if (start) begin
                shift <= {1'b1, data, 1'b0};
                bits <= 10;
                count <= 0;
            end
        end else if (count == CLKS_PER_BIT - 1) begin
            count <= 0;
            bits <= bits - 1;
            shift <= {1'b1, shift[9:1]};
        end else begin
            count <= count + 1;
        end
        if (bits != 0) tx <= shift[0];
    end
endmodule

// 8N1 UART receiver: `valid` is high for one clock with each byte.
module uart_rx #(parameter CLKS_PER_BIT = 217) (
    input  wire       clk,
    input  wire       rx,
    output reg        valid = 1'b0,
    output reg  [7:0] data = 0
);
    reg [1:0]  sync = 2'b11;
    reg [3:0]  bits = 0;
    reg [15:0] count = 0;
    reg [7:0]  shift = 0;
    wire in = sync[1];
    always @(posedge clk) begin
        sync <= {sync[0], rx};
        valid <= 0;
        if (bits == 0) begin
            if (!in) begin
                // Start bit: sample in the middle of each bit from now on.
                bits <= 9;
                count <= CLKS_PER_BIT / 2;
            end
        end else if (count == CLKS_PER_BIT - 1) begin
            count <= 0;
            bits <= bits - 1;
            if (bits == 1) begin
                valid <= in;   // a valid stop bit
                data <= shift;
            end else
                shift <= {in, shift[7:1]};
        end else
            count <= count + 1;
    end
endmodule
