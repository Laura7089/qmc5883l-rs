#![no_std]
#![no_main]

// transport layer for defmt logs
use defmt_rtt as _;
// panicking behavior
use panic_probe as _;
use qmc5883l as _;

#[link_section = ".boot_loader"]
#[used]
pub static BOOT_LOADER: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;

#[defmt_test::tests]
mod tests {
    use defmt::{assert_eq, info};
    use fugit::RateExtU32;
    use hal::{gpio, i2c, pac};
    use qmc5883l::{settings::Settings, QMC8553L};
    use rp2040_hal as hal;
    use shared_bus::BusManagerSimple;

    type I2CPin<P> = gpio::Pin<P, gpio::Function<gpio::I2C>>;
    type I2C = BusManagerSimple<
        i2c::I2C<pac::I2C0, (I2CPin<gpio::bank0::Gpio16>, I2CPin<gpio::bank0::Gpio17>)>,
    >;

    macro_rules! newmag {
        ($i2c:expr) => {
            QMC8553L::new($i2c.acquire_i2c(), Settings::default()).unwrap()
        };
    }

    #[init]
    fn setup() -> I2C {
        let mut perips = pac::Peripherals::take().unwrap();
        let sio = hal::Sio::new(perips.SIO);
        let pins = hal::gpio::Pins::new(
            perips.IO_BANK0,
            perips.PADS_BANK0,
            sio.gpio_bank0,
            &mut perips.RESETS,
        );

        BusManagerSimple::new(hal::I2C::i2c0(
            perips.I2C0,
            pins.gpio16.into_mode(),
            pins.gpio17.into_mode(),
            400.kHz(),
            &mut perips.RESETS,
            125.MHz(),
        ))
    }

    #[test]
    fn make_new(i2c: &mut I2C) {
        newmag!(i2c);
    }

    #[test]
    fn check_ready(i2c: &mut I2C) {
        let mut checks = 0;
        let mut mag = newmag!(i2c);
        while !mag.is_ready().unwrap() {
            checks += 1;
        }
        info!("Ran {} checks before magnetometer reported ready", checks);
    }

    #[test]
    fn read_temp(i2c: &mut I2C) {
        let mut mag = newmag!(i2c);
        let t = mag.get_temp().unwrap();
        info!("Temperature reading: {}", t);
    }

    #[test]
    fn read_all(i2c: &mut I2C) {
        let mut mag = newmag!(i2c);
        let data = mag.read_all().unwrap();
        info!("Field readings: {:?}", data);
    }

    // Read the individual axes in reverse register order
    #[test]
    fn read_reverse(i2c: &mut I2C) {
        use qmc5883l::Axis;

        let mut mag = newmag!(i2c);
        for axis in [Axis::Z, Axis::Y, Axis::X] {
            let val = mag.read(axis).unwrap();
            info!("Read value {} from {:?}", val, axis);
        }
    }

    #[test]
    fn settings_correct(i2c: &mut I2C) {
        use qmc5883l::settings::*;

        let mut mag = newmag!(i2c);

        let set = Settings {
            odr: OutputDataRate::OSR50,
            osr: OverSampleRatio::OSR256,
            rng: FullScale::RNG8G,
        };

        mag.change_settings(set).unwrap();

        assert_eq!(mag.settings().unwrap(), set);
    }
}
