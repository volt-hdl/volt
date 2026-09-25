//! Derleyici yığını (ADR-0080).
//!
//! Ağaç derinliği parser'da sınırlanır (`parser::depth`); o sınırdaki bir
//! ağacı yürümek için gereken yığın ise platforma göre değişir: Windows
//! ana iş parçacığı 1 MB, Linux 8 MB, test ve tokio iş parçacıkları 2 MB.
//! Debug derlemede en ağır geçit (SV üretimi) kat başına ~7 KB kullanır
//! (ölçüm, ADR-0080 §1) — 256 kat ≈ 1,8 MB, Windows ana iş parçacığını
//! aşar. Derleyici bu yüzden her platformda aynı, bilinen boyutta bir
//! yığında koşar; sınır ile yığın birlikte bir güvence oluşturur.

/// Derleyici iş parçacığının yığını: debug'da sınırdaki en ağır geçidin
/// ~35 katı (sanal bellek ayrılır; kullanılmayan sayfa fiziksel yer tutmaz).
pub const COMPILER_STACK_SIZE: usize = 64 * 1024 * 1024;

/// `f`'yi [`COMPILER_STACK_SIZE`] yığınlı bir iş parçacığında koşar ve
/// sonucunu döndürür. `f` panik ederse panik çağırana taşınır. İş
/// parçacığı açılamazsa (kaynak yok) `f` çağıranın yığınında koşar.
pub fn with_compiler_stack<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    // İş parçacığı açılamazsa `f` geri alınabilsin diye paylaşılan yuvada.
    let slot = std::sync::Mutex::new(Some(f));
    let take = || {
        slot.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    };
    std::thread::scope(|scope| {
        let spawned = std::thread::Builder::new()
            .name("volt-compiler".into())
            .stack_size(COMPILER_STACK_SIZE)
            .spawn_scoped(scope, || take().map(|f| f()));
        let ran = match spawned {
            Ok(handle) => match handle.join() {
                Ok(value) => value,
                Err(payload) => std::panic::resume_unwind(payload),
            },
            Err(_) => None,
        };
        ran.unwrap_or_else(|| take().expect("f koşulmadı, yuvada duruyor")())
    })
}
