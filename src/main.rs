use std::{
    fs::{self, File},
    io::Write,
    process::exit,
};

use anyhow::{Error, Result};
use clap::Parser;
use csv::Writer;
use dialoguer::MultiSelect;
use once_cell::sync::Lazy;
use rayon::{
    iter::{IntoParallelIterator, ParallelIterator},
    str::ParallelString,
};
use regex::Regex;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, help = "Path to log file")]
    input: String,

    #[arg(
        short,
        long,
        help = "Format of output file",
        default_value_t,
        value_enum
    )]
    format: OutputFormat,

    #[arg(short, long, help = "Path to output  file")]
    output: String,
}

#[derive(clap::ValueEnum, Debug, Clone, Default)]
enum OutputFormat {
    Image,
    CSV,
    #[default]
    TXT,
}

/// follow up by extracting just the user and the message
static EXTRACT_MSG: Lazy<Regex> = Lazy::new(|| Regex::new(r"<.*> .*").unwrap());
static EXTRACT_TIME: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\[.*?\]").unwrap());

fn main() -> Result<()> {
    let args = Args::parse();

    if !fs::exists(&args.input)? {
        println!("Please provide a valid file path!");
        exit(1)
    }

    let log_contents = fs::read_to_string(&args.input)?;

    let split_log = log_contents.par_split('\n');

    let filtered_log: Vec<&str> = split_log.filter(|x| is_possible_chat_msg(x)).collect();

    let extracted: Vec<String> = filtered_log
        .clone()
        .into_par_iter()
        .map(|x| {
            format!(
                "{} {}",
                EXTRACT_TIME
                    .captures(x)
                    .expect("Unable to extract time")
                    .get(0)
                    .unwrap()
                    .as_str(),
                EXTRACT_MSG
                    .captures(x)
                    .expect("Unable to extract message")
                    .get(0)
                    .unwrap()
                    .as_str()
            )
        })
        .collect();

    let selection = MultiSelect::new()
        .with_prompt("What messages do you want to render?")
        .items(&extracted)
        .interact()?;

    match &args.format {
        OutputFormat::Image => todo!(),
        OutputFormat::CSV => save_csv_file(&args.output, extracted, selection)?,
        OutputFormat::TXT => save_txt_file(&args.output, extracted, selection)?,
    }

    Ok(())
}

fn save_txt_file(
    output: &String,
    extracted: Vec<String>,
    selection: Vec<usize>,
) -> Result<(), Error> {
    let mut out_file = File::create(output)?;
    Ok(for msg in selection {
        out_file.write(extracted[msg].as_bytes())?;
        out_file.write(b"\n")?;
    })
}

fn save_csv_file(
    output: &String,
    extracted: Vec<String>,
    selection: Vec<usize>,
) -> Result<(), Error> {
    let mut out_file = Writer::from_path(output)?;
    out_file.write_record(&["date", "time", "msg"])?;

    Ok(for msg in selection {
        let time: Vec<&str> = EXTRACT_TIME
            .captures(&extracted[msg])
            .expect("Unable to extract time")
            .get(0)
            .unwrap()
            .as_str()
            .strip_prefix("[").expect("Unable to remove time prefix")
            .strip_suffix("]").expect("Unable to remove time suffix")
            .split(' ').collect();
        out_file.write_record(&[
            time[0],
            time[1],
            EXTRACT_MSG
            .captures(&extracted[msg])
            .expect("Unable to extract time")
            .get(0)
            .unwrap()
            .as_str()])?;
    })
}

fn is_possible_chat_msg(input: &str) -> bool {
    /// first pass to find all possible lines that could have a chat message
    static SERVER_MSG_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(
            r"\[.*\] \[Server thread\/INFO\] \[net.minecraft.server.MinecraftServer\/\]: <.*>*",
        )
        .unwrap()
    });
    static CLIENT_MSG_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"\[.*\] \[Render thread\/INFO\] \[minecraft\/ChatComponent\]: \[CHAT\] <.*>*")
            .unwrap()
    });

    return SERVER_MSG_RE.is_match(input) || CLIENT_MSG_RE.is_match(input);
}
