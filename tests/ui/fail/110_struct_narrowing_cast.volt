//~ E2009
// ADR-0077 Karar 4: 'p as uN' needs N >= W (the struct's width); a
// narrower target would silently drop fields.
struct Ver {
    major : u4
    minor : u4
}

module M {
    in  v : Ver
    out y : u4

    y = v as u4
//~^ ERROR loses information
}
