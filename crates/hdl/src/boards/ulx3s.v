// ULX3S (Lattice ECP5): makine SoC at 12.5 MHz (from the board's 25 MHz
// clock), on its USB serial port (115200 baud) and LEDs. Button 1 (FIRE1)
// resets.
`default_nettype none

module top (
    input  wire       clk_25mhz,
    input  wire [6:0] btn,
    input  wire       ftdi_txd,     // FPGA receives
    output wire       ftdi_rxd,     // FPGA transmits
    output wire [7:0] led,
    output wire       wifi_gpio0    // keep the ESP32 from taking over
);
    assign wifi_gpio0 = 1'b1;

    wire clk, locked;
    pll pll (.clkin(clk_25mhz), .clkout0(clk), .locked(locked));

    // Reset for a few clocks after the PLL locks, and while FIRE1 is held.
    reg [3:0] por = 0;
    always @(posedge clk)
        if (!locked) por <= 0;
        else if (!por[3]) por <= por + 1;
    wire reset = !por[3] || btn[1];

    wire halted;
    wire [7:0] last;
    makine_soc #(
        .CLK_HZ(12_500_000),
        .BAUD(115_200),
        .RAM_WORDS(16384),
        .RAM_INIT("program.hex")
    ) soc (
        .clk(clk),
        .reset(reset),
        .uart_rx(ftdi_txd),
        .uart_tx(ftdi_rxd),
        .halted(halted),
        .leds(last)
    );
    assign led = {halted, last[6:0]};
endmodule
