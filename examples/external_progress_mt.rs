//! Minimal external_progress + host progress + multi-thread put (issue #54 goal 3).
use std::ffi::CString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

fn main() {
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = Arc::clone(&stop);
    let _progress = thread::Builder::new()
        .name("host-progress".into())
        .spawn(move || {
            while !stop2.load(Ordering::Acquire) {
                pmix::progress();
                thread::sleep(Duration::from_millis(1));
            }
            for _ in 0..64 {
                pmix::progress();
            }
        })
        .unwrap();

    let mut opts = pmix::InitOptions::new();
    opts.external_progress(true);
    let info = opts.build();
    eprintln!("connecting...");
    let client = pmix::PmixClient::connect_new(Some(info)).expect("connect");
    eprintln!("connected rank={:?}", client.rank());

    const N: usize = 4;
    let barrier = Arc::new(Barrier::new(N));
    let mut hs = vec![];
    for i in 0..N {
        let w = client.clone();
        let b = Arc::clone(&barrier);
        hs.push(thread::spawn(move || {
            assert!(w.is_live());
            b.wait();
            let key = CString::new(format!("ex.ext.{i}")).unwrap();
            let mut val = pmix::PmixValueBuilder::new()
                .string(&format!("v{i}"))
                .unwrap()
                .build()
                .unwrap();
            pmix::put_value(pmix::PmixScope::Global.to_raw(), &key, &mut val).expect("put");
        }));
    }
    for h in hs {
        h.join().unwrap();
    }
    eprintln!("puts done");
    let proc = client.require_proc();
    pmix::commit().expect("commit");
    eprintln!("commit done");
    pmix::fence(&proc, None).expect("fence");
    eprintln!("fence done");
    // PMIx_Progress may block, so do not join the progress thread.
    stop.store(true, Ordering::Release);
    // Give the loop a moment to observe stop between Progress calls.
    thread::sleep(Duration::from_millis(50));
    match client.disconnect(None) {
        Ok(()) => eprintln!("disconnect ok"),
        Err(e) => eprintln!("disconnect: {e:?}"),
    }
    eprintln!("ok");
}
