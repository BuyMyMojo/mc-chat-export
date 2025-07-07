use std::{
    fs::{self, File},
    io::Write,
    path::Path,
    process::exit,
};

use ab_glyph::{FontRef, PxScale};
use anyhow::{Error, Result};
use clap::Parser;
use csv::Writer;
use dialoguer::MultiSelect;
use image::{Rgb, RgbImage};
use imageproc::drawing::{draw_text_mut, text_size};
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
        OutputFormat::Image => save_image_file(&args.output, extracted, selection)?,
        OutputFormat::CSV => save_csv_file(&args.output, extracted, selection)?,
        OutputFormat::TXT => save_txt_file(&args.output, extracted, selection)?,
    }

    Ok(())
}

fn save_image_file(
    output: &String,
    extracted: Vec<String>,
    selection: Vec<usize>,
) -> Result<(), Error> {
    let out_path = Path::new(output);

    let font = FontRef::try_from_slice(include_bytes!("../font/Minecraft-Regular.ttf")).unwrap();

    let height = 40.0;
    let scale = PxScale {
        x: height * 2.0,
        y: height,
    };

    let mut selected: Vec<String> = vec![];

    if selection.is_empty() {
        selected = extracted;
    } else {
        for x in selection {
            selected.push(extracted[x].clone());
        }
    }

    let mut longest: String = selected.first().expect("selected list is empty?").clone();

    for msg in &selected {
        if msg.len() > longest.len() {
            longest = msg.clone();
        }
    }

    // let text = extracted[selection[0]].clone();
    let (w, h) = text_size(scale, &font, &longest);
    println!("Text size: {}x{}", w, h);

    let image_width = w + 8;
    let image_height = (h * selected.len() as u32) + (4 * selected.len() as u32) + 4;

    let mut image = RgbImage::new(image_width, image_height);

    let mut current_line: u32 = 0;

    for msg in &selected {
        draw_text_mut(
            &mut image,
            Rgb([254u8, 254u8, 254u8]),
            4,
            ((current_line * h) + (4 * current_line))
                .try_into()
                .unwrap(),
            scale,
            &font,
            &msg,
        );
        current_line += 1;
    }

    Ok(image.save(out_path).unwrap())
}

fn save_txt_file(
    output: &String,
    extracted: Vec<String>,
    selection: Vec<usize>,
) -> Result<(), Error> {
    let mut out_file = File::create(output)?;

    if selection.is_empty() {
        for msg in extracted {
            out_file.write(msg.as_bytes())?;
            out_file.write(b"\n")?;
        }
    } else {
        for msg in selection {
            out_file.write(extracted[msg].as_bytes())?;
            out_file.write(b"\n")?;
        }
    }

    Ok(())
}

fn save_csv_file(
    output: &String,
    extracted: Vec<String>,
    selection: Vec<usize>,
) -> Result<(), Error> {
    let mut out_file = Writer::from_path(output)?;
    out_file.write_record(&["date", "time", "msg"])?;

    let mut selected: Vec<String> = vec![];

    if selection.is_empty() {
        selected = extracted;
    } else {
        for x in selection {
            selected.push(extracted[x].clone());
        }
    }

    for msg in selected {
        let time: Vec<&str> = EXTRACT_TIME
            .captures(&msg)
            .expect("Unable to extract time")
            .get(0)
            .unwrap()
            .as_str()
            .strip_prefix("[")
            .expect("Unable to remove time prefix")
            .strip_suffix("]")
            .expect("Unable to remove time suffix")
            .split(' ')
            .collect();
        out_file.write_record(&[
            time[0],
            time[1],
            EXTRACT_MSG
                .captures(&msg)
                .expect("Unable to extract time")
                .get(0)
                .unwrap()
                .as_str(),
        ])?;
    }

    Ok(())
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
