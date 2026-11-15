use std::fmt;
use std::fs;
use std::io::{self, IsTerminal};
use std::path::PathBuf;

use tracing::{Event, Subscriber};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::EnvFilter;

struct Plain;

impl<S, N> FormatEvent<S, N> for Plain
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        write!(&mut writer, "[{}] ", event.metadata().level())?;
        ctx.field_format().format_fields(writer.by_ref(), event)?;
        writeln!(&mut writer)
    }
}

pub fn init(verbose: bool) {
    let filter = if verbose {
        EnvFilter::new("privcrab=debug")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("privcrab=info"))
    };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(io::stderr().is_terminal())
        .with_writer(io::stderr)
        .event_format(Plain)
        .try_init();
}

pub fn init_with_file(verbose: bool, log_file: Option<PathBuf>) -> anyhow::Result<()> {
    let filter = if verbose {
        EnvFilter::new("privcrab=debug")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("privcrab=info"))
    };

    if let Some(path) = log_file {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("privcrab.log");
        let appender = RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix(file_name)
            .filename_suffix("log")
            .build(path.parent().unwrap_or_else(|| std::path::Path::new(".")))?;

        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(false)
            .with_writer(appender)
            .event_format(Plain)
            .try_init()?;
    } else {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(io::stderr().is_terminal())
            .with_writer(io::stderr)
            .event_format(Plain)
            .try_init();
    }
    Ok(())
}
