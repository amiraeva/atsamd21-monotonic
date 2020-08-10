#![no_std]
#![no_main]

extern crate cortex_m;
extern crate cortex_m_semihosting;
extern crate feather_m0 as hal;
extern crate nb;
#[cfg(not(feature = "use_semihosting"))]
extern crate panic_halt;
#[cfg(feature = "use_semihosting")]
extern crate panic_semihosting;

use hal::clock::GenericClockController;
use hal::entry;
use hal::pac::{Peripherals, PM, TC4, TC5};
use hal::prelude::*;
use hal::timer::TimerCounter;

use nb::block;

#[entry]
fn main() -> ! {
    let mut peripherals = Peripherals::take().unwrap();
    let mut clocks = GenericClockController::with_internal_32kosc(
        peripherals.GCLK,
        &mut peripherals.PM,
        &mut peripherals.SYSCTRL,
        &mut peripherals.NVMCTRL,
    );
    let mut pins = hal::Pins::new(peripherals.PORT);
    let mut red_led = pins.d13.into_open_drain_output(&mut pins.port);

    // gclk0 represents a configured clock using the system 48MHz oscillator
    let gclk0 = clocks.gclk0();
    // configure a clock for the TC4 and TC5 peripherals
    let tc45 = &clocks.tc4_tc5(&gclk0).unwrap();
    // instantiate a timer objec for the TC4 peripheral

    let mut tc4c = TC4Clock::initialize(peripherals.TC4, peripherals.TC5, &mut peripherals.PM);

    // toggle the red LED at the frequency set by the timer
    let mut counter: u64 = 1;

    loop {
        if tc4c.overflowed() {
            counter += 1;
            tc4c.reset_ovf_flag();
            red_led.toggle();
        }

        // if counter % 1000 == 0 {
        //     red_led.toggle();
        // }

        // if tc4c.now() % 46875 == 0 {
        //     red_led.toggle();
        // }
    }
}

pub struct TC4Clock {
    tc4: TC4,
    _tc5: TC5,
}

impl TC4Clock {
    pub fn initialize(tc4: TC4, _tc5: TC5, pm: &mut PM) -> Self {

        let params = TimerParams::new(1000, 48_000_000);
        let divider = params.divider;
        let cycles = params.cycles;

        pm.apbcmask.modify(|_, w| w.tc4_().set_bit());
        pm.apbcmask.modify(|_, w| w.tc5_().set_bit());

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

        count.readreq.modify(|_, w| w.rcont().set_bit());

        // Set TOP value for mfrq mode
        count.cc[0].write(|w| unsafe { w.cc().bits(46875 / 2) });
        // count.cc[0].write(|w| unsafe { w.cc().bits(cycles) });

        count.ctrla.modify(|_, w| {
            w.mode().count32();
            w.wavegen().mfrq();
            w.prescaler().div1024();

            // match divider {
            //     1 => w.prescaler().div1(),
            //     2 => w.prescaler().div2(),
            //     4 => w.prescaler().div4(),
            //     8 => w.prescaler().div8(),
            //     16 => w.prescaler().div16(),
            //     64 => w.prescaler().div64(),
            //     256 => w.prescaler().div256(),
            //     1024 => w.prescaler().div1024(),
            //     _ => unreachable!(),
            // };
            w.enable().set_bit()
        });

        while count.status.read().syncbusy().bit_is_set() {}

        while _tc5.count32().status.read().syncbusy().bit_is_set() {}

        if !_tc5.count32().status.read().slave().bit_is_set() {
            panic!("");
        }

        Self { tc4, _tc5 }
    }

    fn overflowed(&self) -> bool {
        self.tc4.count32().intflag.read().ovf().bit_is_set()
    }

    fn reset_ovf_flag(&mut self) {
        self.tc4.count32().intflag.write(|w| w.ovf().set_bit());
    }

    #[inline]
    fn now(&self) -> u32 {
        self.tc4.count32().count.read().bits()
    }
}

/// Helper type for computing cycles and divider given frequency
#[derive(Debug, Clone, Copy)]
pub struct TimerParams {
    pub divider: u16,
    pub cycles: u32,
}

impl TimerParams {
    pub fn new(timeout: u64, src_freq: u64) -> Self {
        let ticks = src_freq / timeout.max(1);
        TimerParams::new_from_ticks(ticks)
    }

    fn new_from_ticks(ticks: u64) -> Self {
        let divider = ((ticks >> 32) + 1).next_power_of_two();
        let divider = match divider {
            1 | 2 | 4 | 8 | 16 | 64 | 256 | 1024 => divider,
            // There are a couple of gaps, so we round up to the next largest
            // divider; we'll need to count twice as many but it will work.
            32 => 64,
            128 => 256,
            512 => 1024,
            // Catch all case; this is lame.  Would be great to detect this
            // and fail at compile time.
            _ => 1024,
        };

        let cycles = ticks / divider as u64;

        if cycles > u32::max_value() as u64 {
            panic!("cycles {} is out of range for a 32 bit counter", cycles);
        }

        TimerParams {
            divider: divider as u16,
            cycles: cycles as u32,
        }
    }
}
