use env_logger::{Builder, WriteStyle};
use log::{Level, LevelFilter};
use std::io::Write;

pub fn init_logger(module: impl Into<String>, level: LevelFilter) {
    let module = module.into();

    let _ = Builder::new()
        .filter_level(LevelFilter::Off)
        .filter_module(&module, level)
        .write_style(WriteStyle::Auto)
        .format(move |buf, record| {
            let line = format!("[{}] {}", module, record.args());

            match record.level() {
                Level::Error | Level::Warn => {
                    let style = buf.default_level_style(record.level());
                    writeln!(buf, "{style}{line}{style:#}")
                }
                _ => writeln!(buf, "{line}"),
            }
        })
        .try_init();
}
