//~ E0003
// ADR-0081 Karar 7: generic fn bu turda eşlenmez (W çağrıdan çıkarılmaz).
fn first<const N: u32>(a: bits<N>) -> bool {
//~^ ERROR E0003 not supported yet: generic functions
    a[0]
}
