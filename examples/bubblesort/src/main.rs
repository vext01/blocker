extern crate rand;
use rand::{Rng, SeedableRng, StdRng};

fn main() {
    bench(10000);
}

fn bench(param: usize) {
    let mut v = Vec::with_capacity(param);
    let vlen = param;

    // seed rng
    let seed: &[_] = &[666, 1337, 42, 13];
    let mut rng: StdRng = SeedableRng::from_seed(seed);

    let mut done_in: usize = 0;

    for _ in 0..param {
        v.push(rng.next_u32());
    }

    loop {
        let mut swapped = false;
        for i in 0..vlen - 1 {
            let el1 = v[i];
            let el2 = v[i+1];
            if el2 < el1 {
                v[i] = el2;
                v[i+1] = el1;
                swapped = true;
            }
        }
        done_in += 1;
        if !swapped {
            break;
        }
    }
    println!("done in {}", done_in);
}

