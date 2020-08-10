#![no_std]
#![no_main]

use panic_halt as _;

use atsamd_hal::{
    clock::GenericClockController,
    gpio::{self, GpioExt as _},
    target_device::{Peripherals, PM, TC4, TC5},
    time::U32Ext as _
};
use cortex_m_rt::entry;
use rtic::Monotonic;

use atsamd21_monotonic::FusedTimerCounter;

#[entry]
fn main() -> ! {
    let mut peripherals = Peripherals::take().unwrap();
    let mut clocks = GenericClockController::with_internal_32kosc(
        peripherals.GCLK,
        &mut peripherals.PM,
        &mut peripherals.SYSCTRL,
        &mut peripherals.NVMCTRL,
    );

    let mut pins = peripherals.PORT.split();
    let mut red_led = pins.pa17.into_open_drain_output(&mut pins.port);

    // initialzes fused 32 bit timer
    let tc4tc5_fused = FusedTimerCounter::initialize(
        peripherals.TC4,
        peripherals.TC5,
        &mut clocks,
        &mut peripherals.PM,
    );

    loop {
        // let start = atsamd21_monotonic::Monotonic::now();
        // let next = start + 1000.ms();

        // while atsamd21_monotonic::Monotonic::now() < next {}
        // red_led.toggle();

        if tc4tc5_fused.overflowed() {
            tc4tc5_fused.reset_ovf_flag();
            red_led.toggle();
        }
    }
}
