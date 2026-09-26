// ADR-0083: on/comb içinde match ifadesi — atamanın sağ tarafı kök
// konumdur (case, hedef her kolda), if koşulu ve port bağlaması iç
// konumdur (üçlü). for içinde match her iterasyonda açılır.

enum Mode { Idle, Load, Shift }

module Inc {
    in  x : u8
    out z : u8
    z = x + 1
}

module MatchExprBlocks {
    in  clk  : clock
    in  mode : Mode
    in  sel  : u2
    in  d    : u8
    in  lanes : [u8; 4]
    out q    : u8
    out y    : u8
    out s    : u8
    out w    : [u8; 4]

    reg acc : u8 = 0
    on clk {
        acc <= match mode {
            Mode::Idle  => acc,
            Mode::Load  => d,
            Mode::Shift => acc << 1
        }
    }
    q = acc

    comb {
        if (match sel { 0 | 1 => true, _ => false }) {
            y = d
        } else {
            y = match sel { 2 => d ^ 0xFF, _ => 0 }
        }
        for i in 0..4 {
            w[i] = match sel { 0 => lanes[i], _ => lanes[i] + 1 }
        }
    }

    let inc = Inc { x: match sel { 0 => d, _ => acc } }
    s = inc.z
}
