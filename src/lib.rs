#![no_std]

use core::{
    marker::PhantomData,
    num::NonZeroU32,
    ops::{self, Add},
};

use atsamd_hal::{
    clock::GenericClockController,
    target_device::{PM, TC4, TC5},
};
use ops::Sub;
pub struct FusedTimerCounter<T1, T2> {
    t1: T1,
    t2: T2,
}

impl FusedTimerCounter<TC4, TC5> {
    pub fn initialize(tc4: TC4, tc5: TC5, gclk: &mut GenericClockController, pm: &mut PM) {
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
        count.cc[0].write(|w| unsafe { w.cc().bits(u32::max_value()) }); // continue counting and wrapping around u32::MAX

        count.ctrla.modify(|_, w| {
            w.mode().count32();
            w.wavegen().mfrq();
            w.prescaler().div1024(); // divides CPU clock speed
            w.enable().set_bit()
        });

        while count.status.read().syncbusy().bit_is_set() {}

        while tc5.count32().status.read().syncbusy().bit_is_set() {} // don't know if this is necessary

        // Can test if the 32 bit mode was actually enabled
        // if !_tc5.count32().status.read().slave().bit_is_set() {
        //     panic!("32 bit mode didn't work for fused counter");
        // }

        let counter = Self {
            t1: tc4,
            t2: tc5,
        };

        unsafe { MONOTONIC_TIMER = Some(counter) };
    }

    pub fn overflowed(&self) -> bool {
        self.t1.count32().intflag.read().ovf().bit_is_set()
    }

    pub fn reset_ovf_flag(&mut self) {
        self.t1.count32().intflag.write(|w| w.ovf().set_bit());
    }

    #[inline]
    pub fn now(&self) -> u32 {
        self.t1.count32().count.read().bits()
    }
}

static mut MONOTONIC_TIMER: Option<FusedTimerCounter<TC4, TC5>> = None;

pub struct Monotonic;

impl rtic::Monotonic for Monotonic {
    type Instant = Instant;

    fn ratio() -> rtic::Fraction {
        todo!()
    }
    fn now() -> Self::Instant {
        todo!()
    }
    unsafe fn reset() {
        todo!()
    }
    fn zero() -> Self::Instant {
        todo!()
    }
}

#[derive(Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub struct Instant(u32);

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
        todo!()
    }
}
