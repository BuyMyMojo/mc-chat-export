use std::{fs, process::exit};

use clap::Parser;
use once_cell::sync::Lazy;
use anyhow::{Error, Result};
use rayon::{iter::ParallelIterator, str::ParallelString};
use regex::Regex;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, help="Path to log file")]
    path: String,

}


/// follow up by extracting just the user and the message
static SECOND_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"<.*> .*").unwrap());

fn main() -> Result<()> {
    let args = Args::parse();

    if !fs::exists(&args.path)? {
        println!("Please provide a valid file path!");
        exit(1)
    }

    let log_contents = fs::read_to_string(args.path)?;

    let split_log = log_contents.par_split('\n');

    let filtered_log: Vec<&str> = split_log.filter(|x| is_possible_chat_msg(x)).collect();

    print!("{:#?}", filtered_log);

    Ok(())
}

fn is_possible_chat_msg(input: &str) -> bool {
    /// first pass to find all possible lines that could have a chat message
    static SERVER_MSG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[.*\] \[Server thread\/INFO\] \[net.minecraft.server.MinecraftServer\/\]: <.*>*").unwrap());
    static CLIENT_MSG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[.*\] \[Render thread\/INFO\] \[minecraft\/ChatComponent\]: \[CHAT\] <.*>*").unwrap());

    return SERVER_MSG_RE.is_match(input) || CLIENT_MSG_RE.is_match(input);
}