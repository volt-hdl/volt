// Bu dosya Volt tarafından otomatik üretilmiştir.
// Kaynak: counter.volt
// Volt sürümü: 0.1.0
//
// DÜZENLEMEYİN — değişiklikler kaynak dosyada yapılmalıdır.

`default_nettype none

// 8-bit yukarı sayaç
// enable yüksekken her saat kenarında artar
module Counter (
    input  logic       clk,
    input  logic       rst,
    input  logic       enable,
    output logic [7:0] count
);

    logic [7:0] count_r;

    always_ff @(posedge clk) begin
        if (rst) begin
            count_r <= 8'd0;
        end else begin
            if (enable) begin
                count_r <= count_r + 8'd1;
            end
        end
    end

    assign count = count_r;

endmodule

`default_nettype wire
