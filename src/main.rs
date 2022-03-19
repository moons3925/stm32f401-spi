#![no_std]
#![no_main]

const WHO_AM_I: u8 = 0x0f;  // デバイス確認用のコマンド
const CTRL_REG1: u8 = 0x20; // コントロールレジスタ1
const WAKE_UP: u8 = 0x90;   // デバイスを起こすためのコマンド
const P_ADRS: u8 = 0x28;    // 気圧読み込み用のアドレス
const LPS25HB_DEVICE_CODE: u8 = 0xbd;

use panic_halt as _; // you can put a breakpoint on `rust_begin_unwind` to catch panics
use cortex_m_rt::entry;
use cortex_m::delay;    // Delayを使う
use stm32f4::stm32f401;
//use cortex_m_semihosting::hprintln;

#[entry]
fn main() -> ! {
    let mut dp = stm32f401::Peripherals::take().unwrap();   // デバイス用Peripheralsの取得
    let cp = cortex_m::peripheral::Peripherals::take().unwrap();    // cortex-m Peripheralsの取得
    let mut delay = delay::Delay::new(cp.SYST, 84000000_u32);   // Delayの生成
    clock_init(&dp);    // クロック関連の初期化
    gpioa5_init(&dp);   // GPIOAの初期化
    spi1_init(&dp);     // SPI1の初期化
    lps25hb_init(&mut dp);  // LPS25HBの初期化
    loop {
        delay.delay_ms(5_u32);           // delay 2000msec
        lps25hb_select(&dp);
        lps25hb_send(&mut dp, (P_ADRS | 0xc0) as u16);
        let l = lps25hb_send(&mut dp, 0);
        let m = lps25hb_send(&mut dp, 0);
        let h = lps25hb_send(&mut dp, 0);
        lps25hb_deselect(&dp);
        let mut press = h << 16 | m << 8 | l;
        press >>= 12;   // 1/4096
    }
}

fn clock_init(dp: &stm32f401::Peripherals) {

    // PLLSRC = HSI: 16MHz (default)
    dp.RCC.pllcfgr.modify(|_, w| w.pllp().div4());      // (13)P=4
    dp.RCC.pllcfgr.modify(|_, w| unsafe { w.plln().bits(336) });    // (14)N=336
    // PLLM = 16 (default)

    dp.RCC.cfgr.modify(|_, w| w.ppre1().div2());        // (15) APB1 PSC = 1/2
    dp.RCC.cr.modify(|_, w| w.pllon().on());            // (16)PLL On
    while dp.RCC.cr.read().pllrdy().is_not_ready() {    // (17)安定するまで待つ
        // PLLがロックするまで待つ (PLLRDY)
    }

    // データシートのテーブル15より
    dp.FLASH.acr.modify(|_,w| w.latency().bits(2));    // (18)レイテンシの設定: 2ウェイト

    dp.RCC.cfgr.modify(|_,w| w.sw().pll());     // (19)sysclk = PLL
    while !dp.RCC.cfgr.read().sws().is_pll() {  // (20)SWS システムクロックソースがPLLになるまで待つ
    }
//  SYSCLK = 16MHz * 1/M * N * 1/P
//  SYSCLK = 16MHz * 1/16 * 336 * 1/4 = 84MHz
//  APB2 = 84MHz (SPI1 pclk)
}

fn gpioa5_init(dp: &stm32f401::Peripherals) {

    dp.RCC.ahb1enr.modify(|_, w| w.gpioaen().enabled());    // (21)GPIOAのクロックを有効にする
    dp.GPIOA.moder.modify(|_, w| w.moder4().output());      // (22)GPIOA4を汎用出力に設定    

    dp.GPIOA.moder.modify(|_, w| w.moder5().alternate());   // (22)GPIOA5をオルタネートに設定    
    dp.GPIOA.afrl.modify(|_, w| w.afrl5().af5());           // (23)GPIOA5をAF5に設定    
    dp.GPIOA.moder.modify(|_, w| w.moder6().alternate());   // (22)GPIOA6をオルタネートに設定    
    dp.GPIOA.afrl.modify(|_, w| w.afrl6().af5());           // (23)GPIOA6をAF5に設定    
    dp.GPIOA.moder.modify(|_, w| w.moder7().alternate());   // (22)GPIOA7をオルタネートに設定    
    dp.GPIOA.afrl.modify(|_, w| w.afrl7().af5());           // (23)GPIOA7をAF5に設定    

    lps25hb_deselect(dp);   // CS=High
}

fn spi1_init(dp: &stm32f401::Peripherals) {

    // SPI1のクロックイネーブル機能は APB2 にある
    dp.RCC.apb2enr.modify(|_,w| w.spi1en().enabled());          // (24)SPI1のクロックを有効にする
    dp.SPI1.cr1.modify(|_, w| w.ssm().set_bit());
    dp.SPI1.cr1.modify(|_, w| w.ssi().set_bit());
    dp.SPI1.cr1.modify(|_, w| w.br().div16());          // 84MHz/16=5.25MHz
    dp.SPI1.cr1.modify(|_, w| w.cpha().second_edge());
    dp.SPI1.cr1.modify(|_, w| w.cpol().idle_high());
    dp.SPI1.cr1.modify(|_, w| w.mstr().master());
    dp.SPI1.cr1.modify(|_, w| w.spe().enabled());
}

fn lps25hb_init(dp: &mut stm32f401::Peripherals) -> bool {

    lps25hb_select(dp);
    lps25hb_send(dp, (WHO_AM_I | 0x80).into());  // WHO_AM_I コマンドを送る
    let res = lps25hb_send(dp, 0u16);  // 読む
    lps25hb_deselect(dp);

    lps25hb_select(dp);
    lps25hb_send(dp, (CTRL_REG1).into());   // CTRLREG1
    lps25hb_send(dp, (WAKE_UP).into());     // 起床を指示
    lps25hb_deselect(dp);
    if res == LPS25HB_DEVICE_CODE.into() {
        return true;    // デバイスコードが返ってくれば true
    }
    false
}

fn lps25hb_select(dp: &stm32f401::Peripherals) {    // CS=Low
    dp.GPIOA.odr.modify(|_, w| w.odr4().low());
}

fn lps25hb_deselect(dp: &stm32f401::Peripherals) {  // CS=High
    dp.GPIOA.odr.modify(|_, w| w.odr4().high());
}

fn lps25hb_send(dp: &mut stm32f401::Peripherals, data: u16) -> u32 {
    while dp.SPI1.sr.read().txe().is_not_empty() {}
    dp.SPI1.dr.write(|w| w.dr().bits(data));    // 書いて
    while dp.SPI1.sr.read().rxne().is_empty() {}
    dp.SPI1.dr.read().bits()    // 読む
}
