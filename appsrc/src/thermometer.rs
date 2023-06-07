use core::cell::RefCell;

use embassy_time::{Duration, Timer};
use embassy_rp::gpio::{Pin};
use embassy_rp::adc::{Adc};
use embassy_rp::peripherals::{PIN_26, PIN_27};

use embassy_sync::blocking_mutex::{Mutex, raw::ThreadModeRawMutex};


pub struct ADCIo<'a, T1: Pin, T2: Pin>
{
    adc: Adc<'a>,
    heater1 : T1,
    heater2 : T2,
}

impl<'a, T1, T2> ADCIo<'a, T1, T2>
    where T1: Pin, T2: Pin   
{
    pub fn new(adc_in: Adc<'a>, h1: T1, h2: T2 ) -> Self {
        Self { 
            adc: adc_in,
            heater1: h1,
            heater2: h2,
        }
    }
}

struct Thermometer
{
    tempareture : f32,
    exp_mov_ave_alpha : f32,
}

//
// static const variables
//

// ADC -> celsius tempareture conversion table
// 1000 = 10.00[Celsius]
static TEMPARETURE_TABLE : [i16; 411] = [
    -7300,
    -7300,
    -6440,
    -5901,
    -5500,
    -5177,
    -4906,
    -4671,
    -4463,
    -4275,
    -4105,
    -3948,
    -3802,
    -3666,
    -3539,
    -3419,
    -3305,
    -3196,
    -3093,
    -2995,
    -2900,
    -2809,
    -2722,
    -2637,
    -2555,
    -2476,
    -2400,
    -2326,
    -2253,
    -2183,
    -2115,
    -2048,
    -1983,
    -1919,
    -1857,
    -1796,
    -1736,
    -1678,
    -1621,
    -1565,
    -1510,
    -1456,
    -1402,
    -1350,
    -1299,
    -1248,
    -1198,
    -1149,
    -1101,
    -1053,
    -1006,
    -960,
    -914,
    -869,
    -825,
    -781,
    -737,
    -694,
    -652,
    -610,
    -568,
    -527,
    -486,
    -446,
    -406,
    -367,
    -328,
    -289,
    -250,
    -212,
    -175,
    -137,
    -100,
    -63,
    -27,
    9,
    45,
    81,
    116,
    151,
    186,
    221,
    255,
    289,
    323,
    357,
    391,
    424,
    457,
    490,
    523,
    555,
    588,
    620,
    652,
    684,
    715,
    747,
    778,
    810,
    841,
    872,
    903,
    933,
    964,
    994,
    1025,
    1055,
    1085,
    1115,
    1145,
    1174,
    1204,
    1233,
    1263,
    1292,
    1321,
    1351,
    1380,
    1409,
    1437,
    1466,
    1495,
    1523,
    1552,
    1581,
    1609,
    1637,
    1666,
    1694,
    1722,
    1750,
    1778,
    1806,
    1834,
    1862,
    1890,
    1917,
    1945,
    1973,
    2000,
    2028,
    2055,
    2083,
    2110,
    2138,
    2165,
    2192,
    2220,
    2247,
    2274,
    2302,
    2329,
    2356,
    2383,
    2410,
    2437,
    2464,
    2492,
    2519,
    2546,
    2573,
    2600,
    2627,
    2654,
    2681,
    2708,
    2735,
    2762,
    2789,
    2816,
    2843,
    2870,
    2897,
    2924,
    2951,
    2978,
    3005,
    3032,
    3059,
    3086,
    3113,
    3141,
    3168,
    3195,
    3222,
    3249,
    3277,
    3304,
    3331,
    3358,
    3386,
    3413,
    3441,
    3468,
    3495,
    3523,
    3550,
    3578,
    3606,
    3633,
    3661,
    3689,
    3717,
    3744,
    3772,
    3800,
    3828,
    3856,
    3884,
    3913,
    3941,
    3969,
    3997,
    4026,
    4054,
    4083,
    4111,
    4140,
    4169,
    4198,
    4227,
    4256,
    4285,
    4314,
    4343,
    4372,
    4402,
    4431,
    4461,
    4490,
    4520,
    4550,
    4580,
    4610,
    4640,
    4670,
    4700,
    4731,
    4761,
    4792,
    4823,
    4853,
    4884,
    4916,
    4947,
    4978,
    5010,
    5041,
    5073,
    5105,
    5137,
    5169,
    5201,
    5234,
    5266,
    5299,
    5332,
    5365,
    5398,
    5431,
    5465,
    5498,
    5532,
    5566,
    5600,
    5635,
    5669,
    5704,
    5739,
    5774,
    5809,
    5845,
    5880,
    5916,
    5952,
    5989,
    6025,
    6062,
    6099,
    6136,
    6174,
    6212,
    6250,
    6288,
    6326,
    6365,
    6404,
    6443,
    6483,
    6523,
    6563,
    6603,
    6644,
    6685,
    6727,
    6768,
    6810,
    6853,
    6895,
    6939,
    6982,
    7026,
    7070,
    7114,
    7159,
    7205,
    7251,
    7297,
    7343,
    7390,
    7438,
    7486,
    7534,
    7583,
    7633,
    7683,
    7733,
    7784,
    7835,
    7888,
    7940,
    7993,
    8047,
    8102,
    8157,
    8213,
    8269,
    8326,
    8384,
    8443,
    8502,
    8562,
    8623,
    8685,
    8748,
    8811,
    8876,
    8941,
    9007,
    9075,
    9143,
    9212,
    9283,
    9354,
    9427,
    9501,
    9577,
    9653,
    9731,
    9810,
    9891,
    9974,
    10058,
    10143,
    10230,
    10319,
    10410,
    10503,
    10598,
    10695,
    10794,
    10896,
    10999,
    11106,
    11215,
    11327,
    11442,
    11560,
    11681,
    11806,
    11934,
    12067,
    12203,
    12344,
    12490,
    12640,
    12796,
    12958,
    13125,
    13300,
    13481,
    13670,
    13867,
    14074,
    14290,
    14517,
    14756,
    15008,
    15274,
    15557,
    15858,
    16179,
    16523,
    16894,
    17296,
    17732,
    18211,
    18740,
    19329,
    19992,
    20749,
    21626,
    22665,
    23929,
    25525,
    27652,
    30748,
    32767,
    32767,
    32767,
];

//
// static variables
//
static HEATER1_TEMP : Mutex<ThreadModeRawMutex, RefCell<f32>> = Mutex::new(RefCell::new(0.0));
static HEATER2_TEMP : Mutex<ThreadModeRawMutex, RefCell<f32>> = Mutex::new(RefCell::new(0.0));
static CPU_TEMP : Mutex<ThreadModeRawMutex, RefCell<f32>> = Mutex::new(RefCell::new(0.0));

impl Thermometer
{
    pub fn new(alpha: f32) -> Self {
        Self { 
            tempareture: 0.0,
            exp_mov_ave_alpha: alpha
        }
    }

    pub fn calc_next(&mut self, adc_value: u16) -> f32
    {
        let current_temp : f32 = get_tempareture_from_table(adc_value);
        self.tempareture = (current_temp * self.exp_mov_ave_alpha) + ((1.0-self.exp_mov_ave_alpha) * self.tempareture);

        self.tempareture
    }
}

#[embassy_executor::task]
pub async fn thermometer_task(mut adcio: ADCIo<'static, PIN_26, PIN_27>)
{

    let mut heater1_temp = Thermometer::new(0.22);
    let mut heater2_temp = Thermometer::new(0.22);

    loop {
        let heater1_level = adcio.adc.read(&mut adcio.heater1).await;
        let heater1_current_temp = heater1_temp.calc_next(heater1_level);
        //log::info!("Pin 31 ADC: {}", heater1_level);
        
        let heater2_level = adcio.adc.read(&mut adcio.heater2).await;
        let heater2_current_temp = heater2_temp.calc_next(heater2_level);
        //info!("Pin 32 ADC: {}", level);
        
        let cputemp = adcio.adc.read_temperature().await;
        //info!("Temp: {} degrees", convert_to_celsius(cputemp));

        HEATER1_TEMP.lock(|lock| {
            *lock.borrow_mut() = heater1_current_temp
        });
        HEATER2_TEMP.lock(|lock| {
            *lock.borrow_mut() = heater2_current_temp
        });
        CPU_TEMP.lock(|lock| {
            *lock.borrow_mut() = convert_to_celsius(cputemp);
        });

        Timer::after(Duration::from_millis(20)).await;
    }
}

fn get_tempareture_from_table(adc_value: u16) -> f32
{
    // Clipping 12bit ADC range
    let v : usize = if adc_value < 4096 { adc_value } else { 4095 } as usize;
    
    // Tempareture table has record that every tens digit.
    // The temperature corresponding to one digit of ADC is linearly interpolated.
    let adc_a: usize = (v / 10) as usize;

    let temp_a : f32 = TEMPARETURE_TABLE[adc_a] as f32 / 100.0;
    let temp_b : f32 = TEMPARETURE_TABLE[adc_a + 1] as f32 / 100.0;
    let alpha : f32 = (v - adc_a) as f32 / 10.0;
    
    // Return tempareture
    (temp_a * (1.0 - alpha)) + (temp_b * alpha)
}

fn convert_to_celsius(raw_temp: u16) -> f32
{
    // According to chapter 4.9.5. Temperature Sensor in RP2040 datasheet
    27.0 - (raw_temp as f32 * 3.3 / 4096.0 - 0.706) / 0.001721 as f32
}

pub fn heater1_tempareture() -> f32
{
    let tempareture = HEATER1_TEMP.lock(|lock| {
        *(lock.borrow_mut())
    });
    ((tempareture * 100.0 + 0.5) as u32) as f32 / 100.0
}

pub fn heater2_tempareture() -> f32
{
    let tempareture = HEATER2_TEMP.lock(|lock| {
        *(lock.borrow_mut())
    });
    ((tempareture * 100.0 + 0.5) as u32) as f32 / 100.0
}

pub fn cpu_tempareture() -> f32
{
    let tempareture = CPU_TEMP.lock(|lock| {
        *(lock.borrow_mut())
    });
    ((tempareture * 100.0 + 0.5) as u32) as f32 / 100.0
}

