use std::{
    env,
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};

use base64::{
    alphabet,
    engine::{GeneralPurpose, GeneralPurposeConfig},
    Engine,
};
use core_affinity::CoreId;
use rand_core::OsRng;
use x25519_dalek_fiat::{PublicKey, StaticSecret};

static GENERATED: AtomicUsize = AtomicUsize::new(0);

const BASE64: GeneralPurpose =
    GeneralPurpose::new(&alphabet::STANDARD, GeneralPurposeConfig::new());

const KEY_LENGTH: usize = 32;
const KEY_B64_LENGTH: usize = 44;

fn main() {
    let mut args = env::args().skip(1).collect::<Vec<_>>();

    let (tx, rx) = std::sync::mpsc::channel::<[[u8; KEY_LENGTH]; 2]>();

    let start = Instant::now();

    thread::spawn(move || loop {
        core_affinity::set_for_current(CoreId { id: 0 });
        thread::sleep(Duration::from_secs(60));
        let cycles = GENERATED.swap(0, Ordering::Relaxed);
        eprintln!("{cycles} cycles/min");
    });

    let checker = thread::spawn(move || {
        core_affinity::set_for_current(CoreId { id: 0 });
        let mut pk64 = [0u8; KEY_B64_LENGTH];
        for [sk, pk] in rx {
            let _ = BASE64.encode_slice(pk, &mut pk64);

            args.retain(|s| {
                let retain = !pk64.starts_with(s.as_bytes());
                if !retain {
                    eprintln!(
                        "Found one! {} (after {:.1} minutes)",
                        String::from_utf8(pk64.to_vec()).unwrap(),
                        start.elapsed().as_secs_f64() / 60.,
                    );
                    println!(
                        "sk: {} pk: {}",
                        BASE64.encode(sk),
                        String::from_utf8(pk64.to_vec()).unwrap()
                    );
                }
                retain
            });

            if args.is_empty() {
                eprintln!("All keys found!");
                break;
            }
        }
    });

    let generators = core_affinity::get_core_ids()
        .unwrap()
        .into_iter()
        .map(|core_id| {
            let tx = tx.clone();
            thread::spawn(move || {
                core_affinity::set_for_current(core_id);

                loop {
                    let sk = StaticSecret::new(OsRng);
                    let pk = PublicKey::from(&sk);

                    let _ = tx.send([sk.to_bytes(), pk.to_bytes()]);

                    GENERATED.fetch_add(1, Ordering::Relaxed);
                }
            });
        })
        .collect::<Vec<_>>();

    eprintln!("Spawned {} threads...", generators.len());

    checker.join().unwrap();
}
