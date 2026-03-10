use serde::{Deserialize, Serialize};

use crate::math_error::MathError;

pub const TAU_TICKS_DEFAULT: u16 = 4096;

/// Integer phase ring for additive-only kernels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseTicks {
    tick: u16,
    tau_ticks: u16,
}

impl PhaseTicks {
    pub fn new(tick: u16, tau_ticks: u16) -> Result<Self, MathError> {
        if tau_ticks == 0 {
            return Err(MathError::InvalidConfig("tau_ticks must be > 0"));
        }
        Ok(Self {
            tick: tick % tau_ticks,
            tau_ticks,
        })
    }

    #[inline]
    pub fn tick(self) -> u16 {
        self.tick
    }

    #[inline]
    pub fn tau_ticks(self) -> u16 {
        self.tau_ticks
    }

    pub fn add_ticks(self, delta: u16) -> Self {
        let mut t = self.tick;
        let mut d = delta;
        while d > 0 {
            t = t.wrapping_add(1);
            if t >= self.tau_ticks {
                t = 0;
            }
            d -= 1;
        }
        Self {
            tick: t,
            tau_ticks: self.tau_ticks,
        }
    }

    pub fn shortest_arc(self, other: Self) -> Result<u16, MathError> {
        if self.tau_ticks != other.tau_ticks {
            return Err(MathError::DomainViolation("mismatched tau domain"));
        }

        let mut forward = 0u16;
        let mut t = self.tick;
        while t != other.tick {
            t = t.wrapping_add(1);
            if t >= self.tau_ticks {
                t = 0;
            }
            forward = forward.wrapping_add(1);
        }

        let mut backward = 0u16;
        let mut u = self.tick;
        while u != other.tick {
            if u == 0 {
                u = self.tau_ticks - 1;
            } else {
                u -= 1;
            }
            backward = backward.wrapping_add(1);
        }

        Ok(forward.min(backward))
    }
}

/// Integer accumulator that distributes quanta over repeated steps
/// without division in the hot path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemainderAccumulator {
    carry: i64,
    slots: i64,
}

impl RemainderAccumulator {
    pub fn new(slots: i64) -> Result<Self, MathError> {
        if slots <= 0 {
            return Err(MathError::InvalidConfig("slots must be > 0"));
        }
        Ok(Self { carry: 0, slots })
    }

    #[inline]
    pub fn carry(self) -> i64 {
        self.carry
    }

    #[inline]
    pub fn slots(self) -> i64 {
        self.slots
    }

    /// Pushes integer input and returns integer output amount for this step.
    /// Uses only addition/subtraction/comparison in the update path.
    pub fn push(&mut self, input: i64) -> i64 {
        self.carry += input;
        let mut out = 0i64;
        while self.carry >= self.slots {
            self.carry -= self.slots;
            out += 1;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_ticks_wrap_additively() {
        let p = PhaseTicks::new(7, 8).expect("valid phase");
        let q = p.add_ticks(3);
        assert_eq!(q.tick(), 2);
    }

    #[test]
    fn shortest_arc_is_symmetric() {
        let a = PhaseTicks::new(1, 16).expect("valid phase");
        let b = PhaseTicks::new(15, 16).expect("valid phase");
        let dab = a.shortest_arc(b).expect("same domain");
        let dba = b.shortest_arc(a).expect("same domain");
        assert_eq!(dab, 2);
        assert_eq!(dab, dba);
    }

    #[test]
    fn remainder_accumulator_distributes_without_division() {
        let mut acc = RemainderAccumulator::new(3).expect("valid slots");
        let mut out = 0;
        for _ in 0..10 {
            out += acc.push(1);
        }
        assert_eq!(out, 3);
        assert_eq!(acc.carry(), 1);
    }
}
