#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(async_fn_in_trait)]
#![allow(incomplete_features)]
#[macro_use]
extern crate alloc;

use core::mem::MaybeUninit;

use cyw43_pio::PioSpi;
use embassy_executor::Spawner;
use embassy_net::tcp::TcpSocket;
use embassy_net::{Config, Stack, StackResources};
use embassy_rp::gpio::{Level, Output};
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::{DMA_CH0, PIN_23, PIN_25, PIO0, USB};
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_rp::adc::Adc;
use embassy_rp::pio::Pio;
use embedded_alloc::Heap;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

mod rest;
mod thermometer;
mod controller;
mod led;
mod gpio;
mod util;
use crate::rest::Rest;
use crate::thermometer::*;
use crate::controller::*;
use crate::led::*;
use crate::gpio::*;

macro_rules! singleton {
    ($val:expr) => {{
        type T = impl Sized;
        static STATIC_CELL: StaticCell<T> = StaticCell::new();
        STATIC_CELL.init_with(move || $val)
    }};
}

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
    ADC_IRQ_FIFO => embassy_rp::adc::InterruptHandler;
});

//
// static variables
//
const HEAP_SIZE : usize = 1024 * 32;     // 32KiB 
static mut HEAP_MEM : [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
#[global_allocator]
static HEAP : Heap = Heap::empty();

//
// Tasks
//
#[embassy_executor::task]
async fn wifi_task(
    runner: cyw43::Runner<'static, Output<'static, PIN_23>, PioSpi<'static, PIN_25, PIO0, 0, DMA_CH0>>,
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<cyw43::NetDriver<'static>>) -> ! {
    stack.run().await
}

#[embassy_executor::task]
async fn logger_task(driver: Driver<'static, USB>) {
    //embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);

    // above macro will cause compile error when using crate "log = 0.4.20", expand macro manually
    static LOGGER: ::embassy_usb_logger::UsbLogger<1024> = ::embassy_usb_logger::UsbLogger::new();
    unsafe {
        let _ = ::log::set_logger_racy(&LOGGER).map(|()| log::set_max_level_racy(log::LevelFilter::Info));
    }
    let _ = LOGGER.run(&mut ::embassy_usb_logger::LoggerState::new(), driver).await;
}

#[embassy_executor::main]
async fn main(spawner: Spawner)
{
    // Initialize the allocator BEFORE use it
    {
        unsafe { HEAP.init(HEAP_MEM.as_ptr() as usize, HEAP_SIZE) }
    }

    let p = embassy_rp::init(Default::default());

    // Start USB logger task
    //let usb_driver = Driver::new(p.USB, Irqs);
    //spawner.spawn(logger_task(usb_driver)).unwrap();

    // Set heater gpio
    let heater_port = Output::new(p.PIN_6, Level::Low);
    set_using_gpio_ports(heater_port);
    // Start thermomater(Heater, CPU)
    let adc = Adc::new(p.ADC, Irqs, embassy_rp::adc::Config::default());
    let adcio = ADCIo::new(adc, p.PIN_26, p.PIN_27);
    spawner.spawn(thermometer_task(adcio)).unwrap();

    log::info!("Hello World!");

    let fw = include_bytes!("../../cyw43/firmware/43439A0.bin");
    let clm = include_bytes!("../../cyw43/firmware/43439A0_clm.bin");

    // To make flashing faster for development, you may want to flash the firmwares independently
    // at hardcoded addresses, instead of baking them into the program with `include_bytes!`:
    //     probe-rs-cli download 43439A0.bin --format bin --chip RP2040 --base-address 0x10100000
    //     probe-rs-cli download 43439A0_clm.bin --format bin --chip RP2040 --base-address 0x10140000
    //let fw = unsafe { core::slice::from_raw_parts(0x10100000 as *const u8, 224190) };
    //let clm = unsafe { core::slice::from_raw_parts(0x10140000 as *const u8, 4752) };

    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let mut pio = Pio::new(p.PIO0);
    let spi = PioSpi::new(&mut pio.common, pio.sm0, pio.irq0, cs, p.PIN_24, p.PIN_29, p.DMA_CH0);

    let state = singleton!(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;
    spawner.spawn(wifi_task(runner)).unwrap();

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;
    control.gpio_set(0, true).await;

    let config = Config::Dhcp(Default::default());
    //let config = embassy_net::Config::Static(embassy_net::Config {
    //    address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 69, 2), 24),
    //    dns_servers: Vec::new(),
    //    gateway: Some(Ipv4Address::new(192, 168, 69, 1)),
    //});

    // Generate random seed
    let seed = 0x0123_4567_89ab_cdef; // chosen by fair dice roll. guarenteed to be random.

    // Init network stack
    let stack = &*singleton!(Stack::new(
        net_device,
        config,
        singleton!(StackResources::<2>::new()),
        seed
    ));

    spawner.spawn(net_task(stack)).unwrap();

    loop {
        //control.join_open(env!("WIFI_NETWORK")).await;
        match control.join_wpa2(env!("WIFI_NETWORK"), env!("WIFI_PASSWORD")).await {
            Ok(_) => break,
            Err(err) => {
                log::info!("join failed with status={}", err.status);
            }
        }
    }
    
    // Start Controller task
    spawner.spawn(controller_task()).unwrap();
    // Start LED task
    spawner.spawn(led_task(control)).unwrap();

    // And now we can use it!
    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];

    loop {
        let socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
        let mut server = Rest::new(socket);

        if let Err(s) = server.accept().await {
            log::warn!("{}", s.as_str());
            continue;
        }
        if let Err(s) = server.do_rest_service().await {
            log::warn!("{}", s.as_str());
        }
        server.close().await;
    }
}
