use pi_core::{MathError, PhaseTicks, RemainderAccumulator, TAU_TICKS_DEFAULT};

/// Additive-only recursive branching kernel.
///
/// Hot-path updates avoid multiplication/division and use only
/// addition/subtraction/comparison plus indexing.
#[derive(Debug, Clone)]
pub struct AdditiveFractalKernel {
    levels: Vec<Vec<i64>>,
    split_toggle: bool,
    root_feedback: RemainderAccumulator,
    phase: PhaseTicks,
}

impl AdditiveFractalKernel {
    pub fn new(depth: usize) -> Result<Self, MathError> {
        if depth < 2 {
            return Err(MathError::InvalidConfig("depth must be >= 2"));
        }

        let mut levels = Vec::with_capacity(depth);
        let mut width = 1usize;
        for _ in 0..depth {
            levels.push(vec![0i64; width]);
            width += width;
        }

        Ok(Self {
            levels,
            split_toggle: false,
            root_feedback: RemainderAccumulator::new(4)?,
            phase: PhaseTicks::new(0, TAU_TICKS_DEFAULT)?,
        })
    }

    #[inline]
    pub fn depth(&self) -> usize {
        self.levels.len()
    }

    #[inline]
    pub fn levels(&self) -> &[Vec<i64>] {
        &self.levels
    }

    #[inline]
    pub fn phase(&self) -> PhaseTicks {
        self.phase
    }

    pub fn seed_root(&mut self, amount: i64) {
        if amount > 0 {
            self.levels[0][0] += amount;
        }
    }

    pub fn total_quanta(&self) -> i64 {
        let mut total = 0i64;
        for level in &self.levels {
            for &q in level {
                total += q;
            }
        }
        total
    }

    pub fn step(&mut self) {
        let depth = self.levels.len();

        let mut next = self.levels.clone();
        for layer in &mut next {
            layer.fill(0);
        }

        for lvl in 0..(depth - 1) {
            let this_len = self.levels[lvl].len();
            let mut child_base = 0usize;
            for idx in 0..this_len {
                let mut q = self.levels[lvl][idx];
                while q > 0 {
                    if self.split_toggle {
                        next[lvl + 1][child_base] += 1;
                    } else {
                        next[lvl + 1][child_base + 1] += 1;
                    }
                    self.split_toggle = !self.split_toggle;
                    q -= 1;
                }
                child_base += 2;
            }
        }

        let mut deepest = 0i64;
        let last_idx = depth - 1;
        for q in &mut next[last_idx] {
            deepest += *q;
            *q = 0;
        }

        let feedback = self.root_feedback.push(deepest);
        if feedback > 0 {
            next[0][0] += feedback;
        }

        self.levels = next;
        self.phase = self.phase.add_ticks(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn additive_kernel_conserves_with_feedback_sink() {
        let mut k = AdditiveFractalKernel::new(5).expect("valid kernel");
        k.seed_root(64);

        let total0 = k.total_quanta();
        for _ in 0..50 {
            k.step();
        }
        let total1 = k.total_quanta();

        assert!(total1 <= total0);
        assert!(total1 >= 0);
    }

    #[test]
    fn recursive_levels_receive_flow() {
        let mut k = AdditiveFractalKernel::new(4).expect("valid kernel");
        k.seed_root(12);
        k.step();

        let mut lvl1_total = 0;
        for &q in &k.levels()[1] {
            lvl1_total += q;
        }
        assert_eq!(lvl1_total, 12);
    }
}
