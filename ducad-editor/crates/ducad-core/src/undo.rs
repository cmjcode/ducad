//! Undo/redo lintas domain: [`Command`], [`Transaction`], dan [`UndoStack`].
//!
//! Sebelumnya tiap domain memegang tumpukan undo sendiri (`ducad-app`
//! menyimpan satu `UndoStack<Sketch>` PER bidang sketsa DITAMBAH satu
//! `UndoStack<ModelDoc>` terpisah), sehingga satu aksi pengguna yang
//! menyentuh dua domain — "gambar profil lalu extrude" — menjadi dua
//! langkah undo yang tak berhubungan, dan domain yang lebih baru
//! (assembly, datum plane, drawing sheet) tidak punya undo sama sekali.
//!
//! Tiga hal yang ditambahkan modul ini di atas tumpukan lama:
//!
//! 1. **[`Transaction`]** — satu langkah undo yang membungkus BANYAK
//!    command. `apply` menjalankannya maju, `revert` mundur (urutan
//!    terbalik, karena command belakangan bisa bergantung pada efek
//!    command sebelumnya). `Transaction` sendiri implementasi [`Command`],
//!    jadi bisa bersarang tanpa perlakuan khusus.
//! 2. **Coalescing** — drag gizmo memancarkan puluhan command per detik;
//!    tanpa penggabungan, satu tarikan mouse menyisakan 60 langkah undo.
//!    Command yang memberi [`Command::coalesce_key`] yang sama dan tiba
//!    dalam [`UndoStack::coalesce_window`] digabung ke transaksi yang
//!    sedang di puncak, BUKAN didorong sebagai langkah baru.
//! 3. **Batas kedalaman** — tumpukan lama tumbuh tanpa batas. Untuk
//!    command modeling yang menyimpan snapshot B-rep, itu kebocoran
//!    memori yang nyata, bukan teoretis.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Jam logis bersama untuk SEMUA tumpukan undo di proses ini. Tiap kali
/// transaksi didorong, di-undo, atau di-redo, ia diberi stempel baru;
/// aplikasi dengan beberapa tumpukan (sketch, model, tinta) menurunkan
/// urutan undo global dari stempel puncak tiap tumpukan — tidak ada daftar
/// urutan terpisah yang bisa tidak sinkron akibat coalescing, transaksi
/// `begin/commit`, atau pengusiran `max_depth`.
static CLOCK: AtomicU64 = AtomicU64::new(1);

fn next_stamp() -> u64 {
    CLOCK.fetch_add(1, Ordering::Relaxed)
}

/// Operasi yang bisa di-undo terhadap target `T` — mis. `Document` (body
/// 3D) atau `Sketch` di ducad-sketch (entitas 2D). Generik sejak awal
/// supaya setiap lapisan dokumen dapat undo/redo yang sama tanpa
/// retrofit; semua mutasi WAJIB lewat trait ini.
pub trait Command<T> {
    fn name(&self) -> &str;
    fn apply(&mut self, target: &mut T);
    fn revert(&mut self, target: &mut T);

    /// Penanda penggabungan untuk aksi kontinu (drag gizmo, ketik dimensi,
    /// geser slider). Dua command dengan kunci sama yang tiba berurutan
    /// dalam jendela waktu singkat menjadi SATU langkah undo.
    ///
    /// Kuncinya sengaja `(&'static str, u64)` — tag jenis + id sasaran —
    /// bukan `dyn Any` + downcast: dengan begitu penggabungan tidak butuh
    /// tipe konkret, `Command` tetap object-safe, dan implementasi yang
    /// sudah ada tidak perlu diubah sama sekali (default `None` =
    /// perilaku lama, tidak pernah digabung).
    ///
    /// Contoh: `Some(("move_body", body_id.0.as_ffi()))` — drag pada body
    /// yang SAMA digabung, drag yang berpindah ke body lain tidak.
    fn coalesce_key(&self) -> Option<(&'static str, u64)> {
        None
    }
}

/// Satu langkah undo yang berisi satu atau lebih [`Command`] terhadap
/// target yang sama. Lihat catatan modul.
pub struct Transaction<T> {
    label: String,
    commands: Vec<Box<dyn Command<T>>>,
    coalesce_key: Option<(&'static str, u64)>,
    /// Stempel jam logis terakhir (lihat [`UndoStack::top_undo_stamp`]).
    stamp: u64,
}

impl<T> Transaction<T> {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            commands: Vec::new(),
            coalesce_key: None,
            stamp: 0,
        }
    }

    /// Transaksi berisi satu command, memakai nama command itu sebagai
    /// label dan mewarisi kunci coalescing-nya.
    pub fn single(cmd: Box<dyn Command<T>>) -> Self {
        Self {
            label: cmd.name().to_string(),
            coalesce_key: cmd.coalesce_key(),
            commands: vec![cmd],
            stamp: 0,
        }
    }

    /// Tambahkan command yang SUDAH diterapkan ke target.
    pub fn push_applied(&mut self, cmd: Box<dyn Command<T>>) {
        self.commands.push(cmd);
    }

    /// Terapkan `cmd` ke `target` lalu catat ke transaksi ini.
    pub fn run(&mut self, mut cmd: Box<dyn Command<T>>, target: &mut T) {
        cmd.apply(target);
        self.commands.push(cmd);
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }
}

impl<T> Command<T> for Transaction<T> {
    fn name(&self) -> &str {
        &self.label
    }

    fn apply(&mut self, target: &mut T) {
        for cmd in self.commands.iter_mut() {
            cmd.apply(target);
        }
    }

    /// Mundur dalam urutan TERBALIK: command belakangan boleh bergantung
    /// pada efek command sebelumnya, jadi efeknya harus dibatalkan lebih
    /// dulu.
    fn revert(&mut self, target: &mut T) {
        for cmd in self.commands.iter_mut().rev() {
            cmd.revert(target);
        }
    }

    fn coalesce_key(&self) -> Option<(&'static str, u64)> {
        self.coalesce_key
    }
}

/// Batas kedalaman bawaan. Command modeling menyimpan snapshot B-rep, jadi
/// tumpukan tanpa batas adalah kebocoran memori nyata pada sesi panjang.
pub const DEFAULT_MAX_DEPTH: usize = 200;

/// Jendela penggabungan bawaan untuk aksi kontinu. 300 ms cukup longgar
/// untuk drag pada 30 fps (33 ms antar frame) namun cukup ketat sehingga
/// dua tarikan terpisah yang disengaja tetap jadi dua langkah undo.
pub const DEFAULT_COALESCE_WINDOW: Duration = Duration::from_millis(300);

/// Tumpukan undo/redo, generik atas target `T`.
pub struct UndoStack<T> {
    undo: Vec<Transaction<T>>,
    redo: Vec<Transaction<T>>,
    /// Transaksi yang sedang dibuka lewat [`UndoStack::begin`].
    pending: Option<Transaction<T>>,
    /// Waktu masuknya command terakhir, dasar keputusan coalescing.
    last_push: Option<Instant>,
    max_depth: usize,
    coalesce_window: Duration,
}

// Impl manual (bukan #[derive(Default)]) agar tidak menambahkan bound
// keliru `T: Default` — Vec::new() tidak butuh itu.
impl<T> Default for UndoStack<T> {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            pending: None,
            last_push: None,
            max_depth: DEFAULT_MAX_DEPTH,
            coalesce_window: DEFAULT_COALESCE_WINDOW,
        }
    }
}

impl<T> UndoStack<T> {
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth.max(1);
        self
    }

    pub fn with_coalesce_window(mut self, window: Duration) -> Self {
        self.coalesce_window = window;
        self
    }

    // ----------------------------------------------------------------
    // Jalur sederhana: satu command = satu langkah undo.
    // ----------------------------------------------------------------

    /// Terapkan `cmd` dan catat sebagai satu langkah undo.
    ///
    /// Kalau ada transaksi terbuka ([`UndoStack::begin`]), command masuk ke
    /// dalamnya alih-alih jadi langkah tersendiri — pemanggil lama tidak
    /// perlu tahu apakah dirinya sedang berada di dalam transaksi.
    pub fn execute(&mut self, cmd: Box<dyn Command<T>>, target: &mut T) {
        self.execute_at(cmd, target, Instant::now());
    }

    /// Seperti [`UndoStack::execute`] tapi waktunya disuntikkan — dipakai
    /// test supaya perilaku coalescing bisa diuji tanpa `sleep`.
    pub fn execute_at(&mut self, mut cmd: Box<dyn Command<T>>, target: &mut T, now: Instant) {
        cmd.apply(target);

        if let Some(tx) = self.pending.as_mut() {
            tx.push_applied(cmd);
            self.last_push = Some(now);
            return;
        }

        // `try_coalesce` mengembalikan command-nya bila TIDAK jadi digabung,
        // supaya kepemilikan tidak hilang di jalur gagal.
        let Some(cmd) = self.try_coalesce(cmd, now) else {
            return;
        };

        self.push_transaction(Transaction::single(cmd), now);
    }

    /// Gabungkan `cmd` ke transaksi puncak bila kuncinya cocok dan masih
    /// dalam jendela waktu. Mengembalikan `None` bila tergabung, atau
    /// `Some(cmd)` — mengembalikan kepemilikan — bila tidak.
    ///
    /// Penggabungan dilakukan dengan MENAMBAHKAN command ke transaksi
    /// puncak, bukan menimpa isinya: dengan begitu `revert` transaksi
    /// gabungan tetap memutar balik seluruh rantai sampai keadaan sebelum
    /// drag dimulai, tanpa satu pun command perlu tahu cara menggabungkan
    /// dirinya dengan command lain.
    fn try_coalesce(
        &mut self,
        cmd: Box<dyn Command<T>>,
        now: Instant,
    ) -> Option<Box<dyn Command<T>>> {
        let Some(key) = cmd.coalesce_key() else {
            return Some(cmd);
        };
        let Some(last) = self.last_push else {
            return Some(cmd);
        };
        if now.saturating_duration_since(last) > self.coalesce_window {
            return Some(cmd);
        }
        let Some(top) = self.undo.last_mut() else {
            return Some(cmd);
        };
        if top.coalesce_key != Some(key) {
            return Some(cmd);
        }
        top.push_applied(cmd);
        top.stamp = next_stamp();
        self.last_push = Some(now);
        None
    }

    fn push_transaction(&mut self, mut tx: Transaction<T>, now: Instant) {
        tx.stamp = next_stamp();
        self.undo.push(tx);
        self.redo.clear();
        self.last_push = Some(now);
        while self.undo.len() > self.max_depth {
            self.undo.remove(0);
        }
    }

    // ----------------------------------------------------------------
    // Jalur transaksi eksplisit: banyak domain, satu langkah undo.
    // ----------------------------------------------------------------

    /// Buka transaksi. Semua [`UndoStack::execute`] sesudahnya masuk ke
    /// transaksi ini sampai [`UndoStack::commit`] atau
    /// [`UndoStack::rollback`].
    ///
    /// Memanggil `begin` saat transaksi lain masih terbuka akan
    /// meng-commit yang lama lebih dulu — lebih baik daripada diam-diam
    /// membuang pekerjaan yang sudah diterapkan ke dokumen.
    pub fn begin(&mut self, label: impl Into<String>) {
        if self.pending.is_some() {
            self.commit();
        }
        self.pending = Some(Transaction::new(label));
    }

    pub fn in_transaction(&self) -> bool {
        self.pending.is_some()
    }

    /// Tutup transaksi dan dorong sebagai satu langkah undo. Transaksi
    /// kosong dibuang (tidak menyisakan langkah undo yang tak melakukan
    /// apa-apa). Mengembalikan `true` bila ada langkah yang terdorong.
    pub fn commit(&mut self) -> bool {
        let Some(tx) = self.pending.take() else {
            return false;
        };
        if tx.is_empty() {
            return false;
        }
        self.push_transaction(tx, Instant::now());
        true
    }

    /// Batalkan transaksi yang terbuka: putar balik seluruh command yang
    /// sudah diterapkan, lalu buang. Dipakai saat satu langkah dalam
    /// operasi gabungan gagal — dokumen kembali persis seperti sebelum
    /// [`UndoStack::begin`], dan tidak ada langkah undo setengah jadi yang
    /// tertinggal di tumpukan.
    pub fn rollback(&mut self, target: &mut T) -> bool {
        let Some(mut tx) = self.pending.take() else {
            return false;
        };
        tx.revert(target);
        true
    }

    // ----------------------------------------------------------------
    // Navigasi.
    // ----------------------------------------------------------------

    pub fn undo(&mut self, target: &mut T) -> Option<String> {
        let mut tx = self.undo.pop()?;
        tx.revert(target);
        let label = tx.label().to_string();
        tx.stamp = next_stamp();
        self.redo.push(tx);
        // Langkah berikutnya memulai gugus baru: tanpa ini, command yang
        // datang sesudah undo bisa tergabung ke transaksi yang sekarang
        // sudah pindah ke tumpukan redo.
        self.last_push = None;
        Some(label)
    }

    pub fn redo(&mut self, target: &mut T) -> Option<String> {
        let mut tx = self.redo.pop()?;
        tx.apply(target);
        let label = tx.label().to_string();
        tx.stamp = next_stamp();
        self.undo.push(tx);
        self.last_push = None;
        Some(label)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_count(&self) -> usize {
        self.undo.len()
    }

    /// Stempel jam logis langkah undo teratas: kapan ia terakhir didorong,
    /// digabung, atau di-redo. Di antara beberapa tumpukan, yang stempelnya
    /// terbesar memegang aksi pengguna paling baru.
    pub fn top_undo_stamp(&self) -> Option<u64> {
        self.undo.last().map(|tx| tx.stamp)
    }

    /// Stempel langkah redo teratas: kapan ia di-undo. Yang terbesar di
    /// antara beberapa tumpukan adalah yang terakhir dibatalkan.
    pub fn top_redo_stamp(&self) -> Option<u64> {
        self.redo.last().map(|tx| tx.stamp)
    }

    /// Buang seluruh riwayat redo — dipakai aplikasi multi-tumpukan saat aksi
    /// baru di tumpukan LAIN membuat redo tumpukan ini tidak lagi berlaku.
    pub fn clear_redo(&mut self) {
        self.redo.clear();
    }

    pub fn redo_count(&self) -> usize {
        self.redo.len()
    }

    /// Label langkah yang akan dibatalkan berikutnya — untuk menu
    /// "Undo <nama aksi>".
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|tx| tx.label())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|tx| tx.label())
    }

    /// Daftar label dari yang terbaru ke terlama — untuk panel riwayat.
    pub fn undo_labels(&self) -> impl Iterator<Item = &str> {
        self.undo.iter().rev().map(|tx| tx.label())
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.pending = None;
        self.last_push = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Target uji sederhana: satu angka + jejak urutan apply/revert,
    /// cukup untuk membuktikan urutan eksekusi transaksi tanpa menyeret
    /// `Document` atau kernel ke dalam test.
    #[derive(Default, Debug, PartialEq)]
    struct Counter {
        value: i32,
        trace: Vec<String>,
    }

    struct AddCmd {
        label: String,
        delta: i32,
        key: Option<(&'static str, u64)>,
    }

    impl AddCmd {
        fn new(label: &str, delta: i32) -> Box<Self> {
            Box::new(Self {
                label: label.to_string(),
                delta,
                key: None,
            })
        }

        fn coalescing(label: &str, delta: i32, key: (&'static str, u64)) -> Box<Self> {
            Box::new(Self {
                label: label.to_string(),
                delta,
                key: Some(key),
            })
        }
    }

    impl Command<Counter> for AddCmd {
        fn name(&self) -> &str {
            &self.label
        }
        fn apply(&mut self, t: &mut Counter) {
            t.value += self.delta;
            t.trace.push(format!("+{}", self.label));
        }
        fn revert(&mut self, t: &mut Counter) {
            t.value -= self.delta;
            t.trace.push(format!("-{}", self.label));
        }
        fn coalesce_key(&self) -> Option<(&'static str, u64)> {
            self.key
        }
    }

    #[test]
    fn transaction_reverts_in_reverse_order() {
        // Urutan terbalik BUKAN kosmetik: command belakangan boleh
        // bergantung pada efek command sebelumnya, jadi efeknya harus
        // dibatalkan lebih dulu. Test ini mengunci urutan itu, bukan
        // sekadar nilai akhirnya.
        let mut t = Counter::default();
        let mut tx = Transaction::new("gabungan");
        tx.run(AddCmd::new("a", 1), &mut t);
        tx.run(AddCmd::new("b", 10), &mut t);
        tx.run(AddCmd::new("c", 100), &mut t);
        assert_eq!(t.value, 111);
        assert_eq!(t.trace, vec!["+a", "+b", "+c"]);

        tx.revert(&mut t);
        assert_eq!(t.value, 0);
        assert_eq!(t.trace[3..], ["-c", "-b", "-a"]);
    }

    #[test]
    fn multi_domain_action_is_one_undo_step() {
        // Alasan keberadaan modul ini: "gambar profil lalu extrude" harus
        // jadi SATU langkah undo, bukan dua yang tak berhubungan.
        let mut t = Counter::default();
        let mut stack = UndoStack::default();

        stack.begin("Sketsa + Extrude");
        stack.execute(AddCmd::new("sketsa", 2), &mut t);
        stack.execute(AddCmd::new("extrude", 3), &mut t);
        assert!(stack.in_transaction());
        assert!(stack.commit());

        assert_eq!(t.value, 5);
        assert_eq!(stack.undo_count(), 1, "dua command harus jadi satu langkah");
        assert_eq!(stack.undo_label(), Some("Sketsa + Extrude"));

        stack.undo(&mut t);
        assert_eq!(t.value, 0, "satu undo harus membatalkan keduanya");
        stack.redo(&mut t);
        assert_eq!(t.value, 5);
    }

    #[test]
    fn rollback_restores_state_and_leaves_no_undo_step() {
        // Dipakai saat satu langkah dalam operasi gabungan gagal: dokumen
        // harus kembali persis, dan tumpukan tidak boleh menyimpan langkah
        // setengah jadi.
        let mut t = Counter::default();
        let mut stack = UndoStack::default();
        stack.execute(AddCmd::new("awal", 7), &mut t);

        stack.begin("operasi gagal");
        stack.execute(AddCmd::new("langkah1", 1), &mut t);
        stack.execute(AddCmd::new("langkah2", 1), &mut t);
        assert_eq!(t.value, 9);

        assert!(stack.rollback(&mut t));
        assert_eq!(t.value, 7, "harus kembali ke keadaan sebelum begin()");
        assert_eq!(stack.undo_count(), 1, "hanya langkah 'awal' yang tersisa");
        assert!(!stack.in_transaction());
    }

    #[test]
    fn empty_transaction_leaves_no_undo_step() {
        let mut stack: UndoStack<Counter> = UndoStack::default();
        stack.begin("tidak melakukan apa-apa");
        assert!(!stack.commit());
        assert_eq!(stack.undo_count(), 0);
    }

    #[test]
    fn drag_with_same_key_coalesces_into_one_step() {
        // Drag gizmo memancarkan puluhan command per detik; tanpa ini satu
        // tarikan menyisakan puluhan langkah undo.
        let mut t = Counter::default();
        let mut stack = UndoStack::default();
        let t0 = Instant::now();

        for i in 0..10 {
            stack.execute_at(
                AddCmd::coalescing("geser", 1, ("move_body", 42)),
                &mut t,
                t0 + Duration::from_millis(i * 20),
            );
        }

        assert_eq!(t.value, 10);
        assert_eq!(stack.undo_count(), 1, "satu drag = satu langkah undo");

        stack.undo(&mut t);
        assert_eq!(t.value, 0, "undo harus memutar balik seluruh drag");
    }

    #[test]
    fn separate_drags_stay_separate_steps() {
        let mut t = Counter::default();
        let mut stack = UndoStack::default();
        let t0 = Instant::now();

        stack.execute_at(
            AddCmd::coalescing("geser", 1, ("move_body", 42)),
            &mut t,
            t0,
        );
        // Jeda melebihi jendela coalescing = tarikan baru yang disengaja.
        stack.execute_at(
            AddCmd::coalescing("geser", 1, ("move_body", 42)),
            &mut t,
            t0 + DEFAULT_COALESCE_WINDOW + Duration::from_millis(1),
        );
        assert_eq!(stack.undo_count(), 2);
    }

    #[test]
    fn different_targets_do_not_coalesce() {
        // Drag yang berpindah ke body lain tidak boleh tergabung, walau
        // terjadi dalam jendela waktu yang sama.
        let mut t = Counter::default();
        let mut stack = UndoStack::default();
        let t0 = Instant::now();

        stack.execute_at(
            AddCmd::coalescing("geser A", 1, ("move_body", 1)),
            &mut t,
            t0,
        );
        stack.execute_at(
            AddCmd::coalescing("geser B", 1, ("move_body", 2)),
            &mut t,
            t0 + Duration::from_millis(10),
        );
        assert_eq!(stack.undo_count(), 2);
    }

    #[test]
    fn commands_without_key_never_coalesce() {
        // Perilaku lama harus tetap: implementasi `Command` yang ada tidak
        // memberi `coalesce_key`, jadi tidak boleh ada yang tergabung
        // diam-diam hanya karena berdekatan waktu.
        let mut t = Counter::default();
        let mut stack = UndoStack::default();
        let t0 = Instant::now();
        for i in 0..5 {
            stack.execute_at(
                AddCmd::new("aksi", 1),
                &mut t,
                t0 + Duration::from_millis(i),
            );
        }
        assert_eq!(stack.undo_count(), 5);
    }

    #[test]
    fn coalescing_does_not_resume_across_undo() {
        // Setelah undo, transaksi puncak sudah pindah ke tumpukan redo.
        // Command berikutnya tidak boleh menempel ke transaksi itu.
        let mut t = Counter::default();
        let mut stack = UndoStack::default();
        let t0 = Instant::now();

        stack.execute_at(AddCmd::coalescing("geser", 1, ("move_body", 1)), &mut t, t0);
        stack.undo(&mut t);
        stack.execute_at(
            AddCmd::coalescing("geser", 1, ("move_body", 1)),
            &mut t,
            t0 + Duration::from_millis(10),
        );
        assert_eq!(stack.undo_count(), 1);
        assert_eq!(t.value, 1);
    }

    #[test]
    fn depth_limit_drops_oldest_step() {
        let mut t = Counter::default();
        let mut stack = UndoStack::default().with_max_depth(3);
        for i in 0..5 {
            stack.execute(AddCmd::new(&format!("aksi{i}"), 1), &mut t);
        }
        assert_eq!(stack.undo_count(), 3);
        // Yang tersisa adalah tiga TERBARU; yang terlama dibuang.
        let labels: Vec<&str> = stack.undo_labels().collect();
        assert_eq!(labels, vec!["aksi4", "aksi3", "aksi2"]);
    }

    #[test]
    fn new_action_clears_redo() {
        let mut t = Counter::default();
        let mut stack = UndoStack::default();
        stack.execute(AddCmd::new("a", 1), &mut t);
        stack.undo(&mut t);
        assert!(stack.can_redo());
        stack.execute(AddCmd::new("b", 5), &mut t);
        assert!(!stack.can_redo(), "aksi baru harus membuang cabang redo");
    }

    #[test]
    fn begin_while_open_commits_previous_instead_of_dropping_work() {
        // Transaksi yang sudah menyentuh dokumen tidak boleh hilang diam-
        // diam hanya karena pemanggil lupa commit.
        let mut t = Counter::default();
        let mut stack = UndoStack::default();
        stack.begin("pertama");
        stack.execute(AddCmd::new("a", 1), &mut t);
        stack.begin("kedua");
        stack.execute(AddCmd::new("b", 1), &mut t);
        stack.commit();
        assert_eq!(stack.undo_count(), 2);
        assert_eq!(t.value, 2);
    }

    #[test]
    fn nested_transaction_is_a_plain_command() {
        let mut t = Counter::default();
        let mut inner: Transaction<Counter> = Transaction::new("dalam");
        inner.run(AddCmd::new("a", 1), &mut t);
        inner.run(AddCmd::new("b", 2), &mut t);

        let mut outer: Transaction<Counter> = Transaction::new("luar");
        outer.push_applied(Box::new(inner));
        outer.run(AddCmd::new("c", 4), &mut t);
        assert_eq!(t.value, 7);

        outer.revert(&mut t);
        assert_eq!(t.value, 0);
    }
}
