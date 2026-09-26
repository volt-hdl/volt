//~ E2027
// ADR-0081 Karar 11: her seviye bir alttakini iki kez çağırır; açılım
// 2^19 kopya ister — bütçe çağrı yerinde.
fn f0(a: u8) -> u8 { a + 1 }
fn f1(a: u8) -> u8 { f0(a) ^ f0(~a) }
fn f2(a: u8) -> u8 { f1(a) ^ f1(~a) }
fn f3(a: u8) -> u8 { f2(a) ^ f2(~a) }
fn f4(a: u8) -> u8 { f3(a) ^ f3(~a) }
fn f5(a: u8) -> u8 { f4(a) ^ f4(~a) }
fn f6(a: u8) -> u8 { f5(a) ^ f5(~a) }
fn f7(a: u8) -> u8 { f6(a) ^ f6(~a) }
fn f8(a: u8) -> u8 { f7(a) ^ f7(~a) }
fn f9(a: u8) -> u8 { f8(a) ^ f8(~a) }
fn f10(a: u8) -> u8 { f9(a) ^ f9(~a) }
fn f11(a: u8) -> u8 { f10(a) ^ f10(~a) }
fn f12(a: u8) -> u8 { f11(a) ^ f11(~a) }
fn f13(a: u8) -> u8 { f12(a) ^ f12(~a) }
fn f14(a: u8) -> u8 { f13(a) ^ f13(~a) }
fn f15(a: u8) -> u8 { f14(a) ^ f14(~a) }
fn f16(a: u8) -> u8 { f15(a) ^ f15(~a) }
fn f17(a: u8) -> u8 { f16(a) ^ f16(~a) }
fn f18(a: u8) -> u8 { f17(a) ^ f17(~a) }
fn f19(a: u8) -> u8 { f18(a) ^ f18(~a) }

module Budget {
    in  a : u8
    out y : u8
    y = f19(a)
//~^ ERROR E2027 exceeded the AST node budget
}
