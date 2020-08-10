#![no_std]
#![no_main]

use panic_halt as _;

use atsamd_hal::{
    clock::GenericClockController,
    gpio::{self, GpioExt as _},
    target_device::{Peripherals, CorePeripherals},
    time::U32Ext as _
};

use feather_m0::pac::interrupt;

use cortex_m_rt::entry;
use rtic::Monotonic;

use atsamd21_monotonic::FusedTimerCounter;

use atsamd_hal::usb::UsbBus;
use usb_device::bus::UsbBusAllocator;

use usb_device::prelude::*;
use usbd_serial::{SerialPort, USB_CLASS_CDC};

use cortex_m::asm::delay as cycle_delay;
use cortex_m::peripheral::NVIC;

#[entry]
fn main() -> ! {
    let mut core = CorePeripherals::take().unwrap();
    let mut peripherals = Peripherals::take().unwrap();
    let mut clocks = GenericClockController::with_internal_32kosc(
        peripherals.GCLK,
        &mut peripherals.PM,
        &mut peripherals.SYSCTRL,
        &mut peripherals.NVMCTRL,
    );

    let mut pins = peripherals.PORT.split();
    let mut red_led = pins.pa17.into_open_drain_output(&mut pins.port);


    let bus_allocator = unsafe {
        USB_ALLOCATOR = Some(feather_m0::usb_allocator(
            peripherals.USB,
            &mut clocks,
            &mut peripherals.PM,
            pins.pa24,
            pins.pa25,
            &mut pins.port,
        ));
        USB_ALLOCATOR.as_ref().unwrap()
    };

    unsafe {
        USB_SERIAL = Some(SerialPort::new(&bus_allocator));
        USB_BUS = Some(
            UsbDeviceBuilder::new(&bus_allocator, UsbVidPid(0x16c0, 0x27dd))
                .manufacturer("Fake company")
                .product("Serial port")
                .serial_number("TEST")
                .device_class(USB_CLASS_CDC)
                .build(),
        );
    }

    unsafe {
        core.NVIC.set_priority(interrupt::USB, 2);
        NVIC::unmask(interrupt::USB);
    }

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

        cortex_m::asm::wfi();

        // if tc4tc5_fused.overflowed() {
        //     tc4tc5_fused.reset_ovf_flag();
        //     red_led.toggle();
        // }
    }
}

static mut USB_ALLOCATOR: Option<UsbBusAllocator<UsbBus>> = None;
static mut USB_BUS: Option<UsbDevice<UsbBus>> = None;
static mut USB_SERIAL: Option<SerialPort<UsbBus>> = None;

use numtoa::NumToA as _;

fn poll_usb() {
    unsafe {
        USB_BUS.as_mut().map(|usb_dev| {
            USB_SERIAL.as_mut().map(|serial| {
                usb_dev.poll(&mut [serial]);
                let mut buf = [0u8; 64];

                if let Ok(count) = serial.read(&mut buf) {
                    // for (i, c) in buf.iter().enumerate() {
                    //     if i >= count {
                    //         break;
                    //     }

                    //     serial.write(&[c.clone()]);
                    // }

                    let count = atsamd21_monotonic::Monotonic::now().0;
                    let _ = serial.write(b"time: ");
                    let count = count.numtoa(10u32, &mut buf);
                    let _ = serial.write(count);
                    let _ = serial.write(b"\r\n");
                };
            });
        });
    };
}
// use feather_m0 as _;

#[interrupt]
fn USB() {
    poll_usb();
}
