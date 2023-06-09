use core::cell::RefCell;

use embassy_time::{Duration, Ticker};
use embassy_sync::blocking_mutex::{Mutex, raw::ThreadModeRawMutex};

// Onboard LED Status
#[derive(Copy, Clone)]
pub enum LedStatus
{
    Stop,
    Heating,
    Saturating,
    Error,
}

//
// static variables
//
static LED_STATUS : Mutex<ThreadModeRawMutex, RefCell<LedStatus>> = Mutex::new(RefCell::new(LedStatus::Stop));

#[embassy_executor::task]
pub async fn led_task(mut control: cyw43::Control<'static>) -> !
{
    let mut led : bool = false;
    let mut ticks : u32 = 0;
    let (mut blink_on, mut blink_ticks) : (bool, u32) = (false, 0);
    let mut ticker = Ticker::every(Duration::from_millis(10));

    loop {
        if ticks <= 0 && led == false {
            let led_status = LED_STATUS.lock(|lock| {
                *(lock.borrow_mut())
            });

            // define LED blink setting
            // blink_on    : LED blinking if true, LED turn off if false
            // blink_ticks : blink interval if blink_on=true, this setting ignored if blink_on=false. 
            (blink_on, blink_ticks) = match led_status {
                LedStatus::Stop       => (false, 25),
                LedStatus::Heating    => (true,  50),
                LedStatus::Saturating => (true, 100),
                LedStatus::Error      => (true,  10),
            };

            ticks = blink_ticks;
            led = true;
        }

        if blink_on {
            if ticks <= 0 && led == true {
                led = false;
                ticks = blink_ticks;
            }
        }
        else {
            led = false;
        }

        control.gpio_set(0, led).await;
        ticker.next().await;
        ticks -= 1;
    }
}

pub fn set_led(status: LedStatus)
{
    LED_STATUS.lock(|lock| {
        *(lock.borrow_mut()) = status;
    });
}