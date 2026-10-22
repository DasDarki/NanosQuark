use std::f64::consts::TAU;

#[derive(Clone)]
pub struct Rng {
    state: [u64; 4],
    seed: u64,
}

fn split_mix(value: &mut u64) -> u64 {
    *value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut mixed = *value;
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    mixed ^ (mixed >> 31)
}

pub fn seed_from_text(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

pub fn random_seed() -> u64 {
    getrandom::u64().unwrap_or_else(|_| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos() as u64)
            .unwrap_or(0x5eed)
    })
}

fn ln_gamma(value: f64) -> f64 {
    const COEFFICIENTS: [f64; 6] = [
        76.180_091_729_471_46,
        -86.505_320_329_416_77,
        24.014_098_240_830_91,
        -1.231_739_572_450_155,
        0.001_208_650_973_866_179,
        -0.000_005_395_239_384_953,
    ];
    let mut denominator = value;
    let temporary = value + 5.5;
    let temporary = temporary - (value + 0.5) * temporary.ln();
    let mut series = 1.000_000_000_190_015;
    for coefficient in COEFFICIENTS {
        denominator += 1.0;
        series += coefficient / denominator;
    }
    -temporary + (2.506_628_274_631_000_5 * series / value).ln()
}

impl Rng {
    pub fn from_seed(seed: u64) -> Self {
        let mut mixer = seed;
        let mut state = [0; 4];
        for slot in &mut state {
            *slot = split_mix(&mut mixer);
        }
        if state == [0; 4] {
            state[0] = 1;
        }
        Rng { state, seed }
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn state_text(&self) -> String {
        self.state
            .iter()
            .map(|part| format!("{part:016x}"))
            .collect()
    }

    pub fn set_state_text(&mut self, text: &str) -> Result<(), String> {
        if text.len() != 64 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("a generator state is 64 hexadecimal characters".to_owned());
        }
        let mut state = [0; 4];
        for (index, slot) in state.iter_mut().enumerate() {
            *slot = u64::from_str_radix(&text[index * 16..index * 16 + 16], 16)
                .map_err(|error| error.to_string())?;
        }
        if state == [0; 4] {
            return Err("the generator state must not be all zeros".to_owned());
        }
        self.state = state;
        Ok(())
    }

    pub fn next_u64(&mut self) -> u64 {
        let result = self.state[0]
            .wrapping_add(self.state[3])
            .rotate_left(23)
            .wrapping_add(self.state[0]);
        let shifted = self.state[1] << 17;
        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= shifted;
        self.state[3] = self.state[3].rotate_left(45);
        result
    }

    pub fn float(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    pub fn below(&mut self, bound: u64) -> u64 {
        let mut product = u128::from(self.next_u64()) * u128::from(bound);
        let mut low = product as u64;
        if low < bound {
            let threshold = bound.wrapping_neg() % bound;
            while low < threshold {
                product = u128::from(self.next_u64()) * u128::from(bound);
                low = product as u64;
            }
        }
        (product >> 64) as u64
    }

    pub fn int(&mut self, minimum: i64, maximum: i64) -> i64 {
        let span = (i128::from(maximum) - i128::from(minimum) + 1) as u128;
        if span > u128::from(u64::MAX) {
            return self.next_u64() as i64;
        }
        (i128::from(minimum) + i128::from(self.below(span as u64))) as i64
    }

    pub fn range(&mut self, minimum: f64, maximum: f64) -> f64 {
        minimum + self.float() * (maximum - minimum)
    }

    pub fn normal(&mut self, mean: f64, deviation: f64) -> f64 {
        loop {
            let u = self.float() * 2.0 - 1.0;
            let v = self.float() * 2.0 - 1.0;
            let squared = u * u + v * v;
            if squared > 0.0 && squared < 1.0 {
                return mean + deviation * u * (-2.0 * squared.ln() / squared).sqrt();
            }
        }
    }

    pub fn exponential(&mut self, rate: f64) -> f64 {
        -(1.0 - self.float()).ln() / rate
    }

    pub fn triangular(&mut self, minimum: f64, maximum: f64, mode: f64) -> f64 {
        let width = maximum - minimum;
        if width <= 0.0 {
            return minimum;
        }
        let sample = self.float();
        let split = (mode - minimum) / width;
        if sample < split {
            minimum + (sample * width * (mode - minimum)).sqrt()
        } else {
            maximum - ((1.0 - sample) * width * (maximum - mode)).sqrt()
        }
    }

    pub fn poisson(&mut self, mean: f64) -> i64 {
        if mean <= 0.0 {
            return 0;
        }
        if mean < 10.0 {
            let limit = (-mean).exp();
            let mut count = 0;
            let mut product = self.float();
            while product > limit {
                count += 1;
                product *= self.float();
            }
            return count;
        }

        let root = mean.sqrt();
        let logarithm = mean.ln();
        let b = 0.931 + 2.53 * root;
        let a = -0.059 + 0.02483 * b;
        let inverse_alpha = 1.1239 + 1.1328 / (b - 3.4);
        let ratio = 0.9277 - 3.6224 / (b - 2.0);
        loop {
            let u = self.float() - 0.5;
            let v = self.float();
            let distance = 0.5 - u.abs();
            let count = ((2.0 * a / distance + b) * u + mean + 0.43).floor();
            if distance >= 0.07 && v <= ratio {
                return count as i64;
            }
            if count < 0.0 || (distance < 0.013 && v > distance) {
                continue;
            }
            let left = v.ln() + inverse_alpha.ln() - (a / (distance * distance) + b).ln();
            let right = -mean + count * logarithm - ln_gamma(count + 1.0);
            if left <= right {
                return count as i64;
            }
        }
    }

    pub fn inside_circle(&mut self, radius: f64) -> (f64, f64) {
        let distance = radius * self.float().sqrt();
        let angle = TAU * self.float();
        (distance * angle.cos(), distance * angle.sin())
    }

    pub fn on_sphere(&mut self, radius: f64) -> (f64, f64, f64) {
        let z = self.float() * 2.0 - 1.0;
        let angle = TAU * self.float();
        let ring = (1.0 - z * z).sqrt();
        (
            radius * ring * angle.cos(),
            radius * ring * angle.sin(),
            radius * z,
        )
    }

    pub fn inside_sphere(&mut self, radius: f64) -> (f64, f64, f64) {
        let (x, y, z) = self.on_sphere(1.0);
        let distance = radius * self.float().cbrt();
        (x * distance, y * distance, z * distance)
    }
}
