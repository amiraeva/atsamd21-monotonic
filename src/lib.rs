#![no_std]

use atsamd_hal::{
    clock::Tc4Tc5Clock,
    target_device::{TC4, TC5, PM},
};

pub struct FusedTimerCounter<T1, T2> {
    tc4: T1,
    tc5: T2,
}

impl FusedTimerCounter<TC4, TC5> {
    pub fn initialize(tc4: TC4, tc5: TC5, tc45_clk: &Tc4Tc5Clock, pm: &mut PM) -> Self {

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
        count.cc[0].write(|w| unsafe { w.cc().bits(46875 / 2) });

        count.ctrla.modify(|_, w| {
            w.mode().count32();
            w.wavegen().mfrq();
            w.prescaler().div1024();
            w.enable().set_bit()
        });

        while count.status.read().syncbusy().bit_is_set() {}

        while tc5.count32().status.read().syncbusy().bit_is_set() {} // don't know if this is necessary

        // Can test if the 32 bit mode was actually enabled
        // if !_tc5.count32().status.read().slave().bit_is_set() {
        //     panic!("32 bit mode didn't work for fused counter");
        // }

        Self { tc4, tc5 }
    }

    pub fn overflowed(&self) -> bool {
        self.tc4.count32().intflag.read().ovf().bit_is_set()
    }

    pub fn reset_ovf_flag(&mut self) {
        self.tc4.count32().intflag.write(|w| w.ovf().set_bit());
    }

    #[inline]
    pub fn now(&self) -> u32 {
        self.tc4.count32().count.read().bits()
    }
}
