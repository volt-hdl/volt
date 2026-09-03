//! Arena ve Idx birim testleri (F1b ADIM 0b — kapsam hedefi %70+).

use std::collections::HashSet;

use volt_ast::{Arena, Idx};

#[test]
fn alloc_returns_sequential_indices() {
    // Arrange
    let mut arena: Arena<&str> = Arena::default();

    // Act
    let a = arena.alloc("ilk");
    let b = arena.alloc("ikinci");

    // Assert — indeksler ardışık ve farklı
    assert_ne!(a, b);
    assert_eq!(format!("{a:?}"), "Idx(0)");
    assert_eq!(format!("{b:?}"), "Idx(1)");
}

#[test]
fn index_returns_allocated_value() {
    let mut arena: Arena<u32> = Arena::default();
    let idx = arena.alloc(42);
    assert_eq!(arena[idx], 42);
}

#[test]
fn index_after_many_allocs() {
    let mut arena: Arena<usize> = Arena::default();
    let indices: Vec<Idx<usize>> = (0..100).map(|i| arena.alloc(i * 3)).collect();
    for (i, idx) in indices.iter().enumerate() {
        assert_eq!(arena[*idx], i * 3);
    }
}

#[test]
fn idx_is_copy_and_eq() {
    let mut arena: Arena<bool> = Arena::default();
    let idx = arena.alloc(true);
    let copy = idx; // Copy — taşıma değil
    assert_eq!(idx, copy);
    #[allow(clippy::clone_on_copy)]
    let cloned = idx.clone();
    assert_eq!(idx, cloned);
}

#[test]
fn idx_works_as_hash_key() {
    let mut arena: Arena<char> = Arena::default();
    let a = arena.alloc('a');
    let b = arena.alloc('b');

    let mut set = HashSet::new();
    set.insert(a);
    set.insert(b);
    set.insert(a); // yinelenen ekleme büyütmemeli

    assert_eq!(set.len(), 2);
    assert!(set.contains(&a));
    assert!(set.contains(&b));
}

#[test]
fn index_mut_modifies_in_place() {
    let mut arena: Arena<u32> = Arena::default();
    let idx = arena.alloc(1);
    arena[idx] = 99;
    assert_eq!(arena[idx], 99);
}

#[test]
fn arena_clone_preserves_contents() {
    let mut arena: Arena<String> = Arena::default();
    let idx = arena.alloc("kalıcı".to_string());
    let clone = arena.clone();
    assert_eq!(clone[idx], "kalıcı");
}

#[test]
fn arena_debug_is_printable() {
    let mut arena: Arena<u8> = Arena::default();
    arena.alloc(7);
    let dump = format!("{arena:?}");
    assert!(
        dump.contains('7'),
        "Debug çıktısı içeriği göstermeli: {dump}"
    );
}

#[test]
#[should_panic]
fn index_out_of_bounds_panics() {
    let mut donor: Arena<u8> = Arena::default();
    let idx = donor.alloc(1); // Idx(0) başka arenadan
    let empty: Arena<u8> = Arena::default();
    let _ = empty[idx]; // boş arenada Idx(0) → panik
}
