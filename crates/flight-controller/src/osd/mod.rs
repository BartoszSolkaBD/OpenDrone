//! Betaflight 2026.6's OSD logic, reimplemented from the behaviour of
//! Betaflight 2026.6.2's `osd/osd.c`, `osd/osd_elements.c` and
//! `osd/osd_warnings.c` (the goggles-OSD research §1, #31). It runs beside
//! the Simulation, never inside it ([ADR-0022]): fed each Flight Controller
//! loop's [`DebugRecord`] and the pilot's [`OsdLayout`], it hands out the
//! [`OsdScreen`], which character sits in which cell and what each element
//! reads. Nothing it does feeds back into a flight, and it reads no clock:
//! its time is the Flight Controller's own, counted in Simulation Time, so a
//! replayed flight shows the same OSD.
//!
//! - **Twelve times a second** (`osd_framerate_hz`): at power-up and every
//!   83,333 µs of the Flight Controller's clock after, it redraws the screen.
//!   Between redraws the screen holds.
//! - **Power-up**: when the Flight Controller's clock starts again from
//!   zero (Reset, a new Map or a new Quad), the OSD starts afresh too: its
//!   timers from zero, no statistics, no statistics page. A start already
//!   armed (a mid-air start) shows no `ARMED`.
//! - **Elements** are drawn in Betaflight's order, each where the layout puts
//!   it if it's visible. Those with no source in the sim (RSSI, RSSI dBm,
//!   RSNR, and the current and mAh without a `current_meter` OpenDrone
//!   simulates) read blank, as do the three not drawn yet (the artificial
//!   horizon, its sidebars and the flip arrow).
//! - **Blinking** is Betaflight's 2 Hz: a blinking element is hidden on a
//!   redraw that falls in the first 250 ms of each half second.
//! - **Warnings**, first match wins, in Betaflight's order: the reason
//!   arming is refused while the Arm switch is on (cycling every 0.5 s, the
//!   Arm switch's own reason only when it's alone), `FAIL SAFE`, `>CRASH
//!   FLIP<` or `CRASHFLIP SW`, `LINK QUALITY`, ` LAND NOW`, `LOW BATTERY`,
//!   `OVER CAP`, `BATT < FULL`, then the visual beeper `  * * * *` once for
//!   each beep that started. The RSSI warnings have no source and never
//!   show.
//! - **`ARMED`** shows alone for 0.5 s at arming, with `>CRASH FLIP<` under
//!   it in Crash Flip.
//! - **The statistics page** shows on disarming, for up to 60 s, unless
//!   Failsafe disarmed the Quad, or runaway takeoff prevention did with the
//!   warnings visible. Throttle or pitch above 1750 µs or the Crash Flip
//!   switch dismisses it. It lists the statistics that are on and have a
//!   source in the sim, redrawn each second.
//!
//! [ADR-0022]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0022-osd-worked-out-beside-the-simulation.md

mod layout;

pub use layout::{
    Alarms, COLUMNS, DEFAULT_STATISTICS, DEFAULT_TIMERS, DEFAULT_WARNINGS, Drawn, OsdElement,
    OsdLayout, Placement, ROWS, Statistic, Timer, TimerPrecision, TimerSource, Warning,
};

/// What a Scenario's OSD Expectation reads: an element, whether it blinks,
/// the message in the middle of the screen, the statistics page or one
/// statistic on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OsdReading {
    Element(OsdElement),
    Blinks(OsdElement),
    Message,
    StatisticsPage,
    Statistic(String),
}

impl OsdReading {
    /// Read from its words: an element's name (`warnings`, `Flight Mode`,
    /// or Betaflight's `flymode`), `<element> blinks`, `message`,
    /// `statistics page` or `statistic <label>`, such as `statistic TOTAL
    /// ARM`.
    pub fn named(words: &str) -> Option<OsdReading> {
        let words = words.trim();
        if let Some(element) = words.strip_suffix(" blinks") {
            return OsdElement::named(element).map(OsdReading::Blinks);
        }
        if words.eq_ignore_ascii_case("message") {
            return Some(OsdReading::Message);
        }
        if words.eq_ignore_ascii_case("statistics page") {
            return Some(OsdReading::StatisticsPage);
        }
        if let Some(label) = words.strip_prefix("statistic ") {
            return Some(OsdReading::Statistic(label.trim().to_string()));
        }
        OsdElement::named(words).map(OsdReading::Element)
    }

    /// What the screen shows for it: the text, or `yes` or `no` for a blink.
    pub fn read(&self, screen: &OsdScreen) -> String {
        match self {
            OsdReading::Element(element) => screen.reads(*element),
            OsdReading::Blinks(element) => {
                if screen.blinks(*element) { "yes" } else { "no" }.to_string()
            }
            OsdReading::Message => screen.message(),
            OsdReading::StatisticsPage => screen.statistics_page(),
            OsdReading::Statistic(label) => screen.statistic(label).unwrap_or_default(),
        }
    }
}

use crate::{ArmingBlocks, BatteryState, DebugRecord, FlightMode, Tune};

/// Betaflight's font's symbols (`drivers/osd_symbols.h`): codes in its
/// 256-character map. Letters, digits and punctuation keep their ASCII
/// codes.
pub mod symbols {
    pub const BLACKBOX_LOG: u8 = 0x10;
    pub const THROTTLE: u8 = 0x04;
    pub const VOLT: u8 = 0x06;
    pub const MAH: u8 = 0x07;
    pub const LINK_QUALITY: u8 = 0x7B;
    pub const CROSSHAIR_LEFT: u8 = 0x72;
    pub const CROSSHAIR_CENTRE: u8 = 0x73;
    pub const CROSSHAIR_RIGHT: u8 = 0x74;
    pub const BATTERY_EMPTY: u8 = 0x96;
    pub const MAIN_BATTERY: u8 = 0x97;
    pub const AMP: u8 = 0x9A;
    pub const ON_MINUTES: u8 = 0x9B;
    pub const FLY_MINUTES: u8 = 0x9C;
    pub const BLANK: u8 = b' ';
}

use symbols::*;

/// The OSD redraws every 1,000,000 ÷ 12 µs, in whole µs
/// (`OSD_UPDATE_INTERVAL_US`).
pub const REDRAW_EVERY_US: u64 = 1_000_000 / 12;
/// Blinking: hidden for the first half of each 500 ms.
const BLINK_PERIOD_US: u64 = 500_000;
/// `ARMED` shows this long.
const ARMED_FOR_US: u64 = 500_000;
/// The statistics page shows at most this long, and is redrawn this often.
const STATISTICS_FOR_US: u64 = 60_000_000;
const STATISTICS_EVERY_US: u64 = 1_000_000;
/// A stick past this dismisses the statistics page, in µs (`IS_HI` in
/// `osd.c`).
const STICK_HIGH_US: f64 = 1750.0;

/// What the screen shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    /// The layout's elements.
    Elements,
    /// `ARMED`, for 0.5 s at arming.
    Armed,
    /// The statistics page after disarming.
    Statistics,
}

/// What one element showed at the last redraw.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Reading {
    /// Its characters, a line each (the motor RPM has four).
    lines: Vec<Glyphs>,
    /// Whether it's on the screen now: drawn, and not hidden by its blink.
    shown: bool,
    blinking: bool,
}

/// The OSD screen as the last redraw left it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OsdScreen {
    /// Each cell's character, row by row, in Betaflight's character map.
    pub cells: [[u8; COLUMNS]; ROWS],
    /// Which cells hold a symbol rather than a letter, digit or punctuation:
    /// some symbols share their codes with letters.
    symbols: [[bool; COLUMNS]; ROWS],
    pub page: Page,
    readings: Vec<Reading>,
    /// The statistics page's rows, label and value, in order.
    statistics: Vec<(String, String)>,
    /// `ARMED`'s lines.
    message: Vec<String>,
}

impl OsdScreen {
    fn blank() -> OsdScreen {
        OsdScreen {
            cells: [[BLANK; COLUMNS]; ROWS],
            symbols: [[false; COLUMNS]; ROWS],
            page: Page::Elements,
            readings: vec![Reading::default(); OsdElement::ALL.len()],
            statistics: Vec::new(),
            message: Vec::new(),
        }
    }

    /// What an element reads, as the pilot would read it out: its text,
    /// with Betaflight's unit symbols as `V`, `A` and `mAh` and its other
    /// symbols (the battery, throttle, Link Quality and timer glyphs) left
    /// out, its lines joined with " / ", spaces at either end trimmed. It
    /// holds its last drawn text while it blinks off; it's blank when it
    /// isn't drawn, or a page covers the screen.
    pub fn reads(&self, element: OsdElement) -> String {
        if self.page != Page::Elements {
            return String::new();
        }
        let lines: Vec<String> = self.readings[element.index()]
            .lines
            .iter()
            .map(|line| readable(line))
            .filter(|line| !line.is_empty())
            .collect();
        lines.join(" / ")
    }

    /// Whether an element is blinking.
    pub fn blinks(&self, element: OsdElement) -> bool {
        self.page == Page::Elements && self.readings[element.index()].blinking
    }

    /// Whether an element is on the screen now: drawn, and not blinked off.
    pub fn shows(&self, element: OsdElement) -> bool {
        self.page == Page::Elements && self.readings[element.index()].shown
    }

    /// What the screen says in its middle while a page covers the
    /// elements: `ARMED`, and `>CRASH FLIP<` under it in Crash Flip, joined
    /// with " / ". Blank otherwise.
    pub fn message(&self) -> String {
        match self.page {
            Page::Armed => self.message.join(" / "),
            _ => String::new(),
        }
    }

    /// The statistics page, row by row as "LABEL : VALUE", joined with
    /// " / ", under its heading. Blank when it isn't showing.
    pub fn statistics_page(&self) -> String {
        if self.page != Page::Statistics {
            return String::new();
        }
        let mut rows = vec!["--- STATS ---".to_string()];
        rows.extend(
            self.statistics
                .iter()
                .map(|(label, value)| format!("{label} : {value}")),
        );
        rows.join(" / ")
    }

    /// One statistic's value on the page, by its label (`TOTAL ARM`, `MIN
    /// BATTERY`…), any case; `None` when the page doesn't show it.
    pub fn statistic(&self, label: &str) -> Option<String> {
        if self.page != Page::Statistics {
            return None;
        }
        self.statistics
            .iter()
            .find(|(l, _)| l.eq_ignore_ascii_case(label.trim()))
            .map(|(_, value)| value.clone())
    }

    /// Each row of cells as text, unit symbols as `V`, `A` and `M`, other
    /// symbols as `#`: a picture of the screen for checks and logs.
    pub fn picture(&self) -> Vec<String> {
        self.cells
            .iter()
            .zip(&self.symbols)
            .map(|(row, symbols)| {
                row.iter()
                    .zip(symbols)
                    .map(|(&c, &symbol)| match (symbol, c) {
                        (false, _) => c as char,
                        (true, VOLT) => 'V',
                        (true, AMP) => 'A',
                        (true, MAH) => 'M',
                        (true, _) => '#',
                    })
                    .collect()
            })
            .collect()
    }

    fn write(&mut self, column: usize, row: usize, text: &[u16]) {
        if row >= ROWS {
            return;
        }
        for (i, &glyph) in text.iter().enumerate() {
            if let Some(cell) = self.cells[row].get_mut(column + i) {
                *cell = (glyph & 0xFF) as u8;
                self.symbols[row][column + i] = glyph >= SYMBOL;
            }
        }
    }

    fn clear(&mut self) {
        self.cells = [[BLANK; COLUMNS]; ROWS];
        self.symbols = [[false; COLUMNS]; ROWS];
    }
}

/// An element's characters: letters, digits and punctuation as their ASCII
/// codes, and Betaflight's symbols as their code plus [`SYMBOL`], since some
/// share their codes with letters.
type Glyphs = Vec<u16>;

/// Marks a code as a symbol in [`Glyphs`].
const SYMBOL: u16 = 0x100;

fn symbol(code: u8) -> u16 {
    SYMBOL | u16::from(code)
}

fn glyphs(text: &str) -> Glyphs {
    text.bytes().map(u16::from).collect()
}

/// Characters as a pilot reads them: letters, digits and punctuation as
/// they are, the unit symbols as `V`, `A` and `mAh`, the other symbols left
/// out; trimmed.
fn readable(text: &[u16]) -> String {
    let mut out = String::new();
    for &glyph in text {
        match glyph {
            0x20..=0x7E => out.push(glyph as u8 as char),
            g if g == symbol(VOLT) => out.push('V'),
            g if g == symbol(AMP) => out.push('A'),
            g if g == symbol(MAH) => out.push_str("mAh"),
            _ => {}
        }
    }
    out.trim().to_string()
}

/// The statistics since arming (`statistic_t`), in Betaflight's units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Statistics {
    /// µs.
    armed_time: u64,
    /// Hundredths of a volt.
    min_voltage: u16,
    end_voltage: u16,
    /// Hundredths of an amp.
    max_current: i32,
    /// Percent.
    min_link_quality: u16,
    max_rpm: i64,
}

impl Statistics {
    /// `osdResetStats`.
    fn reset() -> Statistics {
        Statistics {
            armed_time: 0,
            min_voltage: 5000,
            end_voltage: 0,
            max_current: 0,
            min_link_quality: 100,
            max_rpm: 0,
        }
    }
}

/// Betaflight's OSD, worked out beside the Simulation for one Quad.
#[derive(Clone, Debug)]
pub struct Osd {
    layout: OsdLayout,
    /// The Quad's name as the craft name element writes it: upper case.
    craft_name: Glyphs,
    cell_min: u16,
    cell_max: u16,
    cell_full: u16,
    /// The Blackbox log being written, if one is.
    blackbox_log: Option<u32>,
    state: State,
    screen: OsdScreen,
}

/// Everything the OSD remembers, which power-up starts afresh.
#[derive(Clone, Debug)]
struct State {
    /// The Flight Controller's clock at the last loop seen; `None` before
    /// the first.
    last_loop_us: Option<u64>,
    next_redraw_us: u64,
    /// Armed, as of the last redraw (`armState`).
    armed: bool,
    /// Armed at any time since power-up (`WAS_EVER_ARMED`).
    ever_armed: bool,
    /// The armed time since power-up (`osdFlyTime`), in µs.
    fly_time: u64,
    /// The last redraw's moment (`lastTimeUs`).
    last_redraw_us: u64,
    statistics: Statistics,
    statistics_enabled: bool,
    statistics_visible: bool,
    statistics_redraw_us: u64,
    /// While a page covers the screen: when the elements come back
    /// (`resumeRefreshAt`).
    resume_at: Option<u64>,
    /// Each element's blink, in [`OsdElement::ALL`]'s order (`blinkBits`).
    blinking: [bool; 21],
    /// The beeps the Flight Controller had started when last seen.
    beeps_seen: u32,
    /// A beep started that the warnings haven't shown yet.
    visual_beeper: bool,
    /// The arming refusal the warnings show, as its bit, or 0.
    refusal_shown: u32,
    refusal_shown_at: u64,
}

impl State {
    fn fresh() -> State {
        State {
            last_loop_us: None,
            next_redraw_us: 0,
            armed: false,
            ever_armed: false,
            fly_time: 0,
            last_redraw_us: 0,
            statistics: Statistics::reset(),
            statistics_enabled: false,
            statistics_visible: false,
            statistics_redraw_us: 0,
            resume_at: None,
            blinking: [false; 21],
            beeps_seen: 0,
            visual_beeper: false,
            refusal_shown: 0,
            refusal_shown_at: 0,
        }
    }
}

impl Osd {
    /// The OSD for a Quad named `quad_name` flying `tune` (whose cell
    /// voltages pick the battery glyph and `BATT < FULL`), drawing `layout`.
    pub fn new(layout: OsdLayout, quad_name: &str, tune: &Tune) -> Osd {
        Osd {
            layout,
            craft_name: craft_name(quad_name),
            cell_min: tune.vbat_min_cell_voltage,
            cell_max: tune.vbat_max_cell_voltage,
            cell_full: tune.vbat_full_cell_voltage,
            blackbox_log: None,
            state: State::fresh(),
            screen: OsdScreen::blank(),
        }
    }

    pub fn layout(&self) -> &OsdLayout {
        &self.layout
    }

    /// A new layout, from the next redraw.
    pub fn set_layout(&mut self, layout: OsdLayout) {
        self.layout = layout;
    }

    /// The Blackbox log being written, by its number, or `None`: the log
    /// status element and the statistics page show it.
    pub fn set_blackbox_log(&mut self, log: Option<u32>) {
        self.blackbox_log = log;
    }

    pub fn screen(&self) -> &OsdScreen {
        &self.screen
    }

    /// Sees one Flight Controller loop, and redraws when one is due by its
    /// clock.
    pub fn see(&mut self, record: &DebugRecord) {
        let now = record.since_power_up_us;
        if self.state.last_loop_us.is_none_or(|last| now < last) {
            // Power-up: everything starts afresh.
            self.state = State::fresh();
            self.state.armed = record.armed;
            self.state.ever_armed = record.armed;
            self.state.beeps_seen = 0;
            self.screen = OsdScreen::blank();
        }
        self.state.last_loop_us = Some(now);
        if record.beeper.beeps > self.state.beeps_seen {
            self.state.visual_beeper = true;
            self.state.beeps_seen = record.beeper.beeps;
        }
        if now < self.state.next_redraw_us {
            return;
        }
        while self.state.next_redraw_us <= now {
            self.state.next_redraw_us += REDRAW_EVERY_US;
        }
        self.redraw(now, record);
    }

    /// One redraw (`osdUpdate`'s states, in order).
    fn redraw(&mut self, now: u64, record: &DebugRecord) {
        let refresh_statistics = self.process_statistics(now, record);
        if refresh_statistics {
            self.draw_statistics(record);
        }
        if let Some(resume_at) = self.state.resume_at {
            if now < resume_at {
                let [_, pitch, _, throttle] = record.sticks_us;
                if !record.armed
                    && (throttle > STICK_HIGH_US
                        || pitch > STICK_HIGH_US
                        || record.crash_flip_switch)
                {
                    self.state.resume_at = Some(now);
                }
                return;
            }
            self.state.resume_at = None;
            self.state.statistics_enabled = false;
            self.state.statistics.armed_time = 0;
        }
        self.update_alarms(now, record);
        self.draw_elements(now, record);
    }

    /// `osdProcessStats1`: arming and disarming, the statistics and the
    /// armed time. True when the statistics page is due a redraw.
    fn process_statistics(&mut self, now: u64, record: &DebugRecord) -> bool {
        let state = &mut self.state;
        if state.armed != record.armed {
            if record.armed {
                state.statistics_enabled = false;
                state.statistics_visible = false;
                state.statistics = Statistics::reset();
                state.ever_armed = true;
                state.resume_at = Some(now + ARMED_FOR_US);
                self.show_armed(record);
            } else if self.layout.statistics != 0
                && !record.failsafe.active
                && (!record.arming_blocks.runaway
                    || !self.layout.placement(OsdElement::Warnings).visible)
            {
                let state = &mut self.state;
                state.statistics_enabled = true;
                state.resume_at = Some(now + STATISTICS_FOR_US);
                state.statistics.end_voltage = record.battery.voltage;
            }
            self.state.armed = record.armed;
        }
        let state = &mut self.state;
        let mut refresh = false;
        if record.armed {
            let statistics = &mut state.statistics;
            statistics.min_voltage = statistics.min_voltage.min(record.battery.voltage);
            if let Some(current) = record.battery.current {
                statistics.max_current = statistics.max_current.max(current);
            }
            statistics.min_link_quality = statistics
                .min_link_quality
                .min(u16::from(record.link_quality));
            if let Some(rpm) = record.motor_rpm {
                let sum: u64 = rpm.iter().map(|&r| u64::from(r)).sum();
                let average = (sum as f64 / 4.0).round() as i64;
                statistics.max_rpm = statistics.max_rpm.max(average);
            }
            let since = now.saturating_sub(state.last_redraw_us);
            state.fly_time += since;
            statistics.armed_time += since;
        } else if state.statistics_enabled {
            if !state.statistics_visible {
                state.statistics_visible = true;
                state.statistics_redraw_us = 0;
            }
            if now >= state.statistics_redraw_us {
                state.statistics_redraw_us = now + STATISTICS_EVERY_US;
                refresh = true;
            }
        }
        state.last_redraw_us = now;
        refresh
    }

    /// `osdShowArmed`.
    fn show_armed(&mut self, record: &DebugRecord) {
        let screen = &mut self.screen;
        screen.clear();
        screen.page = Page::Armed;
        screen.message = vec!["ARMED".to_string()];
        let (middle_row, middle_column) = (ROWS / 2, COLUMNS / 2);
        screen.write(
            middle_column - "ARMED".len() / 2,
            middle_row,
            &glyphs("ARMED"),
        );
        if record.crash_flip {
            screen.write(
                middle_column - CRASH_FLIP.len() / 2,
                middle_row + 1,
                &glyphs(CRASH_FLIP),
            );
            screen.message.push(CRASH_FLIP.to_string());
        }
    }

    /// `osdUpdateAlarms`: the elements that blink past their alarm.
    fn update_alarms(&mut self, now: u64, record: &DebugRecord) {
        let alarms = self.layout.alarms;
        let battery_not_ok = record.battery.state != BatteryState::Ok;
        let mah = record.battery.mah_drawn.unwrap_or(0);
        let timers = [0, 1].map(|i| {
            let timer = self.layout.timers[i];
            let alarm = u64::from(timer.alarm_minutes) * 60_000_000;
            alarm != 0 && self.timer_value(timer.source, now, record) >= alarm
        });
        let blinking = &mut self.state.blinking;
        let mut set = |element: OsdElement, on: bool| blinking[element.index()] = on;
        // No RSSI source: Betaflight reads 0%, below any alarm.
        set(OsdElement::Rssi, alarms.rssi > 0);
        set(OsdElement::RssiDbm, true);
        set(
            OsdElement::LinkQuality,
            u16::from(record.link_quality) < alarms.link_quality,
        );
        set(OsdElement::BatteryVoltage, battery_not_ok);
        set(OsdElement::AverageCellVoltage, battery_not_ok);
        set(OsdElement::Timer1, timers[0]);
        set(OsdElement::Timer2, timers[1]);
        set(OsdElement::MahDrawn, mah >= i32::from(alarms.capacity));
    }

    /// The canvas: every visible element, in Betaflight's order, each
    /// hidden while it blinks off.
    fn draw_elements(&mut self, now: u64, record: &DebugRecord) {
        self.screen.clear();
        self.screen.page = Page::Elements;
        self.screen.message.clear();
        self.screen.statistics.clear();
        let blink_off = now % BLINK_PERIOD_US < BLINK_PERIOD_US / 2;
        for element in OsdElement::DRAWING_ORDER {
            let placement = self.layout.placement(element);
            let index = element.index();
            if !placement.visible {
                self.screen.readings[index] = Reading::default();
                continue;
            }
            if self.state.blinking[index] && blink_off {
                let reading = &mut self.screen.readings[index];
                reading.shown = false;
                reading.blinking = true;
                continue;
            }
            let lines = self.element_lines(element, now, record);
            for (offset, line) in lines.iter().enumerate() {
                self.screen.write(
                    usize::from(placement.column),
                    usize::from(placement.row) + offset,
                    line,
                );
            }
            self.screen.readings[index] = Reading {
                shown: !lines.is_empty(),
                lines,
                blinking: self.state.blinking[index],
            };
        }
    }

    /// What an element draws now, a line each; none for blank.
    fn element_lines(
        &mut self,
        element: OsdElement,
        now: u64,
        record: &DebugRecord,
    ) -> Vec<Glyphs> {
        let battery = record.battery;
        let line = |text: Glyphs| vec![text];
        match element {
            OsdElement::Rssi
            | OsdElement::RssiDbm
            | OsdElement::Rsnr
            | OsdElement::ArtificialHorizon
            | OsdElement::HorizonSidebars
            | OsdElement::FlipArrow => Vec::new(),
            OsdElement::BatteryVoltage => {
                let volts = f64::from(battery.voltage) / 100.0;
                let decimals = if volts >= 10.0 { 1 } else { 2 };
                let symbol = self.battery_symbol(record);
                line(print_number(
                    Some(symbol),
                    volts,
                    None,
                    decimals,
                    true,
                    Some(VOLT),
                ))
            }
            OsdElement::AverageCellVoltage => {
                let cell = battery.average_cell_voltage();
                let symbol = self.battery_symbol(record);
                line(print_number(
                    Some(symbol),
                    f64::from(cell) / 100.0,
                    None,
                    2,
                    false,
                    Some(VOLT),
                ))
            }
            OsdElement::Crosshairs => line(
                [CROSSHAIR_LEFT, CROSSHAIR_CENTRE, CROSSHAIR_RIGHT]
                    .map(symbol)
                    .to_vec(),
            ),
            OsdElement::Timer1 | OsdElement::Timer2 => {
                let timer = self.layout.timers[usize::from(element == OsdElement::Timer2)];
                let mut text = vec![timer_symbol(timer.source, record.armed)];
                text.extend(format_time(
                    timer.precision,
                    self.timer_value(timer.source, now, record),
                ));
                line(text)
            }
            OsdElement::FlightMode => line(glyphs(flight_mode_text(record))),
            OsdElement::CraftName => line(self.craft_name.clone()),
            OsdElement::Throttle => {
                let mut text = vec![symbol(THROTTLE)];
                text.extend(glyphs(&format!("{:3}", record.throttle_percent)));
                line(text)
            }
            OsdElement::Current => match battery.current {
                None => Vec::new(),
                Some(current) => line(print_number(
                    None,
                    f64::from(current.unsigned_abs()) / 100.0,
                    Some(3),
                    2,
                    false,
                    Some(AMP),
                )),
            },
            OsdElement::MahDrawn => match battery.mah_drawn {
                None => Vec::new(),
                Some(mah) => {
                    let mut text = glyphs(&format!("{mah:4}"));
                    text.push(symbol(MAH));
                    line(text)
                }
            },
            OsdElement::Warnings => {
                let (text, blinks) = self.warning(now, record);
                self.state.blinking[OsdElement::Warnings.index()] = blinks;
                if text.is_empty() {
                    Vec::new()
                } else {
                    line(glyphs(&text))
                }
            }
            OsdElement::Disarmed => {
                if record.armed {
                    Vec::new()
                } else {
                    line(glyphs("DISARMED"))
                }
            }
            OsdElement::MotorRpm => match record.motor_rpm {
                None => Vec::new(),
                Some(rpm) => rpm
                    .iter()
                    .map(|&r| glyphs(&r.min(99_999).to_string()))
                    .collect(),
            },
            OsdElement::LinkQuality => {
                let mut text = vec![symbol(LINK_QUALITY)];
                text.extend(glyphs(&format!(
                    "{}:{:2}",
                    record.rf_mode, record.link_quality
                )));
                line(text)
            }
            OsdElement::LogStatus => match self.blackbox_log {
                None => Vec::new(),
                Some(log) => {
                    let mut text = vec![symbol(BLACKBOX_LOG)];
                    text.extend(glyphs(&log.to_string()));
                    line(text)
                }
            },
        }
    }

    /// `osdGetBatterySymbol`: the battery glyph for the average cell
    /// voltage, or the bare battery when critical.
    fn battery_symbol(&self, record: &DebugRecord) -> u8 {
        if record.battery.state == BatteryState::Critical {
            return MAIN_BATTERY;
        }
        let cell = i64::from(record.battery.average_cell_voltage());
        let (low, high) = (i64::from(self.cell_min), i64::from(self.cell_max));
        // `scaleRange` onto 0–8, in whole numbers.
        let offset = if high == low {
            0
        } else {
            (8 * (cell - low)) / (high - low)
        };
        BATTERY_EMPTY - offset.clamp(0, 6) as u8
    }

    /// `osdGetTimerValue`, in µs.
    fn timer_value(&self, source: TimerSource, now: u64, record: &DebugRecord) -> u64 {
        match source {
            TimerSource::OnTime => now,
            TimerSource::TotalArmed | TimerSource::LaunchTime => self.state.fly_time,
            TimerSource::LastArmed => self.state.statistics.armed_time,
            TimerSource::OnOrArmed => {
                if record.armed {
                    self.state.fly_time
                } else {
                    now
                }
            }
        }
    }

    /// `renderOsdWarning`: the first warning that applies, and whether it
    /// blinks.
    fn warning(&mut self, now: u64, record: &DebugRecord) -> (String, bool) {
        let warnings = self.layout.warnings;
        let on = |warning: Warning| warnings & warning.bit() != 0;
        let battery = record.battery;
        if on(Warning::ArmingDisabled) {
            if record.arm_switch && record.arming_blocks.any() {
                let arm_switch = 1u32 << ArmingBlocks::BITS[8];
                let mut flags = record.arming_blocks.bits();
                if flags != arm_switch {
                    flags &= !arm_switch;
                }
                let state = &mut self.state;
                if now.saturating_sub(state.refusal_shown_at) > 500_000
                    || flags & state.refusal_shown == 0
                {
                    state.refusal_shown_at = now;
                    // The next flag up, or round again from the lowest.
                    let next = state.refusal_shown << 1;
                    let above = if next == 0 { 0 } else { flags & !(next - 1) };
                    let lowest = |bits: u32| bits.isolate_lowest_one();
                    state.refusal_shown = match lowest(above) {
                        0 => lowest(flags),
                        flag => flag,
                    };
                }
                let bit = self.state.refusal_shown.trailing_zeros() as u8;
                let name = ArmingBlocks::name_of_bit(bit).unwrap_or("");
                return (name.to_string(), false);
            }
            self.state.refusal_shown = 0;
        }
        if on(Warning::FailSafe) && record.failsafe.active {
            return ("FAIL SAFE".to_string(), true);
        }
        if on(Warning::CrashFlip) && record.crash_flip_switch {
            if record.crash_flip {
                return (CRASH_FLIP.to_string(), false);
            } else if !record.armed {
                return ("CRASHFLIP SW".to_string(), false);
            }
        }
        if on(Warning::LinkQuality)
            && u16::from(record.link_quality) < self.layout.alarms.link_quality
        {
            return ("LINK QUALITY".to_string(), true);
        }
        if on(Warning::BatteryCritical) && battery.state == BatteryState::Critical {
            return (" LAND NOW".to_string(), true);
        }
        if on(Warning::BatteryWarning) && battery.state == BatteryState::Warning {
            return ("LOW BATTERY".to_string(), true);
        }
        let capacity = self.layout.alarms.capacity;
        if on(Warning::OverCapacity)
            && record.armed
            && capacity > 0
            && battery.mah_drawn.unwrap_or(0) >= i32::from(capacity)
        {
            return ("OVER CAP".to_string(), true);
        }
        if on(Warning::BatteryNotFull)
            && !(record.armed || self.state.ever_armed)
            && battery.state == BatteryState::Ok
            && battery.average_cell_voltage() < self.cell_full
        {
            return ("BATT < FULL".to_string(), false);
        }
        if on(Warning::VisualBeeper) && self.state.visual_beeper {
            self.state.visual_beeper = false;
            return ("  * * * *".to_string(), false);
        }
        (String::new(), false)
    }

    /// `osdRefreshStats`: the page, centred, with its heading when there's
    /// room.
    fn draw_statistics(&mut self, record: &DebugRecord) {
        let rows: Vec<(String, String)> = Statistic::DISPLAY_ORDER
            .into_iter()
            .filter(|statistic| self.layout.statistic_on(*statistic))
            .filter_map(|statistic| self.statistic_row(statistic, record))
            .take(ROWS)
            .collect();
        let screen = &mut self.screen;
        screen.clear();
        screen.page = Page::Statistics;
        screen.message.clear();
        let heading = rows.len() < ROWS;
        let shown = rows.len() + usize::from(heading);
        let mut row = (ROWS - shown.min(ROWS)) / 2;
        let middle = COLUMNS / 2;
        if heading {
            let title = "--- STATS ---";
            screen.write(middle - title.len() / 2, row, &glyphs(title));
            row += 1;
        }
        for (label, value) in &rows {
            screen.write(middle - 13, row, &glyphs(label));
            screen.write(middle + 5, row, &glyphs(":"));
            screen.write(middle + 7, row, &statistic_bytes(value));
            row += 1;
        }
        screen.statistics = rows
            .into_iter()
            .map(|(label, value)| (label.trim().to_string(), value))
            .collect();
    }

    /// One row of the statistics page, label and value, or `None` for a
    /// statistic with no source in the sim (#31 §4).
    fn statistic_row(
        &self,
        statistic: Statistic,
        record: &DebugRecord,
    ) -> Option<(String, String)> {
        let statistics = self.state.statistics;
        let volts = |hundredths: u16| {
            readable(&print_number(
                None,
                f64::from(hundredths) / 100.0,
                None,
                2,
                true,
                Some(VOLT),
            ))
        };
        let row = |label: &str, value: String| Some((label.to_string(), value));
        match statistic {
            Statistic::Timer1 | Statistic::Timer2 => {
                let timer = self.layout.timers[usize::from(statistic == Statistic::Timer2)];
                let precision = if timer.source == TimerSource::OnTime {
                    TimerPrecision::Seconds
                } else {
                    timer.precision
                };
                let value = self.timer_value(timer.source, self.state.last_redraw_us, record);
                row(
                    timer.source.label(),
                    readable(&format_time(precision, value)),
                )
            }
            Statistic::MinBattery => row("MIN BATTERY", volts(statistics.min_voltage)),
            Statistic::EndBattery => row("END BATTERY", volts(statistics.end_voltage)),
            Statistic::Battery => row("BATTERY", volts(record.battery.voltage)),
            Statistic::MaxCurrent => record.battery.current.and(row(
                "MAX CURRENT",
                readable(&print_number(
                    None,
                    f64::from(statistics.max_current) / 100.0,
                    None,
                    2,
                    false,
                    Some(AMP),
                )),
            )),
            Statistic::UsedMah => record
                .battery
                .mah_drawn
                .and_then(|mah| row("USED MAH", format!("{mah}mAh"))),
            Statistic::BlackboxNumber => self
                .blackbox_log
                .and_then(|log| row("BB LOG NUM", log.to_string())),
            Statistic::MaxEscRpm => record
                .motor_rpm
                .and(row("MAX ESC RPM", statistics.max_rpm.to_string())),
            Statistic::MinLinkQuality => {
                row("MIN LINK", format!("{}%", statistics.min_link_quality))
            }
            _ => None,
        }
    }
}

/// Crash Flip's warning in 2026.6 (`CRASHFLIP_WARNING`).
const CRASH_FLIP: &str = ">CRASH FLIP<";

/// The Flight Mode element's text (`osdElementFlymode`): Failsafe first,
/// then Angle and Horizon, then Airmode, which is always on.
fn flight_mode_text(record: &DebugRecord) -> &'static str {
    if record.failsafe.active {
        "!FS!"
    } else {
        match record.flight_mode {
            FlightMode::Angle => "ANGL",
            FlightMode::Horizon => "HOR ",
            FlightMode::Acro => "AIR ",
        }
    }
}

/// `osdGetTimerSymbol`.
fn timer_symbol(source: TimerSource, armed: bool) -> u16 {
    match source {
        TimerSource::OnTime => symbol(ON_MINUTES),
        TimerSource::TotalArmed | TimerSource::LastArmed => symbol(FLY_MINUTES),
        TimerSource::OnOrArmed => {
            if armed {
                symbol(FLY_MINUTES)
            } else {
                symbol(ON_MINUTES)
            }
        }
        TimerSource::LaunchTime => u16::from(b'L'),
    }
}

/// `osdFormatTime`: minutes and seconds, with tenths or hundredths.
fn format_time(precision: TimerPrecision, micros: u64) -> Glyphs {
    let seconds = micros / 1_000_000;
    let (minutes, seconds) = (seconds / 60, seconds % 60);
    match precision {
        TimerPrecision::Seconds => format!("{minutes:02}:{seconds:02}"),
        TimerPrecision::Hundredths => {
            format!("{minutes:02}:{seconds:02}.{:02}", (micros / 10_000) % 100)
        }
        TimerPrecision::Tenths => format!("{minutes:02}:{seconds:02}.{}", (micros / 100_000) % 10),
    }
    .bytes()
    .map(u16::from)
    .collect()
}

/// `osdPrintFloat`: a number with a fixed count of decimals, rounded or cut,
/// its whole part at least `width` wide, with symbols before and after.
fn print_number(
    leading: Option<u8>,
    value: f64,
    width: Option<usize>,
    decimals: u32,
    round: bool,
    trailing: Option<u8>,
) -> Glyphs {
    let multiplier = 10u64.pow(decimals);
    let scaled = value * multiplier as f64;
    let scaled_abs = if round {
        scaled.round().abs() as u64
    } else {
        scaled.trunc().abs() as u64
    };
    let (whole, fraction) = (scaled_abs / multiplier, scaled_abs % multiplier);
    let mut text = Vec::new();
    text.extend(leading.map(symbol));
    if value < 0.0 && scaled_abs > 0 {
        text.push(u16::from(b'-'));
    }
    let whole = match width {
        Some(width) => format!("{whole:width$}"),
        None => whole.to_string(),
    };
    text.extend(glyphs(&whole));
    if decimals > 0 {
        text.extend(glyphs(&format!(
            ".{fraction:0width$}",
            width = decimals as usize
        )));
    }
    text.extend(trailing.map(symbol));
    text
}

/// A statistic's value as the page writes it: `V`, `A` and `mAh` back as
/// Betaflight's symbols.
fn statistic_bytes(value: &str) -> Glyphs {
    if let Some(number) = value.strip_suffix("mAh") {
        let mut text = glyphs(number);
        text.push(symbol(MAH));
        return text;
    }
    let mut text = glyphs(value);
    if let Some(last) = text.last_mut() {
        if *last == u16::from(b'V') {
            *last = symbol(VOLT);
        } else if *last == u16::from(b'A') {
            *last = symbol(AMP);
        }
    }
    text
}

/// The Quad's name as the craft name element writes it: upper case, at most
/// 16 characters (`MAX_NAME_LENGTH`), on Betaflight's font: `″` as `"`,
/// anything else outside plain ASCII left out.
fn craft_name(name: &str) -> Glyphs {
    name.chars()
        .filter_map(|c| match c {
            '″' | '”' | '“' => Some(u16::from(b'"')),
            '′' | '’' | '‘' => Some(u16::from(b'\'')),
            c if c.is_ascii() && !c.is_ascii_control() => Some(c.to_ascii_uppercase() as u16),
            _ => None,
        })
        .take(16)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_print_as_betaflight_prints_them() {
        assert_eq!(
            readable(&print_number(None, 3.478, None, 2, false, Some(VOLT))),
            "3.47V"
        );
        assert_eq!(
            readable(&print_number(None, 25.19, None, 1, true, Some(VOLT))),
            "25.2V"
        );
        assert_eq!(
            print_number(None, 1.5, Some(3), 2, false, Some(AMP)),
            [32, 32, 49, 46, 53, 48, symbol(AMP)]
        );
    }

    #[test]
    fn the_freestyle_5s_name_reads_in_capitals_with_a_plain_inch_mark() {
        assert_eq!(craft_name("Freestyle 5″"), glyphs("FREESTYLE 5\""));
    }

    #[test]
    fn timers_count_minutes_and_seconds() {
        assert_eq!(
            format_time(TimerPrecision::Seconds, 83_500_000),
            glyphs("01:23")
        );
        assert_eq!(
            format_time(TimerPrecision::Tenths, 83_560_000),
            glyphs("01:23.5")
        );
        assert_eq!(
            format_time(TimerPrecision::Hundredths, 83_567_000),
            glyphs("01:23.56")
        );
    }
}
