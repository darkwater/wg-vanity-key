fn add(out: &mut [u32], a: &[u32], b: &[u32]) {
    let mut u: u32 = 0;
    for j in 0..31 {
        u += a[j] + b[j];
        out[j] = u & 255;
        u >>= 8;
    }
    u += a[31] + b[31];
    out[31] = u;
}

fn sub(out: &mut [u32], a: &[u32], b: &[u32]) {
    let mut u: u32 = 218;
    for j in 0..31 {
        u += a[j] + 65280 - b[j];
        out[j] = u & 255;
        u >>= 8;
    }
    incr!(u, subw!(a[31], b[31]));
    out[31] = u;
}

fn squeeze(a: &mut [u32]) {
    let mut u: u32 = 0;
    for j in 0..31 {
        u += a[j];
        a[j] = u & 255;
        u >>= 8;
    }
    u += a[31];
    a[31] = u & 127;
    u = 19 * (u >> 7);
    for j in 0..31 {
        u += a[j];
        a[j] = u & 255;
        u >>= 8;
    }
    u += a[31];
    a[31] = u;
}

static MINUSP: [u32; 32] = [
    19, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    128,
];

fn freeze(a: &mut [u32]) {
    let mut aorig: [u32; 32] = [0; 32];
    for j in 0..32 {
        aorig[j] = a[j];
    }
    add(a, &aorig, &MINUSP);
    let negative = -(((a[31] >> 7) & 1) as i32) as u32;
    for j in 0..32 {
        a[j] ^= negative & (aorig[j] ^ a[j]);
    }
}

fn mult(out: &mut [u32], a: &[u32], b: &[u32]) {
    for i in 0..32 {
        let mut u: u32 = 0;
        for j in 0..i + 1 {
            u += a[j] * b[i - j];
        }
        for j in i + 1..32 {
            u += 38 * a[j] * b[i + 32 - j]
        }
        out[i] = u;
    }
    squeeze(out);
}

fn mult121665(out: &mut [u32], a: &[u32]) {
    let mut u: u32 = 0;
    for j in 0..31 {
        u += 121665 * a[j];
        out[j] = u & 255;
        u >>= 8;
    }
    u += 121665 * a[31];
    out[31] = u & 127;
    u = 19 * (u >> 7);
    for j in 0..31 {
        u += out[j];
        out[j] = u & 255;
        u >>= 8;
    }
    u += out[31];
    out[31] = u;
}

fn square(out: &mut [u32], a: &[u32]) {
    for i in 0..32 {
        let mut u: u32 = 0;
        let mut j: usize = 0;
        while j < (i - j) {
            u += a[j] * a[i - j];
            j += 1;
        }
        j = i + 1;
        while j < (i + 32 - j) {
            u += 38 * a[j] * a[i + 32 - j];
            j += 1;
        }
        u *= 2;
        if (i & 1) == 0 {
            u += a[i / 2] * a[i / 2];
            u += 38 * a[i / 2 + 16] * a[i / 2 + 16];
        }
        out[i] = u;
    }
    squeeze(out);
}

fn select(p: &mut [u32], q: &mut [u32], r: &[u32], s: &[u32], b: u32) {
    let bminus1 = subw!(b, 1 as u32);
    for j in 0..64 {
        let t = bminus1 & (r[j] ^ s[j]);
        p[j] = s[j] ^ t;
        q[j] = r[j] ^ t;
    }
}

fn mainloop(work1: &mut [u32], work2: &mut [u32], e: &[u32]) {
    let mut xzm1: [u32; 64] = [0; 64];
    let mut xzm: [u32; 64] = [0; 64];
    let mut xzmb: [u32; 64] = [0; 64];
    let mut xzm1b: [u32; 64] = [0; 64];
    let mut xznb: [u32; 64] = [0; 64];
    let mut xzn1b: [u32; 64] = [0; 64];
    let mut a0: [u32; 64] = [0; 64];
    let mut a1: [u32; 64] = [0; 64];
    let mut b0: [u32; 64] = [0; 64];
    let mut b1: [u32; 64] = [0; 64];
    let mut c1: [u32; 64] = [0; 64];
    let mut r: [u32; 32] = [0; 32];
    let mut s: [u32; 32] = [0; 32];
    let mut t: [u32; 32] = [0; 32];
    let mut u: [u32; 32] = [0; 32];

    xzm1[0..32].copy_from_slice(work1);
    xzm1[32] = 1;

    xzm[0] = 1;
    for j in 1..64 {
        xzm[j] = 0;
    }

    let mut pos: usize = 254;
    loop {
        let mut b = e[pos / 8] >> (pos & 7);
        b &= 1;
        select(&mut xzmb, &mut xzm1b, &xzm, &xzm1, b);
        add(&mut a0[0..32], &xzmb[0..32], &xzmb[32..]);
        sub(&mut a0[32..], &xzmb[0..32], &xzmb[32..]);
        add(&mut a1[0..32], &xzm1b[0..32], &xzm1b[32..]);
        sub(&mut a1[32..], &xzm1b[0..32], &xzm1b[32..]);
        square(&mut b0[0..32], &a0[0..32]);
        square(&mut b0[32..], &a0[32..]);
        mult(&mut b1[0..32], &a1[0..32], &a0[32..]);
        mult(&mut b1[32..], &a1[32..], &a0[0..32]);
        add(&mut c1[0..32], &b1[0..32], &b1[32..]);
        sub(&mut c1[32..], &b1[0..32], &b1[32..]);
        square(&mut r[0..32], &c1[32..]);
        sub(&mut s[0..32], &b0[0..32], &b0[32..]);
        mult121665(&mut t[0..32], &s[0..32]);
        add(&mut u[0..32], &t[0..32], &b0[0..32]);
        mult(&mut xznb[0..32], &b0[0..32], &b0[32..]);
        mult(&mut xznb[32..], &s[0..32], &u[0..32]);
        square(&mut xzn1b[0..32], &c1[0..32]);
        mult(&mut xzn1b[32..], &r[0..32], &work1);
        select(&mut xzm, &mut xzm1, &xznb, &xzn1b, b);
        if pos == 0 {
            break;
        }
        pos -= 1;
    }

    work1.copy_from_slice(&xzm[0..32]);
    work2.copy_from_slice(&xzm[32..64]);
}

fn recip(out: &mut [u32], z: &[u32]) {
    let mut z2: [u32; 32] = [0; 32];
    let mut z9: [u32; 32] = [0; 32];
    let mut z11: [u32; 32] = [0; 32];
    let mut z2_5_0: [u32; 32] = [0; 32];
    let mut z2_10_0: [u32; 32] = [0; 32];
    let mut z2_20_0: [u32; 32] = [0; 32];
    let mut z2_50_0: [u32; 32] = [0; 32];
    let mut z2_100_0: [u32; 32] = [0; 32];
    let mut t0: [u32; 32] = [0; 32];
    let mut t1: [u32; 32] = [0; 32];

    /* 2 */
    square(&mut z2, &z);
    /* 4 */
    square(&mut t1, &z2);
    /* 8 */
    square(&mut t0, &t1);
    /* 9 */
    mult(&mut z9, &t0, &z);
    /* 11 */
    mult(&mut z11, &z9, &z2);
    /* 22 */
    square(&mut t0, &z11);
    /* 2^5 - 2^0 = 31 */
    mult(&mut z2_5_0, &t0, &z9);

    /* 2^6 - 2^1 */
    square(&mut t0, &z2_5_0);
    /* 2^7 - 2^2 */
    square(&mut t1, &t0);
    /* 2^8 - 2^3 */
    square(&mut t0, &t1);
    /* 2^9 - 2^4 */
    square(&mut t1, &t0);
    /* 2^10 - 2^5 */
    square(&mut t0, &t1);
    /* 2^10 - 2^0 */
    mult(&mut z2_10_0, &t0, &z2_5_0);

    /* 2^11 - 2^1 */
    square(&mut t0, &z2_10_0);
    /* 2^12 - 2^2 */
    square(&mut t1, &t0);
    /* 2^20 - 2^10 */
    for _ in 1..5 {
        square(&mut t0, &t1);
        square(&mut t1, &t0);
    }
    /* 2^20 - 2^0 */
    mult(&mut z2_20_0, &t1, &z2_10_0);

    /* 2^21 - 2^1 */
    square(&mut t0, &z2_20_0);
    /* 2^22 - 2^2 */
    square(&mut t1, &t0);
    /* 2^40 - 2^20 */
    for _ in 1..10 {
        square(&mut t0, &t1);
        square(&mut t1, &t0);
    }
    /* 2^40 - 2^0 */
    mult(&mut t0, &t1, &z2_20_0);

    /* 2^41 - 2^1 */
    square(&mut t1, &t0);
    /* 2^42 - 2^2 */
    square(&mut t0, &t1);
    /* 2^50 - 2^10 */
    for _ in 1..5 {
        square(&mut t1, &t0);
        square(&mut t0, &t1);
    }
    /* 2^50 - 2^0 */
    mult(&mut z2_50_0, &t0, &z2_10_0);

    /* 2^51 - 2^1 */
    square(&mut t0, &z2_50_0);
    /* 2^52 - 2^2 */
    square(&mut t1, &t0);
    /* 2^100 - 2^50 */
    for _ in 1..25 {
        square(&mut t0, &t1);
        square(&mut t1, &t0);
    }
    /* 2^100 - 2^0 */
    mult(&mut z2_100_0, &t1, &z2_50_0);

    /* 2^101 - 2^1 */
    square(&mut t1, &z2_100_0);
    /* 2^102 - 2^2 */
    square(&mut t0, &t1);
    /* 2^200 - 2^100 */
    for _ in 1..50 {
        square(&mut t1, &t0);
        square(&mut t0, &t1);
    }
    /* 2^200 - 2^0 */
    mult(&mut t1, &t0, &z2_100_0);

    /* 2^201 - 2^1 */
    square(&mut t0, &t1);
    /* 2^202 - 2^2 */
    square(&mut t1, &t0);
    /* 2^250 - 2^50 */
    for _ in 1..25 {
        square(&mut t0, &t1);
        square(&mut t1, &t0);
    }
    /* 2^250 - 2^0 */
    mult(&mut t0, &t1, &z2_50_0);

    /* 2^251 - 2^1 */
    square(&mut t1, &t0);
    /* 2^252 - 2^2 */
    square(&mut t0, &t1);
    /* 2^253 - 2^3 */
    square(&mut t1, &t0);
    /* 2^254 - 2^4 */
    square(&mut t0, &t1);
    /* 2^255 - 2^5 */
    square(&mut t1, &t0);
    /* 2^255 - 21 */
    mult(out, &t1, &z11);
}

pub fn curve25519(q: &mut [u8], n: &[u8], p: &[u8]) {
    let mut work1: [u32; 32] = [0; 32];
    let mut work2: [u32; 32] = [0; 32];
    let mut work3: [u32; 32] = [0; 32];
    let mut e: [u32; 32] = [0; 32];
    for i in 0..32 {
        e[i] = n[i] as u32;
    }
    e[0] &= 248;
    e[31] &= 127;
    e[31] |= 64;
    for i in 0..32 {
        work1[i] = p[i] as u32;
    }
    mainloop(&mut work1, &mut work2, &e);
    let mut t_work2: [u32; 32] = [0; 32];
    t_work2.copy_from_slice(&work2);
    recip(&mut work2, &t_work2);
    mult(&mut work3, &work1, &work2);
    freeze(&mut work3);
    for i in 0..32 {
        q[i] = work3[i] as u8;
    }
}

const BASE: [u8; 32] = [
    9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

pub fn curve25519_base(q: &mut [u8], n: &[u8]) {
    return curve25519(q, n, &BASE);
}
