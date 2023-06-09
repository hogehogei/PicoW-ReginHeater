use core::cell::RefCell;
use core::ops::{DerefMut};

use embassy_rp::gpio;
use gpio::{Output};
use embassy_sync::blocking_mutex::{Mutex, raw::ThreadModeRawMutex};
use embassy_rp::peripherals::{PIN_6};

//
// static variables
//
static HEATER_PORT : Mutex<ThreadModeRawMutex, RefCell<Option<Output<PIN_6>>>> = Mutex::new(RefCell::new(None));

pub fn set_using_gpio_ports(heater_output: Output<'static, PIN_6>)
{
    HEATER_PORT.lock(|lock| {
        *(lock.borrow_mut()) = Some(heater_output);
    })
}

pub fn on_heater_port()
{
    HEATER_PORT.lock(|lock| {
        if let Some(ref mut heater_port) = lock.borrow_mut().deref_mut().as_mut() {
            heater_port.set_high();
        }
    });
}

pub fn off_heater_port()
{
    HEATER_PORT.lock(|lock| {
        if let Some(ref mut heater_port) = lock.borrow_mut().deref_mut().as_mut() {
            heater_port.set_low();
        }
    });
}

