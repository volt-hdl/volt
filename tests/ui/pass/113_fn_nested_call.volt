// ADR-0081: iç içe çağrı ve hijyen — fn gövdesindeki K, çağıran modüldeki
// `let K`'ye değil birimin const'una bağlanır (çözüm tabanlı açılım).

const K : u8 = 3

fn add_k(a: u8) -> u8 {
    a + K
}

fn twice(a: u8) -> u8 {
    add_k(add_k(a))
}

module FnNested {
    in  a : u8
    in  n : u8
    out y : u8
    out z : u8

    let K = n
    y = twice(a)
    z = K
}
