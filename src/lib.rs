#![no_std]

use core::ops::{self, Add, Sub};

use atsamd_hal::{
    clock::GenericClockController,
    target_device::{PM, TC4, TC5},
};
use rtic::Monotonic as _;
pub struct FusedTimerCounter<T1, T2> {
    t1: T1,
    _t2: T2,
}

impl FusedTimerCounter<TC4, TC5> {
    pub fn initialize(
        tc4: TC4,
        tc5: TC5,
        gclk: &mut GenericClockController,
        pm: &mut PM,
    ) -> &'static Self {
        let gclk0 = gclk.gclk0();
        let tc45 = gclk.tc4_tc5(&gclk0).expect("tc4_tc5 already initialized");

        pm.apbcmask.modify(|_, w| w.tc4_().set_bit());
        pm.apbcmask.modify(|_, w| w.tc5_().set_bit()); // don't know if this is necessary

        let count = tc4.count32();

        // Disable the timer while we reconfigure it
        count.ctrla.modify(|_, w| w.enable().clear_bit());
        while count.status.read().syncbusy().bit_is_set() {}

        // Now that we have a clock routed to the peripheral, we
        // can ask it to perform a reset.
        count.ctrla.write(|w| w.swrst().set_bit());
        while count.status.read().syncbusy().bit_is_set() {}
        // the SVD erroneously marks swrst as write-only, so we
        // need to manually read the bit here
        while count.ctrla.read().bits() & 1 != 0 {}

        count.ctrlbset.write(|w| {
            // Count up when the direction bit is zero
            w.dir().clear_bit();
            // Periodic
            w.oneshot().clear_bit()
        });

        // not sure if this is also necessary, but probably?
        count.readreq.modify(|_, w| w.rcont().set_bit());

        // Set TOP value for mfrq mode
        count.cc[0].write(|w| unsafe { w.cc().bits(46875) }); // continue counting and wrapping around u32::MAX

        count.ctrla.modify(|_, w| {
            w.mode().count32();
            w.wavegen().mfrq();
            w.prescaler().div1024(); // divides CPU clock speed
            w.enable().set_bit()
        });

        while count.status.read().syncbusy().bit_is_set() {}

        while tc5.count32().status.read().syncbusy().bit_is_set() {} // don't know if this is necessary

        // Tests if the 32 bit mode was actually enabled
        if !tc5.count32().status.read().slave().bit_is_set() {
            panic!("32 bit mode didn't work for fused counter");
        }

        let counter = Self { t1: tc4, _t2: tc5 };

        unsafe {
            MONOTONIC_TIMER = Some(counter);
            MONOTONIC_TIMER.as_ref().unwrap()
        }
    }

    pub fn overflowed(&self) -> bool {
        self.t1.count32().intflag.read().ovf().bit_is_set()
    }

    pub fn reset_ovf_flag(&self) {
        self.t1.count32().intflag.write(|w| w.ovf().set_bit());
    }

    #[inline]
    fn now(&self) -> Instant {
        Instant(self.now_u32())
    }

    #[inline]
    fn now_u32(&self) -> u32 {
        self.t1.count32().count.read().bits()
    }

    pub fn reset(&self) {
        self.t1.count32().count.reset();
    }
}

static mut MONOTONIC_TIMER: Option<FusedTimerCounter<TC4, TC5>> = None;

pub struct Monotonic;

impl rtic::Monotonic for Monotonic {
    type Instant = Instant;

    fn ratio() -> rtic::Fraction {
        rtic::Fraction {
            numerator: 1,
            denominator: 1024, // remember that clock divider, huh?
        }
    }
    fn now() -> Self::Instant {
        let timer = unsafe { MONOTONIC_TIMER.as_ref().unwrap() };
        timer.now()
    }
    unsafe fn reset() {
        let timer = MONOTONIC_TIMER.as_ref().unwrap();
        timer.reset();
    }
    fn zero() -> Self::Instant {
        Instant(0)
    }
}

#[derive(Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub struct Instant(u32);

// impl Instant {
//     pub fn elapsed(&self) -> Instant {
//         let diff = Monotonic::now().0.wrapping_sub(self.0);
//         Instant(diff)
//     }
// }

impl Sub for Instant {
    type Output = Instant;

    fn sub(self, other: Self) -> Self::Output {
        let sub = self.0 - other.0;
        Instant(sub)
    }
}

impl From<u32> for Instant {
    fn from(val: u32) -> Self {
        Instant(val)
    }
}

impl Add<atsamd_hal::time::Miliseconds> for Instant {
    type Output = Instant;

    fn add(self, other: atsamd_hal::time::Miliseconds) -> Self::Output {
        const MILLIS_TO_CLK: u32 = (48_000_000 / 1024) / 1000;
        let counter_cycles = other.0 * MILLIS_TO_CLK;
        Self(self.0 + counter_cycles)
    }
}
