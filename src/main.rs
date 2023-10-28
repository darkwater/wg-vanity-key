use std::{
    env,
    mem::MaybeUninit,
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::Duration,
};

use base64::{
    alphabet,
    engine::{GeneralPurpose, GeneralPurposeConfig},
    Engine,
};
use core_affinity::CoreId;
use nacl::public_box::KEY_LENGTH;

static GENERATED: AtomicUsize = AtomicUsize::new(0);

const BASE64: GeneralPurpose =
    GeneralPurpose::new(&alphabet::STANDARD, GeneralPurposeConfig::new());

const KEY_B64_LENGTH: usize = 44;

fn main() {
    let mut args = env::args().skip(1).collect::<Vec<_>>();

    let (tx, rx) = std::sync::mpsc::channel::<[[u8; KEY_LENGTH]; 2]>();

    thread::spawn(move || loop {
        core_affinity::set_for_current(CoreId { id: 0 });
        thread::sleep(Duration::from_secs(1));
        let cycles = GENERATED.swap(0, Ordering::Relaxed);
        eprintln!("{cycles} cycles/s");
    });

    let checker = thread::spawn(move || {
        core_affinity::set_for_current(CoreId { id: 0 });
        let mut pk64 = [0u8; KEY_B64_LENGTH];
        for [sk, pk] in rx.iter() {
            let _ = BASE64.encode_slice(&pk, &mut pk64);

            args.retain(|s| {
                let retain = !pk64.starts_with(s.as_bytes());
                if !retain {
                    eprintln!("found one! {}", String::from_utf8(pk64.to_vec()).unwrap());
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

    let _generators = core_affinity::get_core_ids()
        .unwrap()
        .into_iter()
        .map(|core_id| {
            let tx = tx.clone();
            thread::spawn(move || {
                core_affinity::set_for_current(core_id);

                loop {
                    let sk = rand::random::<[u8; KEY_LENGTH]>();
                    let pk = unsafe { nacl::public_box::generate_pubkey(&sk).unwrap_unchecked() };

                    let pk_array = MaybeUninit::uninit();
                    unsafe {
                        std::ptr::copy(pk.as_ptr(), pk_array.as_ptr() as *mut u8, KEY_LENGTH);
                    }

                    let _ = tx.send([sk, unsafe {
                        std::mem::transmute::<MaybeUninit<[u8; KEY_LENGTH]>, [u8; KEY_LENGTH]>(
                            pk_array,
                        )
                    }]);

                    GENERATED.fetch_add(1, Ordering::Relaxed);
                }
            });
        })
        .collect::<Vec<_>>();

    checker.join().unwrap();
}
