//! Batas waktu kernel (Step 3, B1-1). Binary tersendiri: penghitung
//! pekerjaan yang ditinggalkan bersifat global per proses, jadi tes timeout
//! tidak boleh berbagi proses dengan tes boolean lain.
#![allow(clippy::result_large_err)]

use std::time::Duration;

use ducad_engine::compute::budget::{abandoned_jobs, run};
use ducad_engine::OpErrorCode;

#[test]
fn slow_job_times_out_blocks_new_jobs_then_releases() {
    let e = run("lambat", Duration::from_millis(50), || {
        std::thread::sleep(Duration::from_millis(400));
        Ok(1)
    })
    .unwrap_err();
    assert_eq!(e.code, OpErrorCode::KernelTimeout);
    // Satu pekerjaan ditinggalkan: permintaan baru masih dilayani...
    let e2 = run("lambat-2", Duration::from_millis(50), || {
        std::thread::sleep(Duration::from_millis(400));
        Ok(1)
    })
    .unwrap_err();
    assert_eq!(e2.code, OpErrorCode::KernelTimeout);
    // ...dua ditinggalkan: permintaan baru ditolak sampai salah satu selesai.
    let busy = run("berikut", Duration::from_secs(5), || Ok(2)).unwrap_err();
    assert_eq!(busy.code, OpErrorCode::KernelTimeout);
    let t0 = std::time::Instant::now();
    while abandoned_jobs() > 0 && t0.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(run("lagi", Duration::from_secs(5), || Ok(3)).unwrap(), 3);
}
