// Derleme zamanı sabitleri ve for döngüsü açma
// const-eval.md §7, §8

const WIDTH : u32 = 8;
const DEPTH : u32 = 4;

module ConstAndGenerate {
    in  clk  : clock
    in  data : bits<WIDTH>
    in  mask : bits<WIDTH>
    out result : bits<WIDTH>

    wire temp : bits<WIDTH>

    // for derleme zamanında açılıyor — donanımda döngü yok
    for i in 0..WIDTH {
        temp[i] = data[i] & mask[i]
    }

    result = temp
}
