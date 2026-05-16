use env_logger::Builder;
use log::{Level, LevelFilter};
use std::io::Write;

pub fn init_logger(module: impl Into<String>, level: LevelFilter) {
    const fn mpv_level(level: Level) -> &'static str {
        match level {
            Level::Error => "error",
            Level::Warn => "warn",
            Level::Info => "info",
            Level::Debug => "v",
            Level::Trace => "trace",
        }
    }

    let module = module.into();

    let _ = Builder::new()
        .filter_level(LevelFilter::Off)
        .filter_module(&module, level)
        .format(move |buf, record| writeln!(buf, "[{}] {}: {}", module, mpv_level(record.level()), record.args()))
        .try_init();
}
