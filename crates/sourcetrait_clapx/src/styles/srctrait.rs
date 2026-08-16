pub(crate) mod item {
    use crate::*;
    pub const CLAPX_NOOP: Style = Style::new();
    pub const CLAPX_HEADER: Style = AnsiColor::Green.on_default().effects(Effects::BOLD);
    pub const CLAPX_USAGE: Style = AnsiColor::Green.on_default().effects(Effects::BOLD);
    pub const CLAPX_LITERAL: Style = AnsiColor::Cyan.on_default().effects(Effects::BOLD);
    pub const CLAPX_PLACEHOLDER: Style = AnsiColor::Cyan.on_default();
    pub const CLAPX_ERROR: Style = AnsiColor::Red.on_default().effects(Effects::BOLD);
    pub const CLAPX_WARN: Style = AnsiColor::Yellow.on_default().effects(Effects::BOLD);
    pub const CLAPX_NOTE: Style = AnsiColor::Cyan.on_default().effects(Effects::BOLD);
    pub const CLAPX_GOOD: Style = AnsiColor::Green.on_default().effects(Effects::BOLD);
    pub const CLAPX_VALID: Style = AnsiColor::Cyan.on_default().effects(Effects::BOLD);
    pub const CLAPX_INVALID: Style = AnsiColor::Yellow.on_default().effects(Effects::BOLD);
}

pub(crate) mod set {
    use crate::*;
    use super::item::*;
    /// SrcTrait's terminal color style
    pub const SRCTRAIT: Styles = Styles::styled()
        .header(CLAPX_HEADER)
        .usage(CLAPX_USAGE)
        .literal(CLAPX_LITERAL)
        .placeholder(CLAPX_PLACEHOLDER)
        .error(CLAPX_ERROR)
        .valid(CLAPX_VALID)
        .invalid(CLAPX_INVALID);
}

pub(crate) mod util {
    use crate::*;
    use super::item::*;
    pub fn exit_error(e: impl Error) -> std::process::ExitCode {
        let source = e.source()
            .map_or(String::new(), |s| format!("\n       {s}"));
    
        eprintln!("{CLAPX_ERROR}error:{CLAPX_ERROR:#} {e}{source}");
        std::process::ExitCode::FAILURE
    }
}