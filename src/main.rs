#![feature(portable_simd)]
#![feature(test)]

use std::{
    env,
    simd::{u8x8, SimdPartialEq},
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
use rayon::prelude::*;

extern crate test;

static GENERATED: AtomicUsize = AtomicUsize::new(0);
static CHECKED: AtomicUsize = AtomicUsize::new(0);

const BASE64: GeneralPurpose =
    GeneralPurpose::new(&alphabet::STANDARD, GeneralPurposeConfig::new());

const KEY_B64_LENGTH: usize = 44;

fn main() {
    let mut args = env::args().skip(1).collect::<Vec<_>>();

    let (tx, rx) = std::sync::mpsc::channel::<[u8; KEY_LENGTH]>();

    thread::spawn(move || loop {
        core_affinity::set_for_current(CoreId { id: 0 });
        thread::sleep(Duration::from_secs(1));
        let cycles = GENERATED.swap(0, Ordering::Relaxed);
        let checks = CHECKED.swap(0, Ordering::Relaxed);
        println!("{cycles} cycles/s, {checks} checks/s");
    });

    thread::spawn(move || {
        core_affinity::set_for_current(CoreId { id: 0 });
        let mut pk64 = [0u8; KEY_B64_LENGTH];
        for pk in rx.iter() {
            let _ = BASE64.encode_slice(&pk, &mut pk64);

            args.retain(|s| {
                let retain = !pk64.starts_with(s.as_bytes());
                if !retain {
                    println!("{}: {}", s, String::from_utf8(pk64.to_vec()).unwrap());
                }
                retain
            });
            CHECKED.fetch_add(1, Ordering::Relaxed);

            if args.is_empty() {
                println!("All keys found!");
                std::process::exit(0);
            }
        }
    });

    std::iter::from_fn(|| Some(rand::random::<[u8; KEY_LENGTH]>()))
        .par_bridge()
        // .progress_with(ProgressBar::new_spinner())
        .for_each(move |sk| {
            GENERATED.fetch_add(1, Ordering::Relaxed);
            let pk = unsafe { nacl::public_box::generate_pubkey(&sk).unwrap_unchecked() };
            tx.send(unsafe { pk.try_into().unwrap_unchecked() })
                .unwrap();
        });
}

#[derive(Debug)]
struct SearchResult<'a> {
    prefix: &'a str,
    sk: String,
}

const FIRST_CHAR_COUNT: usize = 2;
struct SearchState<'a> {
    matches: &'a [&'a str],
    first_chars: [u8x8; FIRST_CHAR_COUNT],
    sk64: [u8; KEY_B64_LENGTH],
    pk64: [u8; 32 * 4 / 3 + 2],
}

impl<'a> SearchState<'a> {
    fn new(matches: &'a [&'a str]) -> Self {
        let first_chars = (0..FIRST_CHAR_COUNT)
            .map(|n| {
                u8x8::from_slice(
                    &matches
                        .iter()
                        .map(|s| s.as_bytes()[n])
                        .chain([0; 8].into_iter())
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        Self {
            matches,
            first_chars,
            sk64: [0u8; 32 * 4 / 3 + 2],
            pk64: [0u8; 32 * 4 / 3 + 2],
        }
    }
}

fn find_key<'a>(matches: &'a [&'a str]) -> SearchResult<'a> {
    let mut state = SearchState::new(matches);

    loop {
        GENERATED.fetch_add(1, Ordering::Relaxed);

        let result = search_step(&mut state);

        if let Some(s) = result {
            return SearchResult {
                prefix: s,
                sk: String::from_utf8(state.sk64.to_vec()).unwrap(),
            };
        }
    }
}

#[inline(always)]
fn search_step<'a>(state: &mut SearchState<'a>) -> Option<&'a str> {
    let sk = rand::random::<[u8; KEY_LENGTH]>();
    let pk = unsafe { nacl::public_box::generate_pubkey(&sk).unwrap_unchecked() };

    let _ = BASE64.encode_slice(&sk, &mut state.sk64);
    let _ = BASE64.encode_slice(&pk, &mut state.pk64);

    let first = u8x8::splat(state.pk64[0]);
    let comp = first.simd_eq(state.first_chars[0]);

    if comp.any() {
        state
            .matches
            .iter()
            .find(|s| state.pk64.starts_with(s.as_bytes()))
            .copied()
    } else {
        None
    }
}

#[inline(always)]
fn compare<'a, 'b>(pk: &'a [u8; KEY_B64_LENGTH], state: &SearchState<'b>) -> Option<&'b str> {
    let first = u8x8::splat(pk[0]);
    let comp = first.simd_eq(state.first_chars[0]);

    if comp.any() {
        state
            .matches
            .iter()
            .find(|s| state.pk64.starts_with(s.as_bytes()))
            .copied()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use test::Bencher;

    use super::*;

    #[bench]
    fn bench_search_step(b: &mut Bencher) {
        let matches = &["a+", "b+", "c+", "d+", "e+"];

        let mut state = SearchState::new(matches);

        b.iter(|| {
            let result = search_step(&mut state);

            if let Some(s) = result {
                test::black_box(s);
            }
        });
    }

    #[bench]
    fn bench_generate(b: &mut Bencher) {
        b.iter(|| {
            test::black_box(rand::random::<[u8; nacl::public_box::KEY_LENGTH]>());
        });
    }

    #[bench]
    fn bench_pubkey(b: &mut Bencher) {
        let sk = rand::random::<[u8; nacl::public_box::KEY_LENGTH]>();

        b.iter(|| {
            let _ = test::black_box(nacl::public_box::generate_pubkey(&sk));
        });
    }

    #[bench]
    fn bench_compare(b: &mut Bencher) {
        let matches = &["a+", "b+", "c+", "d+", "e+"];

        let state = SearchState::new(matches);

        b.iter(|| {
            test::black_box(compare(test::black_box(&[0u8; KEY_B64_LENGTH]), &state));
        });
    }
}
