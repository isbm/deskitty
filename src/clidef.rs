use clap::builder::styling;
use clap::{Arg, ArgAction, Command};
use colored::Colorize;

pub static APPNAME: &str = "deskitty";

pub fn cli(version: &'static str) -> Command {
    let styles = styling::Styles::styled()
        .header(styling::AnsiColor::Yellow.on_default())
        .usage(styling::AnsiColor::Yellow.on_default())
        .literal(styling::AnsiColor::BrightGreen.on_default())
        .placeholder(styling::AnsiColor::BrightMagenta.on_default());

    Command::new(APPNAME)
        .version(version)
        .about(format!("{} - {}", APPNAME.bright_magenta().bold(), "terminal workspace manager"))
        .override_usage(format!("{APPNAME} [OPTIONS]"))
        .styles(styles)
        .arg(Arg::new("config").short('c').long("config").help("Path to config file").value_name("FILE"))
        .arg(help_arg())
        .disable_help_flag(true)
}

fn help_arg() -> Arg {
    Arg::new("help").short('h').long("help").action(ArgAction::SetTrue).help("Display help")
}
